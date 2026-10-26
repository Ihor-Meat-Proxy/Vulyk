//! LLM config admin + generate cases / automation / Q&A (W9).

use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use serde_json::Value;
use uuid::Uuid;

use crate::auth::{require_permission, OptionalAuthUser, Permission};
use crate::error::{AppError, AppResult};
use crate::llm::{build_provider, LlmRequest, ProviderConfig, ProviderKind};
use crate::models::{
    AskTestsRequest, AskTestsResponse, CreateCase, CreateLlmConfig, CreateStep,
    GenerateCasesRequest, GenerateCasesResponse, GenerateCodeRequest, GenerateCodeResponse,
    GeneratedCaseDraft, LlmConfigPublic, LlmConfigRow, TestCase, TestCaseDetail, UpdateLlmConfig,
};
use crate::routes::cases::{ensure_project, load_steps, replace_steps};
use crate::routes::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/projects/{project_id}/llm/configs",
            get(list_configs).post(create_config),
        )
        .route(
            "/api/v1/projects/{project_id}/llm/configs/{config_id}",
            get(get_config).put(update_config).delete(delete_config),
        )
        .route(
            "/api/v1/projects/{project_id}/llm/generate-cases",
            axum::routing::post(generate_cases),
        )
        .route(
            "/api/v1/projects/{project_id}/llm/generate-code",
            axum::routing::post(generate_code),
        )
        .route(
            "/api/v1/projects/{project_id}/llm/ask",
            axum::routing::post(ask_tests),
        )
        .route("/api/v1/llm/providers", get(list_provider_kinds))
}

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct ProviderKindInfo {
    pub id: String,
    pub name: String,
    pub default_base_url: Option<String>,
    pub requires_api_key: bool,
}

#[utoipa::path(
    get,
    path = "/api/v1/llm/providers",
    tag = "llm",
    responses((status = 200, body = [ProviderKindInfo])),
    security(("bearer_auth" = []))
)]
pub async fn list_provider_kinds(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
) -> AppResult<Json<Vec<ProviderKindInfo>>> {
    if state.auth.required && auth.is_none() {
        return Err(AppError::Unauthorized("authentication required".into()));
    }
    Ok(Json(vec![
        info(ProviderKind::Ollama, "Ollama (local)", false),
        info(
            ProviderKind::OpenaiCompatible,
            "Unsloth / OpenAI-compatible local",
            false,
        ),
        info(ProviderKind::Openrouter, "OpenRouter", true),
        info(ProviderKind::Anthropic, "Anthropic Claude", true),
        info(ProviderKind::Gemini, "Google Gemini", true),
        info(ProviderKind::Openai, "OpenAI", true),
        info(ProviderKind::Xai, "xAI Grok", true),
        info(ProviderKind::Mock, "Mock (tests / offline)", false),
    ]))
}

