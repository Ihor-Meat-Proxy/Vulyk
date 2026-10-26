//! Chat propose/create orchestration: placement retrieval + JSON-schema tool fallback.

use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::llm::{LlmProvider, LlmRequest};
use crate::models::{
    ChatConfirmRequest, ChatConfirmResponse, ChatCreationEventPublic, ChatJsonEnvelope,
    ChatPlacement, ChatProposal, ChatProposeRequest, ChatProposeResponse, ChatResponseKind,
    CreateStep, ProposedCaseDraft, ProposedRequirementDraft, Requirement, TestCase, TestCaseDetail,
};
use crate::services::case_keys::allocate_case_key;
use crate::services::cases::{replace_case_tags, replace_steps, resolve_tag_ids, snapshot_case};

const SYSTEM_PROMPT: &str = r#"You are Vulyk's test-management assistant. You help propose test cases and requirements.

When the user wants to CREATE cases or requirements, respond with ONLY a JSON object (no markdown) using one of:

1) Propose cases:
{"type":"propose_cases","placement":{"folder_id":null,"folder_path":"Unfiled","new_folder_name":null,"reason":"..."},"cases":[{"title":"...","description":"...","precondition":"...","priority":"medium","tags":[],"steps":[{"action":"...","expected_result":"..."}]}],"links":[]}

2) Propose requirements:
{"type":"propose_requirements","placement":{"folder_id":null,"folder_path":"Unfiled","new_folder_name":null,"reason":"..."},"requirements":[{"title":"...","description":"...","acceptance_criteria":"..."}],"cases":[],"links":[]}

3) Ambiguous placement — ask instead of guessing:
{"type":"clarify","clarify_question":"..."}

4) Ordinary Q&A (no create):
{"type":"answer","answer":"..."}

Tools you may reason about (already executed server-side and included in context):
- list_folders: suite/requirement folder tree
- search_cases: similar cases by keyword
- propose_test_cases / propose_requirements: emit structured drafts

Prefer an existing folder when similarity is clear. If nothing fits, set new_folder_name. If two folders are equally plausible, use clarify.
"#;

pub async fn run_chat_propose(
    pool: &PgPool,
    project_id: Uuid,
    provider: &dyn LlmProvider,
    model_name: &str,
    body: &ChatProposeRequest,
) -> AppResult<ChatProposeResponse> {
    let folders_ctx = list_folders_context(pool, project_id).await?;
    let search_ctx = search_cases_context(pool, project_id, &body.message).await?;
    let req_ctx = search_requirements_context(pool, project_id, &body.message).await?;

    let mut history = String::new();
    for m in &body.history {
        history.push_str(&format!("{}: {}\n", m.role, m.content));
    }

    let user = format!(
        "Current project_id={project_id}\n\n## Folders\n{folders_ctx}\n\n## Similar cases\n{search_ctx}\n\n## Similar requirements\n{req_ctx}\n\n## Conversation\n{history}\nuser: {}\n\nRespond with the JSON object only.",
        body.message.trim()
    );

    // Heuristic for mock / offline: draft a proposal when intent is clear.
    if provider.id() == "mock" {
        return Ok(mock_propose(
            project_id,
            &body.message,
            model_name,
            &folders_ctx,
        ));
    }

    let resp = provider
        .complete(LlmRequest {
            system: SYSTEM_PROMPT.into(),
            user,
            max_tokens: 4096,
        })
        .await?;

    let envelope = parse_envelope(&resp.text)?;
    Ok(envelope_to_response(
        project_id,
        &resp.provider,
        &resp.model,
        envelope,
        &folders_ctx,
    ))
}

