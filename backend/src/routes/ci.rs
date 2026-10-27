use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use crate::auth::{require_permission, OptionalAuthUser, Permission};
use crate::error::{AppError, AppResult};
use crate::integrations::{github, gitlab};
use crate::models::{
    merge_ci_config, AutomationRepoRow, CiConfig, CiConfigPublic, CiDispatch, CreateCiConfig,
    CreateRun, DispatchCiRequest, RunInCiRequest, RunInCiResponse, TestRun, UpdateCiConfig,
};
use crate::routes::automation_repos::decrypt_actions_token;
use crate::routes::AppState;
use crate::services::automation_repo as repo_svc;
use crate::services::automation_repo_remote::{parse_remote, HostKind};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/projects/{project_id}/ci-configs",
            get(list_ci_configs).post(create_ci_config),
        )
        .route(
            "/api/v1/ci-configs/{id}",
            get(get_ci_config)
                .put(update_ci_config)
                .delete(delete_ci_config),
        )
        .route(
            "/api/v1/ci-configs/{id}/dispatch",
            axum::routing::post(dispatch_ci),
        )
        .route(
            "/api/v1/projects/{project_id}/run-in-ci",
            axum::routing::post(run_in_ci),
        )
        .route(
            "/api/v1/projects/{project_id}/ci-dispatches",
            get(list_dispatches),
        )
        .route("/api/v1/ci-dispatches/{id}", get(get_dispatch))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/ci-configs",
    tag = "ci",
    params(("project_id" = Uuid, Path, description = "Project id")),
    responses((status = 200, description = "CI configs", body = [CiConfigPublic])),
    security(("bearer_auth" = []))
)]
pub async fn list_ci_configs(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
) -> AppResult<Json<Vec<CiConfigPublic>>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    ensure_project(&state, project_id).await?;
    let rows = sqlx::query_as::<_, CiConfig>(
        r#"SELECT id, project_id, provider, name, config, webhook_secret, enabled,
                  created_at, updated_at
           FROM ci_configs WHERE project_id = $1 ORDER BY name"#,
    )
    .bind(project_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows.into_iter().map(CiConfigPublic::from).collect()))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/ci-configs",
    tag = "ci",
    params(("project_id" = Uuid, Path, description = "Project id")),
    request_body = CreateCiConfig,
    responses((status = 200, description = "Created", body = CiConfigPublic)),
    security(("bearer_auth" = []))
)]
pub async fn create_ci_config(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<CreateCiConfig>,
) -> AppResult<Json<CiConfigPublic>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ManageCiConfig,
    )
    .await?;
    ensure_project(&state, project_id).await?;
    let provider = body.provider.trim().to_lowercase();
    if provider != "github" && provider != "gitlab" {
        return Err(AppError::BadRequest(
            "provider must be 'github' or 'gitlab'".into(),
        ));
    }
    if body.name.trim().is_empty() {
        return Err(AppError::BadRequest("name is required".into()));
    }
    validate_config(&provider, &body.config)?;

    let row = sqlx::query_as::<_, CiConfig>(
        r#"INSERT INTO ci_configs (project_id, provider, name, config, webhook_secret, enabled)
           VALUES ($1, $2, $3, $4, $5, $6)
           RETURNING id, project_id, provider, name, config, webhook_secret, enabled,
                     created_at, updated_at"#,
    )
    .bind(project_id)
    .bind(provider)
    .bind(body.name.trim())
    .bind(body.config)
    .bind(body.webhook_secret)
    .bind(body.enabled)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(CiConfigPublic::from(row)))
}

#[utoipa::path(
    get,
    path = "/api/v1/ci-configs/{id}",
    tag = "ci",
    params(("id" = Uuid, Path, description = "CI config id")),
    responses((status = 200, description = "CI config", body = CiConfigPublic)),
    security(("bearer_auth" = []))
)]
pub async fn get_ci_config(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<CiConfigPublic>> {
    let cfg = load_config(&state, id).await?;
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        cfg.project_id,
        Permission::ViewProject,
    )
    .await?;
    Ok(Json(CiConfigPublic::from(cfg)))
}