fn info(kind: ProviderKind, name: &str, requires_api_key: bool) -> ProviderKindInfo {
    ProviderKindInfo {
        id: kind.as_str().into(),
        name: name.into(),
        default_base_url: kind.default_base_url().map(str::to_string),
        requires_api_key,
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/llm/configs",
    tag = "llm",
    params(("project_id" = Uuid, Path, description = "Project id")),
    responses((status = 200, body = [LlmConfigPublic])),
    security(("bearer_auth" = []))
)]
pub async fn list_configs(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
) -> AppResult<Json<Vec<LlmConfigPublic>>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    ensure_project(&state.pool, project_id).await?;
    let rows = sqlx::query_as::<_, LlmConfigRow>(
        r#"SELECT id, project_id, provider, name, base_url, model,
                  api_key_ciphertext, api_key_nonce, extra, is_default, created_at, updated_at
           FROM llm_configs
           WHERE project_id = $1 OR project_id IS NULL
           ORDER BY is_default DESC, name"#,
    )
    .bind(project_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows.iter().map(LlmConfigRow::to_public).collect()))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/llm/configs",
    tag = "llm",
    request_body = CreateLlmConfig,
    params(("project_id" = Uuid, Path, description = "Project id")),
    responses((status = 200, body = LlmConfigPublic)),
    security(("bearer_auth" = []))
)]
pub async fn create_config(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<CreateLlmConfig>,
) -> AppResult<Json<LlmConfigPublic>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ManageLlmConfig,
    )
    .await?;
    ensure_project(&state.pool, project_id).await?;
    let kind = ProviderKind::parse(&body.provider)?;
    if body.name.trim().is_empty() {
        return Err(AppError::BadRequest("name required".into()));
    }

    let (ct, nonce) = match body
        .api_key
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(key) => {
            let (c, n) = state.secrets.encrypt(key)?;
            (Some(c), Some(n))
        }
        None => (None, None),
    };

    let scoped_project_id: Option<Uuid> = if body.global { None } else { Some(project_id) };

    if body.is_default {
        clear_default(&state.pool, scoped_project_id).await?;
    }

    let row = sqlx::query_as::<_, LlmConfigRow>(
        r#"INSERT INTO llm_configs
            (project_id, provider, name, base_url, model, api_key_ciphertext, api_key_nonce, extra, is_default)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
           RETURNING id, project_id, provider, name, base_url, model,
                     api_key_ciphertext, api_key_nonce, extra, is_default, created_at, updated_at"#,
    )
    .bind(scoped_project_id)
    .bind(kind.as_str())
    .bind(body.name.trim())
    .bind(&body.base_url)
    .bind(&body.model)
    .bind(ct)
    .bind(nonce)
    .bind(&body.extra)
    .bind(body.is_default)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(row.to_public()))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/llm/configs/{config_id}",
    tag = "llm",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("config_id" = Uuid, Path, description = "Config id")
    ),
    responses((status = 200, body = LlmConfigPublic)),
    security(("bearer_auth" = []))
)]
pub async fn get_config(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, config_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<LlmConfigPublic>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    let row = load_config_row(&state.pool, project_id, config_id).await?;
    Ok(Json(row.to_public()))
}

#[utoipa::path(
    put,
    path = "/api/v1/projects/{project_id}/llm/configs/{config_id}",
    tag = "llm",
    request_body = UpdateLlmConfig,
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("config_id" = Uuid, Path, description = "Config id")
    ),
    responses((status = 200, body = LlmConfigPublic)),
    security(("bearer_auth" = []))
)]
pub async fn update_config(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, config_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateLlmConfig>,
) -> AppResult<Json<LlmConfigPublic>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ManageLlmConfig,
    )
    .await?;
    let existing = load_config_row(&state.pool, project_id, config_id).await?;
    let name = body.name.unwrap_or(existing.name);
    let base_url = body.base_url.or(existing.base_url);
    let model = body.model.unwrap_or(existing.model);
    let extra = body.extra.unwrap_or(existing.extra);
    let is_default = body.is_default.unwrap_or(existing.is_default);

    let (ct, nonce) = if let Some(key) = body.api_key {
        if key.is_empty() {
            (None, None)
        } else {
            let (c, n) = state.secrets.encrypt(&key)?;
            (Some(c), Some(n))
        }
    } else {
        (existing.api_key_ciphertext, existing.api_key_nonce)
    };

    if is_default {
        clear_default(&state.pool, existing.project_id).await?;
    }

    let row = sqlx::query_as::<_, LlmConfigRow>(
        r#"UPDATE llm_configs SET
             name = $1, base_url = $2, model = $3,
             api_key_ciphertext = $4, api_key_nonce = $5,
             extra = $6, is_default = $7, updated_at = NOW()
           WHERE id = $8
           RETURNING id, project_id, provider, name, base_url, model,
                     api_key_ciphertext, api_key_nonce, extra, is_default, created_at, updated_at"#,
    )
    .bind(&name)
    .bind(&base_url)
    .bind(&model)
    .bind(ct)
    .bind(nonce)
    .bind(&extra)
    .bind(is_default)
    .bind(config_id)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(row.to_public()))
}