fn mock_propose(
    project_id: Uuid,
    message: &str,
    model: &str,
    folders_ctx: &str,
) -> ChatProposeResponse {
    let lower = message.to_lowercase();
    let wants_req = lower.contains("requirement") || lower.contains("вимог");
    let wants_create = lower.contains("create")
        || lower.contains("add ")
        || lower.contains("draft")
        || lower.contains("напиши")
        || lower.contains("створ");

    if !wants_create && !wants_req {
        return ChatProposeResponse {
            provider: "mock".into(),
            model: model.to_string(),
            kind: ChatResponseKind::Answer,
            answer: Some(format!(
                "Mock Q&A: I can draft cases or requirements from your prompt. Folders:\n{folders_ctx}"
            )),
            clarify_question: None,
            proposal: None,
            confirmation_prompt: None,
        };
    }

    // Ambiguous if message asks about two domains without preference.
    if lower.contains("or ") && lower.contains("folder") {
        return ChatProposeResponse {
            provider: "mock".into(),
            model: model.to_string(),
            kind: ChatResponseKind::Clarify,
            answer: None,
            clarify_question: Some(
                "Which folder should I use for these items? Reply with a folder name or say create a new one.".into(),
            ),
            proposal: None,
            confirmation_prompt: None,
        };
    }

    if wants_req && !lower.contains("case") && !lower.contains("test") {
        let proposal = ChatProposal {
            placement: ChatPlacement {
                project_id,
                folder_id: None,
                folder_path: "Unfiled".into(),
                new_folder_name: None,
                reason: "No requirement folder matched strongly; placing at root.".into(),
            },
            cases: vec![],
            requirements: vec![ProposedRequirementDraft {
                title: truncate_title(message, "New requirement"),
                description: message.to_string(),
                acceptance_criteria: "Given the described behaviour, acceptance checks pass."
                    .into(),
            }],
            links: vec![],
        };
        return ChatProposeResponse {
            provider: "mock".into(),
            model: model.to_string(),
            kind: ChatResponseKind::ProposeRequirements,
            answer: None,
            clarify_question: None,
            confirmation_prompt: Some(confirm_prompt(&proposal)),
            proposal: Some(proposal),
        };
    }

    let proposal = ChatProposal {
        placement: ChatPlacement {
            project_id,
            folder_id: None,
            folder_path: "Unfiled".into(),
            new_folder_name: None,
            reason: "Closest match is project root until a suite is chosen.".into(),
        },
        cases: vec![ProposedCaseDraft {
            title: truncate_title(message, "Generated case"),
            description: message.to_string(),
            precondition: String::new(),
            priority: "medium".into(),
            tags: vec!["chat".into()],
            steps: vec![
                CreateStep {
                    action: "Execute the described scenario".into(),
                    expected_result: "Expected outcome observed".into(),
                    data: String::new(),
                },
                CreateStep {
                    action: "Verify side effects".into(),
                    expected_result: "System state matches acceptance".into(),
                    data: String::new(),
                },
            ],
        }],
        requirements: vec![],
        links: vec![],
    };
    ChatProposeResponse {
        provider: "mock".into(),
        model: model.to_string(),
        kind: ChatResponseKind::ProposeCases,
        answer: None,
        clarify_question: None,
        confirmation_prompt: Some(confirm_prompt(&proposal)),
        proposal: Some(proposal),
    }
}

fn truncate_title(message: &str, fallback: &str) -> String {
    let t = message.trim();
    if t.is_empty() {
        return fallback.into();
    }
    let one_line = t.lines().next().unwrap_or(t).trim();
    if one_line.chars().count() <= 80 {
        one_line.to_string()
    } else {
        format!("{}…", one_line.chars().take(77).collect::<String>())
    }
}

fn confirm_prompt(p: &ChatProposal) -> String {
    format!(
        "Is this the right project and folder? Proposed path: {}{}. {}",
        p.placement.folder_path,
        p.placement
            .new_folder_name
            .as_ref()
            .map(|n| format!(" → new «{n}»"))
            .unwrap_or_default(),
        p.placement.reason
    )
}