#[utoipa::path(
    put,
    path = "/api/v1/ci-configs/{id}",
    tag = "ci",
    params(("id" = Uuid, Path, description = "CI config id")),
    request_body = UpdateCiConfig,
    responses((status = 200, description = "Updated", body = CiConfigPublic)),
    security(("bearer_auth" = []))
)]
pub async fn update_ci_config(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateCiConfig>,
) -> AppResult<Json<CiConfigPublic>> {
    let existing = load_config(&state, id).await?;
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        existing.project_id,
        Permission::ManageCiConfig,
    )
    .await?;
    let name = body.name.unwrap_or(existing.name);
    let config = match body.config {
        Some(incoming) => merge_ci_config(&existing.config, incoming),
        None => existing.config.clone(),
    };
    validate_config(&existing.provider, &config)?;
    // Empty string clears webhook secret; omit preserves.
    let webhook_secret = match body.webhook_secret {
        Some(s) if s.is_empty() => None,
        Some(s) => Some(s),
        None => existing.webhook_secret,
    };
    let enabled = body.enabled.unwrap_or(existing.enabled);

    let row = sqlx::query_as::<_, CiConfig>(
        r#"UPDATE ci_configs
           SET name = $2, config = $3, webhook_secret = $4, enabled = $5, updated_at = NOW()
           WHERE id = $1
           RETURNING id, project_id, provider, name, config, webhook_secret, enabled,
                     created_at, updated_at"#,
    )
    .bind(id)
    .bind(name)
    .bind(config)
    .bind(webhook_secret)
    .bind(enabled)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(CiConfigPublic::from(row)))
}