#[utoipa::path(
    delete,
    path = "/api/v1/projects/{project_id}/llm/configs/{config_id}",
    tag = "llm",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("config_id" = Uuid, Path, description = "Config id")
    ),
    responses((status = 204, description = "Deleted")),
    security(("bearer_auth" = []))
)]
pub async fn delete_config(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, config_id)): Path<(Uuid, Uuid)>,
) -> AppResult<crate::routes::tokens::StatusNoContent> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ManageLlmConfig,
    )
    .await?;
    let _ = load_config_row(&state.pool, project_id, config_id).await?;
    sqlx::query("DELETE FROM llm_configs WHERE id = $1")
        .bind(config_id)
        .execute(&state.pool)
        .await?;
    Ok(crate::routes::tokens::StatusNoContent)
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/llm/generate-cases",
    tag = "llm",
    request_body = GenerateCasesRequest,
    params(("project_id" = Uuid, Path, description = "Project id")),
    responses((status = 200, body = GenerateCasesResponse)),
    security(("bearer_auth" = []))
)]
pub async fn generate_cases(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<GenerateCasesRequest>,
) -> AppResult<Json<GenerateCasesResponse>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::LlmAssist,
    )
    .await?;
    ensure_project(&state.pool, project_id).await?;
    let provider = resolve_provider(&state, project_id, body.config_id).await?;

    let mut req_text = body.requirements_text.clone();
    if !body.requirement_ids.is_empty() {
        let rows: Vec<(String, String)> = sqlx::query_as(
            r#"SELECT title, description FROM requirements
               WHERE project_id = $1 AND id = ANY($2)"#,
        )
        .bind(project_id)
        .bind(&body.requirement_ids)
        .fetch_all(&state.pool)
        .await?;
        for (title, desc) in rows {
            req_text.push_str(&format!("\n- {title}: {desc}"));
        }
    }
    if req_text.trim().is_empty() {
        return Err(AppError::BadRequest(
            "provide requirements_text or requirement_ids".into(),
        ));
    }

    let system = format!(
        "You are a senior QA engineer. Generate up to {} test cases as a JSON array. \
         Each object: key_suffix, title, description, precondition, priority, \
         steps:[{{action, expected_result}}]. Return ONLY JSON.",
        body.count.clamp(1, 20)
    );
    let resp = provider
        .complete(LlmRequest {
            system,
            user: req_text,
            max_tokens: 4096,
        })
        .await?;

    let drafts = parse_case_drafts(&resp.text, project_id, &state.pool).await?;
    let mut persisted = Vec::new();
    if body.persist {
        for draft in &drafts {
            let create = CreateCase {
                key: None,
                title: draft.title.clone(),
                suite_id: body.suite_id,
                description: draft.description.clone(),
                precondition: draft.precondition.clone(),
                priority: draft.priority.clone(),
                status: "draft".into(),
                steps: draft.steps.clone(),
                tag_ids: vec![],
                tag_names: vec![],
                external_key: Some(draft.key.clone()),
                sort_order: None,
            };
            persisted.push(insert_case(&state.pool, project_id, create).await?);
        }
    }

    Ok(Json(GenerateCasesResponse {
        provider: resp.provider,
        model: resp.model,
        drafts,
        persisted,
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/llm/generate-code",
    tag = "llm",
    request_body = GenerateCodeRequest,
    params(("project_id" = Uuid, Path, description = "Project id")),
    responses((status = 200, body = GenerateCodeResponse)),
    security(("bearer_auth" = []))
)]
pub async fn generate_code(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<GenerateCodeRequest>,
) -> AppResult<Json<GenerateCodeResponse>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::LlmAssist,
    )
    .await?;
    ensure_project(&state.pool, project_id).await?;
    if body.case_ids.is_empty() {
        return Err(AppError::BadRequest("case_ids required".into()));
    }
    let provider = resolve_provider(&state, project_id, body.config_id).await?;
    let framework = body.framework.to_lowercase();
    if framework != "playwright" && framework != "pytest" {
        return Err(AppError::BadRequest(
            "framework must be playwright or pytest".into(),
        ));
    }

    let mut bundle = String::new();
    let mut query_parts = Vec::new();
    for case_id in &body.case_ids {
        let case = sqlx::query_as::<_, TestCase>(
            r#"SELECT id, project_id, suite_id, key, title, description, precondition,
                      priority, status, version, external_key, sort_order, created_at, updated_at
               FROM test_cases WHERE id = $1 AND project_id = $2"#,
        )
        .bind(case_id)
        .bind(project_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("case".into()))?;
        let steps = load_steps(&state.pool, case.id).await?;
        query_parts.push(format!("{} {}", case.key, case.title));
        query_parts.push(case.description.clone());
        bundle.push_str(&format!(
            "Case {} — {}\nDescription: {}\nPrecondition: {}\nSteps:\n",
            case.key, case.title, case.description, case.precondition
        ));
        for s in steps {
            query_parts.push(s.action.clone());
            bundle.push_str(&format!(
                "  {}. {} => {}\n",
                s.position + 1,
                s.action,
                s.expected_result
            ));
        }
        bundle.push('\n');
    }

    let (context_files, repo_context) = if let Some(repo) =
        crate::services::automation_repo::load_repo_for_project(&state.pool, project_id).await?
    {
        if repo.index_status == "ready" {
            let q = format!("{framework} {}", query_parts.join(" "));
            crate::services::automation_repo::retrieve_context(&state.pool, repo.id, &q, None)
                .await?
        } else {
            (vec![], String::new())
        }
    } else {
        (vec![], String::new())
    };

    let system = format!(
        "You write test automation. Produce {framework} code for the given cases. \
         Prefer reusing existing page objects, fixtures, helpers, and import styles from the \
         repository context when provided. Return only the source code. Mark as automation generation."
    );
    let user = if repo_context.is_empty() {
        format!("FRAMEWORK: {framework}\n\n{bundle}")
    } else {
        format!(
            "FRAMEWORK: {framework}\n\n## Repository context (reuse patterns/imports)\n{repo_context}\n\n## Cases to automate\n{bundle}"
        )
    };
    let resp = provider
        .complete(LlmRequest {
            system,
            user,
            max_tokens: 4096,
        })
        .await?;

    Ok(Json(GenerateCodeResponse {
        provider: resp.provider,
        model: resp.model,
        framework,
        code: resp.text,
        context_files,
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/llm/ask",
    tag = "llm",
    request_body = AskTestsRequest,
    params(("project_id" = Uuid, Path, description = "Project id")),
    responses((status = 200, body = AskTestsResponse)),
    security(("bearer_auth" = []))
)]
pub async fn ask_tests(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<AskTestsRequest>,
) -> AppResult<Json<AskTestsResponse>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::LlmAssist,
    )
    .await?;
    ensure_project(&state.pool, project_id).await?;
    if body.question.trim().is_empty() {
        return Err(AppError::BadRequest("question required".into()));
    }
    let provider = resolve_provider(&state, project_id, body.config_id).await?;

    let cases: Vec<TestCase> = if body.case_ids.is_empty() {
        sqlx::query_as::<_, TestCase>(
            r#"SELECT id, project_id, suite_id, key, title, description, precondition,
                      priority, status, version, external_key, sort_order, created_at, updated_at
               FROM test_cases WHERE project_id = $1
               ORDER BY updated_at DESC LIMIT 40"#,
        )
        .bind(project_id)
        .fetch_all(&state.pool)
        .await?
    } else {
        sqlx::query_as::<_, TestCase>(
            r#"SELECT id, project_id, suite_id, key, title, description, precondition,
                      priority, status, version, external_key, sort_order, created_at, updated_at
               FROM test_cases WHERE project_id = $1 AND id = ANY($2)"#,
        )
        .bind(project_id)
        .bind(&body.case_ids)
        .fetch_all(&state.pool)
        .await?
    };

    let mut ctx = String::from("Project test catalogue:\n");
    for c in &cases {
        ctx.push_str(&format!(
            "- {} | {} | {} | {}\n",
            c.key, c.title, c.priority, c.status
        ));
        if !c.description.is_empty() {
            ctx.push_str(&format!("  {}\n", c.description));
        }
    }

    let resp = provider
        .complete(LlmRequest {
            system: "You are a Q&A assistant over a project's test cases. Be concise and cite case keys when relevant.".into(),
            user: format!("Question: {}\n\n{ctx}", body.question.trim()),
            max_tokens: 2048,
        })
        .await?;

    Ok(Json(AskTestsResponse {
        provider: resp.provider,
        model: resp.model,
        answer: resp.text,
    }))
}