fn parse_envelope(text: &str) -> AppResult<ChatJsonEnvelope> {
    let trimmed = text.trim();
    let json_slice = extract_json_object(trimmed).unwrap_or(trimmed);
    serde_json::from_str(json_slice).map_err(|e| {
        AppError::BadRequest(format!(
            "LLM did not return valid chat JSON ({e}). Raw: {trimmed}"
        ))
    })
}

fn extract_json_object(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if end > start {
        Some(&text[start..=end])
    } else {
        None
    }
}

fn envelope_to_response(
    project_id: Uuid,
    provider: &str,
    model: &str,
    env: ChatJsonEnvelope,
    _folders_ctx: &str,
) -> ChatProposeResponse {
    let kind_raw = env.kind.to_lowercase();
    if kind_raw.contains("clarify") {
        return ChatProposeResponse {
            provider: provider.into(),
            model: model.into(),
            kind: ChatResponseKind::Clarify,
            answer: None,
            clarify_question: env
                .clarify_question
                .or(env.question)
                .or(env.answer)
                .or_else(|| Some("Which folder should I use?".into())),
            proposal: None,
            confirmation_prompt: None,
        };
    }
    if kind_raw.contains("answer") && env.cases.is_empty() && env.requirements.is_empty() {
        return ChatProposeResponse {
            provider: provider.into(),
            model: model.into(),
            kind: ChatResponseKind::Answer,
            answer: env.answer.or(env.question),
            clarify_question: None,
            proposal: None,
            confirmation_prompt: None,
        };
    }

    let placement = placement_from_envelope(project_id, &env);
    let proposal = ChatProposal {
        placement,
        cases: env.cases,
        requirements: env.requirements,
        links: env.links,
    };
    let kind = if !proposal.cases.is_empty()
        || (!kind_raw.contains("requirement") && proposal.requirements.is_empty())
    {
        ChatResponseKind::ProposeCases
    } else {
        ChatResponseKind::ProposeRequirements
    };
    let confirmation_prompt = Some(confirm_prompt(&proposal));
    ChatProposeResponse {
        provider: provider.into(),
        model: model.into(),
        kind,
        answer: None,
        clarify_question: None,
        proposal: Some(proposal),
        confirmation_prompt,
    }
}