#[utoipa::path(
    delete,
    path = "/api/v1/ci-configs/{id}",
    tag = "ci",
    params(("id" = Uuid, Path, description = "CI config id")),
    responses((status = 204, description = "Deleted")),
    security(("bearer_auth" = []))
)]
pub async fn delete_ci_config(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<axum::http::StatusCode> {
    let existing = load_config(&state, id).await?;
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        existing.project_id,
        Permission::ManageCiConfig,
    )
    .await?;
    let result = sqlx::query("DELETE FROM ci_configs WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("ci_config".into()));
    }
    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/api/v1/ci-configs/{id}/dispatch",
    tag = "ci",
    params(("id" = Uuid, Path, description = "CI config id")),
    request_body = DispatchCiRequest,
    responses((status = 200, description = "Dispatch record", body = CiDispatch)),
    security(("bearer_auth" = []))
)]
pub async fn dispatch_ci(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<DispatchCiRequest>,
) -> AppResult<Json<CiDispatch>> {
    let cfg = load_config(&state, id).await?;
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        cfg.project_id,
        Permission::TriggerCi,
    )
    .await?;
    if !cfg.enabled {
        return Err(AppError::BadRequest("CI config is disabled".into()));
    }

    let run_id = if let Some(run_id) = body.run_id {
        let run = sqlx::query_as::<_, TestRun>(&format!(
            r#"SELECT {cols} FROM test_runs WHERE id = $1"#,
            cols = crate::models::TEST_RUN_COLS
        ))
        .bind(run_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("run".into()))?;
        if run.project_id != cfg.project_id {
            return Err(AppError::BadRequest(
                "run does not belong to this project".into(),
            ));
        }
        Some(run.id)
    } else {
        let name = body.run_name.unwrap_or_else(|| {
            format!(
                "{} dispatch {}",
                cfg.name,
                Utc::now().format("%Y-%m-%d %H:%M")
            )
        });
        let create = CreateRun {
            name,
            description: String::new(),
            status: Some("queued".into()),
            source: format!("ci:{}", cfg.provider),
            external_ref: None,
            meta: json!({ "ci_config_id": cfg.id }),
            plan_id: None,
            suite_ids: Vec::new(),
            case_ids: Vec::new(),
            case_statuses: Vec::new(),
            tag_names: Vec::new(),
            include_all_cases: false,
            start: false,
        };
        let run = sqlx::query_as::<_, TestRun>(&format!(
            r#"INSERT INTO test_runs (project_id, name, status, source, meta, lifecycle)
               VALUES ($1, $2, $3, $4, $5, 'ci')
               RETURNING {cols}"#,
            cols = crate::models::TEST_RUN_COLS
        ))
        .bind(cfg.project_id)
        .bind(create.name)
        .bind(create.status.as_deref().unwrap_or("queued"))
        .bind(create.source)
        .bind(create.meta)
        .fetch_one(&state.pool)
        .await?;
        Some(run.id)
    };

    let mut inputs = if body.inputs.is_null() {
        json!({})
    } else {
        body.inputs
    };
    // Always expose the Vulyk run id so workflows can attach via VULYK_RUN_ID.
    if let Some(rid) = run_id {
        if let Some(obj) = inputs.as_object_mut() {
            obj.entry("vulyk_run_id")
                .or_insert_with(|| json!(rid.to_string()));
        }
    }

    let dispatch = sqlx::query_as::<_, CiDispatch>(
        r#"INSERT INTO ci_dispatches (project_id, ci_config_id, run_id, status, inputs)
           VALUES ($1, $2, $3, 'queued', $4)
           RETURNING id, project_id, ci_config_id, run_id, status, external_id, external_url,
                     inputs, last_error, created_at, updated_at"#,
    )
    .bind(cfg.project_id)
    .bind(cfg.id)
    .bind(run_id)
    .bind(&inputs)
    .fetch_one(&state.pool)
    .await?;

    let handle = match cfg.provider.as_str() {
        "github" => github::dispatch(&cfg.config, &inputs).await,
        "gitlab" => gitlab::dispatch(&cfg.config, &inputs).await,
        other => Err(AppError::BadRequest(format!("unknown provider {other}"))),
    };

    let updated = match handle {
        Ok(h) => {
            sqlx::query_as::<_, CiDispatch>(
                r#"UPDATE ci_dispatches
                   SET status = $2, external_id = COALESCE($3, external_id),
                       external_url = COALESCE($4, external_url), updated_at = NOW()
                   WHERE id = $1
                   RETURNING id, project_id, ci_config_id, run_id, status, external_id, external_url,
                             inputs, last_error, created_at, updated_at"#,
            )
            .bind(dispatch.id)
            .bind(&h.status)
            .bind(&h.external_id)
            .bind(&h.external_url)
            .fetch_one(&state.pool)
            .await?
        }
        Err(err) => {
            let msg = err.to_string();
            sqlx::query_as::<_, CiDispatch>(
                r#"UPDATE ci_dispatches
                   SET status = 'failed', last_error = $2, updated_at = NOW()
                   WHERE id = $1
                   RETURNING id, project_id, ci_config_id, run_id, status, external_id, external_url,
                             inputs, last_error, created_at, updated_at"#,
            )
            .bind(dispatch.id)
            .bind(&msg)
            .fetch_one(&state.pool)
            .await?
        }
    };

    Ok(Json(updated))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/ci-dispatches",
    tag = "ci",
    params(("project_id" = Uuid, Path, description = "Project id")),
    responses((status = 200, description = "Dispatches", body = [CiDispatch])),
    security(("bearer_auth" = []))
)]
pub async fn list_dispatches(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
) -> AppResult<Json<Vec<CiDispatch>>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    ensure_project(&state, project_id).await?;
    let rows = sqlx::query_as::<_, CiDispatch>(
        r#"SELECT id, project_id, ci_config_id, run_id, status, external_id, external_url,
                  inputs, last_error, created_at, updated_at
           FROM ci_dispatches
           WHERE project_id = $1
           ORDER BY created_at DESC
           LIMIT 100"#,
    )
    .bind(project_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

#[utoipa::path(
    get,
    path = "/api/v1/ci-dispatches/{id}",
    tag = "ci",
    params(("id" = Uuid, Path, description = "Dispatch id")),
    responses((status = 200, description = "Dispatch", body = CiDispatch)),
    security(("bearer_auth" = []))
)]
pub async fn get_dispatch(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<CiDispatch>> {
    let row = sqlx::query_as::<_, CiDispatch>(
        r#"SELECT id, project_id, ci_config_id, run_id, status, external_id, external_url,
                  inputs, last_error, created_at, updated_at
           FROM ci_dispatches WHERE id = $1"#,
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("ci_dispatch".into()))?;
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        row.project_id,
        Permission::ViewProject,
    )
    .await?;
    Ok(Json(row))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/run-in-ci",
    tag = "ci",
    params(("project_id" = Uuid, Path, description = "Project id")),
    request_body = RunInCiRequest,
    responses((status = 200, body = RunInCiResponse)),
    security(("bearer_auth" = []))
)]
pub async fn run_in_ci(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<RunInCiRequest>,
) -> AppResult<Json<RunInCiResponse>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::TriggerCi,
    )
    .await?;
    ensure_project(&state, project_id).await?;

    let repo = load_repo_for_run_in_ci(&state, project_id, body.repo_id).await?;
    if repo.provider.to_lowercase() != "github" {
        return Err(AppError::BadRequest(
            "Run in CI currently supports GitHub automation repos only".into(),
        ));
    }
    let actions_token = decrypt_actions_token(&state, &repo)?.ok_or_else(|| {
        AppError::BadRequest(
            "automation repo is missing an Actions write token (Settings → automation repo)".into(),
        )
    })?;
    let workflow_file = repo.ci_workflow_file.trim();
    if workflow_file.is_empty() {
        return Err(AppError::BadRequest(
            "automation repo is missing ci_workflow_file (e.g. .github/workflows/vulyk.yml)".into(),
        ));
    }
    let workflow_ref = if repo.ci_workflow_ref.trim().is_empty() {
        "main"
    } else {
        repo.ci_workflow_ref.trim()
    };

    let HostKind::GitHub {
        api_base,
        owner,
        repo: gh_repo,
    } = parse_remote("github", &repo.remote_url)?
    else {
        return Err(AppError::BadRequest(
            "could not parse GitHub owner/repo from remote URL".into(),
        ));
    };

    let case_keys = resolve_case_keys(&state, project_id, &body).await?;
    if case_keys.is_empty() {
        return Err(AppError::BadRequest(
            "select at least one case (case_ids, case_keys, or source_run_id)".into(),
        ));
    }

    let run_name = body
        .run_name
        .unwrap_or_else(|| format!("CI {}", Utc::now().format("%Y-%m-%d %H:%M")));
    let run = sqlx::query_as::<_, TestRun>(&format!(
        r#"INSERT INTO test_runs
            (project_id, name, status, source, meta, lifecycle, ci_provider, ci_repo, suite_name)
           VALUES ($1, $2, 'queued', 'ci:github', $3, 'ci', 'github', $4, $2)
           RETURNING {cols}"#,
        cols = crate::models::TEST_RUN_COLS
    ))
    .bind(project_id)
    .bind(&run_name)
    .bind(json!({
        "automation_repo_id": repo.id,
        "workflow_file": workflow_file,
        "workflow_ref": workflow_ref,
        "case_keys": case_keys,
    }))
    .bind(format!("{owner}/{gh_repo}"))
    .fetch_one(&state.pool)
    .await?;

    // Attach run_items for selected cases (lookup by key).
    for key in &case_keys {
        let case_id: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT id FROM test_cases WHERE project_id = $1 AND upper(key) = upper($2)"#,
        )
        .bind(project_id)
        .bind(key)
        .fetch_optional(&state.pool)
        .await?;
        if let Some(cid) = case_id {
            let _ = sqlx::query(
                r#"INSERT INTO run_items (run_id, case_id, status)
                   VALUES ($1, $2, 'untested')
                   ON CONFLICT (run_id, case_id) DO NOTHING"#,
            )
            .bind(run.id)
            .bind(cid)
            .execute(&state.pool)
            .await;
        }
    }

    let cfg = upsert_github_dispatch_config(
        &state,
        project_id,
        &repo,
        &api_base,
        &owner,
        &gh_repo,
        workflow_file,
        workflow_ref,
        &actions_token,
    )
    .await?;

    let inputs = json!({
        "vulyk_run_id": run.id.to_string(),
        "case_keys": case_keys.join(","),
    });

    let dispatch = sqlx::query_as::<_, CiDispatch>(
        r#"INSERT INTO ci_dispatches (project_id, ci_config_id, run_id, status, inputs)
           VALUES ($1, $2, $3, 'queued', $4)
           RETURNING id, project_id, ci_config_id, run_id, status, external_id, external_url,
                     inputs, last_error, created_at, updated_at"#,
    )
    .bind(project_id)
    .bind(cfg.id)
    .bind(run.id)
    .bind(&inputs)
    .fetch_one(&state.pool)
    .await?;

    let updated = match github::dispatch(&cfg.config, &inputs).await {
        Ok(h) => {
            let d = sqlx::query_as::<_, CiDispatch>(
                r#"UPDATE ci_dispatches
                   SET status = $2, external_id = COALESCE($3, external_id),
                       external_url = COALESCE($4, external_url), updated_at = NOW()
                   WHERE id = $1
                   RETURNING id, project_id, ci_config_id, run_id, status, external_id, external_url,
                             inputs, last_error, created_at, updated_at"#,
            )
            .bind(dispatch.id)
            .bind(&h.status)
            .bind(&h.external_id)
            .bind(&h.external_url)
            .fetch_one(&state.pool)
            .await?;
            // Seed pipeline_url with the workflow page until workflow_run webhook arrives.
            if let Some(url) = &h.external_url {
                let _ = sqlx::query(
                    r#"UPDATE test_runs SET pipeline_url = COALESCE(pipeline_url, $2), updated_at = NOW()
                       WHERE id = $1"#,
                )
                .bind(run.id)
                .bind(url)
                .execute(&state.pool)
                .await;
            }
            d
        }
        Err(err) => {
            let msg = err.to_string();
            let _ = sqlx::query(
                r#"UPDATE test_runs SET status = 'failed', finished_at = NOW(), updated_at = NOW()
                   WHERE id = $1"#,
            )
            .bind(run.id)
            .execute(&state.pool)
            .await;
            sqlx::query_as::<_, CiDispatch>(
                r#"UPDATE ci_dispatches
                   SET status = 'failed', last_error = $2, updated_at = NOW()
                   WHERE id = $1
                   RETURNING id, project_id, ci_config_id, run_id, status, external_id, external_url,
                             inputs, last_error, created_at, updated_at"#,
            )
            .bind(dispatch.id)
            .bind(&msg)
            .fetch_one(&state.pool)
            .await?
        }
    };

    let run = sqlx::query_as::<_, TestRun>(&format!(
        r#"SELECT {cols} FROM test_runs WHERE id = $1"#,
        cols = crate::models::TEST_RUN_COLS
    ))
    .bind(run.id)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(RunInCiResponse {
        run,
        dispatch: updated,
        case_keys,
    }))
}