async fn clear_default(pool: &sqlx::PgPool, project_id: Option<Uuid>) -> AppResult<()> {
    match project_id {
        Some(pid) => {
            sqlx::query("UPDATE llm_configs SET is_default = FALSE WHERE project_id = $1")
                .bind(pid)
                .execute(pool)
                .await?;
        }
        None => {
            sqlx::query("UPDATE llm_configs SET is_default = FALSE WHERE project_id IS NULL")
                .execute(pool)
                .await?;
        }
    }
    Ok(())
}

async fn load_config_row(
    pool: &sqlx::PgPool,
    project_id: Uuid,
    config_id: Uuid,
) -> AppResult<LlmConfigRow> {
    ensure_project(pool, project_id).await?;
    sqlx::query_as::<_, LlmConfigRow>(
        r#"SELECT id, project_id, provider, name, base_url, model,
                  api_key_ciphertext, api_key_nonce, extra, is_default, created_at, updated_at
           FROM llm_configs
           WHERE id = $1 AND (project_id = $2 OR project_id IS NULL)"#,
    )
    .bind(config_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("llm config".into()))
}

/// Resolve which LLM provider to use for chat / generate-cases / generate-code.
///
/// Order (when `config_id` is omitted):
/// 1. Project-specific default
/// 2. Any other project-specific config
/// 3. Global default (`project_id IS NULL`)
/// 4. Any other global config
/// 5. Built-in mock — **only** when no configs exist
///
/// `VULYK_LLM_MOCK=1` no longer overrides a configured provider (that hid real
/// OpenRouter keys behind a silent mock). It remains documented for CI as a
/// hint that mock is acceptable when nothing is configured.
pub(crate) async fn resolve_provider(
    state: &AppState,
    project_id: Uuid,
    config_id: Option<Uuid>,
) -> AppResult<Box<dyn crate::llm::LlmProvider>> {
    let row = if let Some(id) = config_id {
        load_config_row(&state.pool, project_id, id).await?
    } else {
        let found = sqlx::query_as::<_, LlmConfigRow>(
            r#"SELECT id, project_id, provider, name, base_url, model,
                      api_key_ciphertext, api_key_nonce, extra, is_default, created_at, updated_at
               FROM llm_configs
               WHERE (project_id = $1 OR project_id IS NULL)
               ORDER BY
                 CASE
                   WHEN project_id = $1 AND is_default THEN 0
                   WHEN project_id = $1 THEN 1
                   WHEN project_id IS NULL AND is_default THEN 2
                   ELSE 3
                 END,
                 created_at ASC
               LIMIT 1"#,
        )
        .bind(project_id)
        .fetch_optional(&state.pool)
        .await?;

        match found {
            Some(r) => r,
            None => {
                return Ok(Box::new(crate::llm::providers::MockProvider));
            }
        }
    };

    let kind = ProviderKind::parse(&row.provider)?;
    let api_key = match (&row.api_key_ciphertext, &row.api_key_nonce) {
        (Some(ct), Some(nonce)) => Some(state.secrets.decrypt(ct, nonce)?),
        _ => None,
    };

    build_provider(&ProviderConfig {
        kind,
        base_url: row.base_url,
        model: row.model,
        api_key,
    })
}