fn placement_from_envelope(project_id: Uuid, env: &ChatJsonEnvelope) -> ChatPlacement {
    if let Some(Value::Object(map)) = &env.placement {
        let folder_id = map
            .get("folder_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .or(env.folder_id);
        let folder_path = map
            .get("folder_path")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .or_else(|| env.folder_path.clone())
            .unwrap_or_else(|| "Unfiled".into());
        let new_folder_name = map
            .get("new_folder_name")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .or_else(|| env.new_folder_name.clone());
        let reason = map
            .get("reason")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .or_else(|| env.reason.clone())
            .unwrap_or_else(|| "Model-suggested placement".into());
        return ChatPlacement {
            project_id,
            folder_id,
            folder_path,
            new_folder_name,
            reason,
        };
    }
    ChatPlacement {
        project_id,
        folder_id: env.folder_id,
        folder_path: env.folder_path.clone().unwrap_or_else(|| "Unfiled".into()),
        new_folder_name: env.new_folder_name.clone(),
        reason: env
            .reason
            .clone()
            .unwrap_or_else(|| "Model-suggested placement".into()),
    }
}

async fn list_folders_context(pool: &PgPool, project_id: Uuid) -> AppResult<String> {
    let suites: Vec<(Uuid, Option<Uuid>, String)> = sqlx::query_as(
        r#"SELECT id, parent_id, name FROM suites WHERE project_id = $1 ORDER BY sort_order, name"#,
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    let req_folders: Vec<(Uuid, Option<Uuid>, String)> = sqlx::query_as(
        r#"SELECT id, parent_id, name FROM requirement_folders WHERE project_id = $1 ORDER BY sort_order, name"#,
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;

    let mut out = String::from("Case suites:\n");
    if suites.is_empty() {
        out.push_str("  (none — Unfiled/root)\n");
    }
    for (id, parent, name) in &suites {
        out.push_str(&format!(
            "  - {name} (id={id}, parent={})\n",
            parent
                .map(|p| p.to_string())
                .unwrap_or_else(|| "null".into())
        ));
    }
    out.push_str("Requirement folders:\n");
    if req_folders.is_empty() {
        out.push_str("  (none — Unfiled/root)\n");
    }
    for (id, parent, name) in &req_folders {
        out.push_str(&format!(
            "  - {name} (id={id}, parent={})\n",
            parent
                .map(|p| p.to_string())
                .unwrap_or_else(|| "null".into())
        ));
    }
    Ok(out)
}

async fn search_cases_context(pool: &PgPool, project_id: Uuid, q: &str) -> AppResult<String> {
    let pattern = format!("%{}%", q.chars().take(64).collect::<String>());
    let rows: Vec<TestCase> = sqlx::query_as(
        r#"SELECT id, project_id, suite_id, key, title, description, precondition,
                  priority, status, version, external_key, sort_order, created_at, updated_at
           FROM test_cases
           WHERE project_id = $1
             AND (title ILIKE $2 OR description ILIKE $2 OR key ILIKE $2)
           ORDER BY updated_at DESC
           LIMIT 12"#,
    )
    .bind(project_id)
    .bind(&pattern)
    .fetch_all(pool)
    .await?;
    if rows.is_empty() {
        return Ok("(no similar cases)".into());
    }
    let mut out = String::new();
    for c in rows {
        out.push_str(&format!(
            "- {} | {} | suite={:?}\n",
            c.key, c.title, c.suite_id
        ));
    }
    Ok(out)
}

async fn search_requirements_context(
    pool: &PgPool,
    project_id: Uuid,
    q: &str,
) -> AppResult<String> {
    let pattern = format!("%{}%", q.chars().take(64).collect::<String>());
    let rows: Vec<(String, Option<Uuid>)> = sqlx::query_as(
        r#"SELECT title, folder_id FROM requirements
           WHERE project_id = $1 AND (title ILIKE $2 OR description ILIKE $2)
           ORDER BY updated_at DESC LIMIT 12"#,
    )
    .bind(project_id)
    .bind(&pattern)
    .fetch_all(pool)
    .await?;
    if rows.is_empty() {
        return Ok("(no similar requirements)".into());
    }
    let mut out = String::new();
    for (title, folder) in rows {
        out.push_str(&format!("- {title} | folder={folder:?}\n"));
    }
    Ok(out)
}

pub async fn confirm_proposal(
    pool: &PgPool,
    user_id: Option<Uuid>,
    body: ChatConfirmRequest,
) -> AppResult<ChatConfirmResponse> {
    let project_id = body.proposal.placement.project_id;
    let provider = if body.provider.is_empty() {
        "unknown"
    } else {
        body.provider.as_str()
    };
    let model = if body.model.is_empty() {
        "unknown"
    } else {
        body.model.as_str()
    };

    let mut tx = pool.begin().await?;
    let mut created_folder_id = None;

    // Resolve folder: create suite or requirement folder if requested.
    let mut case_suite_id = body.proposal.placement.folder_id;
    let mut req_folder_id = body.proposal.placement.folder_id;

    if let Some(name) = body
        .proposal
        .placement
        .new_folder_name
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        if !body.proposal.cases.is_empty() {
            let id: Uuid = sqlx::query_scalar(
                r#"INSERT INTO suites (project_id, parent_id, name, sort_order)
                   VALUES ($1, $2, $3, 0) RETURNING id"#,
            )
            .bind(project_id)
            .bind(body.proposal.placement.folder_id)
            .bind(name)
            .fetch_one(&mut *tx)
            .await?;
            case_suite_id = Some(id);
            created_folder_id = Some(id);
            record_event(
                &mut tx,
                project_id,
                user_id,
                provider,
                model,
                "suite",
                Some(id),
                None,
                &format!("Created suite «{name}» via chat"),
            )
            .await?;
        } else if !body.proposal.requirements.is_empty() {
            let id: Uuid = sqlx::query_scalar(
                r#"INSERT INTO requirement_folders (project_id, parent_id, name, sort_order)
                   VALUES ($1, $2, $3, 0) RETURNING id"#,
            )
            .bind(project_id)
            .bind(body.proposal.placement.folder_id)
            .bind(name)
            .fetch_one(&mut *tx)
            .await?;
            req_folder_id = Some(id);
            created_folder_id = Some(id);
            record_event(
                &mut tx,
                project_id,
                user_id,
                provider,
                model,
                "requirement_folder",
                Some(id),
                None,
                &format!("Created requirement folder «{name}» via chat"),
            )
            .await?;
        }
    }

    let mut cases_out = Vec::new();
    for draft in &body.proposal.cases {
        if draft.title.trim().is_empty() {
            return Err(AppError::BadRequest("case title required".into()));
        }
        let (key, _) = allocate_case_key(&mut tx, project_id).await?;
        let case_id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO test_cases
               (project_id, suite_id, key, title, description, precondition, priority, status, version)
               VALUES ($1, $2, $3, $4, $5, $6, $7, 'draft', 1)
               RETURNING id"#,
        )
        .bind(project_id)
        .bind(case_suite_id)
        .bind(&key)
        .bind(draft.title.trim())
        .bind(&draft.description)
        .bind(&draft.precondition)
        .bind(&draft.priority)
        .fetch_one(&mut *tx)
        .await?;

        let steps = if draft.steps.is_empty() {
            vec![CreateStep {
                action: "Execute scenario".into(),
                expected_result: "Expected result".into(),
                data: String::new(),
            }]
        } else {
            draft.steps.clone()
        };
        replace_steps(&mut tx, case_id, &steps).await?;
        let tag_ids = resolve_tag_ids(&mut tx, project_id, &[], &draft.tags).await?;
        replace_case_tags(&mut tx, case_id, &tag_ids).await?;
        let detail = load_case_detail_tx(&mut tx, project_id, case_id).await?;
        snapshot_case(&mut tx, &detail.case, "created via chat").await?;

        record_event(
            &mut tx,
            project_id,
            user_id,
            provider,
            model,
            "case",
            Some(case_id),
            Some(&key),
            &format!("Created case {key} via chat ({provider}/{model})"),
        )
        .await?;

        cases_out.push(detail);
    }

    let mut reqs_out = Vec::new();
    for draft in &body.proposal.requirements {
        if draft.title.trim().is_empty() {
            return Err(AppError::BadRequest("requirement title required".into()));
        }
        let row = sqlx::query_as::<_, Requirement>(
            r#"INSERT INTO requirements
               (project_id, folder_id, title, description, acceptance_criteria, source, url, metadata)
               VALUES ($1, $2, $3, $4, $5, 'manual', '', '{}'::jsonb)
               RETURNING id, project_id, folder_id, external_key, external_id, title, description,
                         acceptance_criteria, source, url, metadata, created_at, updated_at"#,
        )
        .bind(project_id)
        .bind(req_folder_id)
        .bind(draft.title.trim())
        .bind(&draft.description)
        .bind(&draft.acceptance_criteria)
        .fetch_one(&mut *tx)
        .await?;
        record_event(
            &mut tx,
            project_id,
            user_id,
            provider,
            model,
            "requirement",
            Some(row.id),
            None,
            &format!(
                "Created requirement «{}» via chat ({provider}/{model})",
                row.title
            ),
        )
        .await?;
        reqs_out.push(row);
    }

    let mut links_created = 0u32;
    for link in &body.proposal.links {
        let Some(detail) = cases_out.get(link.case_index as usize) else {
            continue;
        };
        let Some(req) = reqs_out.get(link.requirement_index as usize) else {
            continue;
        };
        sqlx::query(
            r#"INSERT INTO case_requirements (case_id, requirement_id)
               VALUES ($1, $2)
               ON CONFLICT DO NOTHING"#,
        )
        .bind(detail.case.id)
        .bind(req.id)
        .execute(&mut *tx)
        .await?;
        links_created += 1;
        record_event(
            &mut tx,
            project_id,
            user_id,
            provider,
            model,
            "case_requirement_link",
            Some(detail.case.id),
            Some(&detail.case.key),
            &format!("Linked {} → requirement {}", detail.case.key, req.title),
        )
        .await?;
    }

    let events: Vec<ChatCreationEventPublic> = sqlx::query_as(
        r#"SELECT id, project_id, provider, model, kind, entity_id, entity_key, summary, created_at
           FROM chat_creation_events
           WHERE project_id = $1
           ORDER BY created_at DESC
           LIMIT 50"#,
    )
    .bind(project_id)
    .fetch_all(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(ChatConfirmResponse {
        cases: cases_out,
        requirements: reqs_out,
        created_folder_id,
        links_created,
        events,
    })
}

#[allow(clippy::too_many_arguments)]
async fn record_event(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    project_id: Uuid,
    user_id: Option<Uuid>,
    provider: &str,
    model: &str,
    kind: &str,
    entity_id: Option<Uuid>,
    entity_key: Option<&str>,
    summary: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO chat_creation_events
           (project_id, user_id, provider, model, kind, entity_id, entity_key, summary)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"#,
    )
    .bind(project_id)
    .bind(user_id)
    .bind(provider)
    .bind(model)
    .bind(kind)
    .bind(entity_id)
    .bind(entity_key)
    .bind(summary)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn load_case_detail_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    project_id: Uuid,
    case_id: Uuid,
) -> AppResult<TestCaseDetail> {
    let case: TestCase = sqlx::query_as(
        r#"SELECT id, project_id, suite_id, key, title, description, precondition,
                  priority, status, version, external_key, sort_order, created_at, updated_at
           FROM test_cases WHERE id = $1 AND project_id = $2"#,
    )
    .bind(case_id)
    .bind(project_id)
    .fetch_one(&mut **tx)
    .await?;
    let steps = sqlx::query_as(
        r#"SELECT id, case_id, position, action, expected_result, data
           FROM test_steps WHERE case_id = $1 ORDER BY position"#,
    )
    .bind(case_id)
    .fetch_all(&mut **tx)
    .await?;
    let tags = sqlx::query_as(
        r#"SELECT t.id, t.project_id, t.name, t.color, t.created_at
           FROM tags t
           JOIN case_tags ct ON ct.tag_id = t.id
           WHERE ct.case_id = $1
           ORDER BY t.name"#,
    )
    .bind(case_id)
    .fetch_all(&mut **tx)
    .await?;
    Ok(TestCaseDetail {
        case,
        steps,
        tags,
        custom_fields: vec![],
    })
}

/// Tool schema documentation for OpenAPI / clients.
pub fn tool_schemas() -> Value {
    json!([
        {
            "name": "list_folders",
            "description": "List case suites and requirement folders for the project",
            "parameters": { "type": "object", "properties": {} }
        },
        {
            "name": "search_cases",
            "description": "Search cases by key/title/description",
            "parameters": {
                "type": "object",
                "properties": { "query": { "type": "string" } },
                "required": ["query"]
            }
        },
        {
            "name": "propose_test_cases",
            "description": "Propose one or more test cases with placement",
            "parameters": { "type": "object" }
        },
        {
            "name": "propose_requirements",
            "description": "Propose one or more requirements with placement",
            "parameters": { "type": "object" }
        }
    ])
}