async fn load_repo_for_run_in_ci(
    state: &AppState,
    project_id: Uuid,
    repo_id: Option<Uuid>,
) -> AppResult<AutomationRepoRow> {
    if let Some(id) = repo_id {
        let row = repo_svc::load_repo_by_id(&state.pool, id)
            .await?
            .ok_or_else(|| AppError::NotFound("automation repo".into()))?;
        let links = repo_svc::project_ids_for_repo(&state.pool, id).await?;
        if !links.contains(&project_id) && row.project_id != Some(project_id) {
            return Err(AppError::BadRequest(
                "automation repo is not linked to this project".into(),
            ));
        }
        return Ok(row);
    }
    repo_svc::load_repo_for_project(&state.pool, project_id)
        .await?
        .ok_or_else(|| {
            AppError::BadRequest(
                "no automation repo linked to this project; configure one under Settings".into(),
            )
        })
}

async fn resolve_case_keys(
    state: &AppState,
    project_id: Uuid,
    body: &RunInCiRequest,
) -> AppResult<Vec<String>> {
    let mut keys: Vec<String> = body
        .case_keys
        .iter()
        .map(|k| k.trim().to_uppercase())
        .filter(|k| !k.is_empty())
        .collect();

    if !body.case_ids.is_empty() {
        let rows: Vec<String> = sqlx::query_scalar(
            r#"SELECT key FROM test_cases
               WHERE project_id = $1 AND id = ANY($2) AND key IS NOT NULL AND key <> ''"#,
        )
        .bind(project_id)
        .bind(&body.case_ids)
        .fetch_all(&state.pool)
        .await?;
        keys.extend(rows.into_iter().map(|k| k.to_uppercase()));
    }

    if let Some(source_run_id) = body.source_run_id {
        let rows: Vec<String> = sqlx::query_scalar(
            r#"SELECT DISTINCT c.key
               FROM run_items ri
               JOIN test_cases c ON c.id = ri.case_id
               JOIN test_runs r ON r.id = ri.run_id
               WHERE ri.run_id = $1 AND r.project_id = $2
                 AND c.key IS NOT NULL AND c.key <> ''"#,
        )
        .bind(source_run_id)
        .bind(project_id)
        .fetch_all(&state.pool)
        .await?;
        keys.extend(rows.into_iter().map(|k| k.to_uppercase()));
    }

    keys.sort();
    keys.dedup();
    Ok(keys)
}