async fn parse_case_drafts(
    text: &str,
    project_id: Uuid,
    pool: &sqlx::PgPool,
) -> AppResult<Vec<GeneratedCaseDraft>> {
    let json_slice = extract_json_array(text).unwrap_or(text);
    let value: Value = serde_json::from_str(json_slice).unwrap_or_else(|_| {
        serde_json::json!([{
            "key_suffix": "GEN-1",
            "title": "Generated case",
            "description": text,
            "precondition": "",
            "priority": "medium",
            "steps": []
        }])
    });
    let arr = value
        .as_array()
        .cloned()
        .ok_or_else(|| AppError::BadRequest("LLM did not return a JSON array".into()))?;

    let project_key: String = sqlx::query_scalar("SELECT key FROM projects WHERE id = $1")
        .bind(project_id)
        .fetch_one(pool)
        .await?;

    let mut drafts = Vec::new();
    for (idx, item) in arr.into_iter().enumerate() {
        let suffix = item
            .get("key_suffix")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| format!("GEN-{}", idx + 1));
        let key = if suffix.contains('-') {
            suffix
        } else {
            format!("{project_key}-{suffix}")
        };
        let steps = item
            .get("steps")
            .and_then(|v| v.as_array())
            .map(|steps| {
                steps
                    .iter()
                    .map(|s| CreateStep {
                        action: s
                            .get("action")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .into(),
                        expected_result: s
                            .get("expected_result")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .into(),
                        data: s.get("data").and_then(|v| v.as_str()).unwrap_or("").into(),
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        drafts.push(GeneratedCaseDraft {
            key,
            title: item
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or("Untitled")
                .into(),
            description: item
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .into(),
            precondition: item
                .get("precondition")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .into(),
            priority: item
                .get("priority")
                .and_then(|v| v.as_str())
                .unwrap_or("medium")
                .into(),
            steps,
        });
    }
    Ok(drafts)
}

fn extract_json_array(text: &str) -> Option<&str> {
    let start = text.find('[')?;
    let end = text.rfind(']')?;
    if end > start {
        Some(&text[start..=end])
    } else {
        None
    }
}

async fn insert_case(
    pool: &sqlx::PgPool,
    project_id: Uuid,
    body: CreateCase,
) -> AppResult<TestCaseDetail> {
    use crate::services::case_keys::allocate_case_key;

    let mut tx = pool.begin().await?;
    let (key, _) = allocate_case_key(&mut tx, project_id).await?;
    let case = sqlx::query_as::<_, TestCase>(
        r#"INSERT INTO test_cases
            (project_id, suite_id, key, title, description, precondition, priority, status, external_key)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
           RETURNING id, project_id, suite_id, key, title, description, precondition,
                     priority, status, version, external_key, sort_order, created_at, updated_at"#,
    )
    .bind(project_id)
    .bind(body.suite_id)
    .bind(&key)
    .bind(&body.title)
    .bind(&body.description)
    .bind(&body.precondition)
    .bind(&body.priority)
    .bind(&body.status)
    .bind(body.external_key.as_deref())
    .fetch_one(&mut *tx)
    .await?;
    let steps = replace_steps(&mut tx, case.id, &body.steps).await?;
    tx.commit().await?;
    Ok(TestCaseDetail {
        case,
        steps,
        tags: vec![],
        custom_fields: vec![],
    })
}