#[allow(clippy::too_many_arguments)]
async fn upsert_github_dispatch_config(
    state: &AppState,
    project_id: Uuid,
    repo: &AutomationRepoRow,
    api_base: &str,
    owner: &str,
    gh_repo: &str,
    workflow_file: &str,
    workflow_ref: &str,
    actions_token: &str,
) -> AppResult<CiConfig> {
    let name = format!("vulyk-actions-{}", &repo.id.to_string()[..8]);
    let config = json!({
        "token": actions_token,
        "owner": owner,
        "repo": gh_repo,
        "workflow_id": workflow_file,
        "ref": workflow_ref,
        "api_base": api_base,
    });
    validate_config("github", &config)?;

    if let Some(existing) = sqlx::query_as::<_, CiConfig>(
        r#"SELECT id, project_id, provider, name, config, webhook_secret, enabled,
                  created_at, updated_at
           FROM ci_configs WHERE project_id = $1 AND name = $2"#,
    )
    .bind(project_id)
    .bind(&name)
    .fetch_optional(&state.pool)
    .await?
    {
        let merged = merge_ci_config(&existing.config, config);
        let row = sqlx::query_as::<_, CiConfig>(
            r#"UPDATE ci_configs
               SET config = $2, enabled = TRUE, updated_at = NOW()
               WHERE id = $1
               RETURNING id, project_id, provider, name, config, webhook_secret, enabled,
                         created_at, updated_at"#,
        )
        .bind(existing.id)
        .bind(merged)
        .fetch_one(&state.pool)
        .await?;
        return Ok(row);
    }

    let row = sqlx::query_as::<_, CiConfig>(
        r#"INSERT INTO ci_configs (project_id, provider, name, config, enabled)
           VALUES ($1, 'github', $2, $3, TRUE)
           RETURNING id, project_id, provider, name, config, webhook_secret, enabled,
                     created_at, updated_at"#,
    )
    .bind(project_id)
    .bind(&name)
    .bind(config)
    .fetch_one(&state.pool)
    .await?;
    Ok(row)
}

async fn load_config(state: &AppState, id: Uuid) -> AppResult<CiConfig> {
    sqlx::query_as::<_, CiConfig>(
        r#"SELECT id, project_id, provider, name, config, webhook_secret, enabled,
                  created_at, updated_at
           FROM ci_configs WHERE id = $1"#,
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("ci_config".into()))
}

async fn ensure_project(state: &AppState, project_id: Uuid) -> AppResult<()> {
    let found: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM projects WHERE id = $1")
        .bind(project_id)
        .fetch_optional(&state.pool)
        .await?;
    if found.is_none() {
        return Err(AppError::NotFound("project".into()));
    }
    Ok(())
}

fn validate_config(provider: &str, config: &serde_json::Value) -> AppResult<()> {
    match provider {
        "github" => {
            for key in ["token", "owner", "repo", "workflow_id"] {
                if config
                    .get(key)
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .is_empty()
                {
                    return Err(AppError::BadRequest(format!(
                        "github config requires '{key}'"
                    )));
                }
            }
        }
        "gitlab" => {
            for key in ["project_id", "trigger_token"] {
                if config
                    .get(key)
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .is_empty()
                {
                    return Err(AppError::BadRequest(format!(
                        "gitlab config requires '{key}'"
                    )));
                }
            }
        }
        _ => {}
    }
    Ok(())
}
