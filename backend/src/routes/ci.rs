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
    CiConfig, CiDispatch, CreateCiConfig, CreateRun, DispatchCiRequest, TestRun, UpdateCiConfig,
};
use crate::routes::AppState;

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
    responses((status = 200, description = "CI configs", body = [CiConfig])),
    security(("bearer_auth" = []))
)]
pub async fn list_ci_configs(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
) -> AppResult<Json<Vec<CiConfig>>> {
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
    Ok(Json(rows))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/ci-configs",
    tag = "ci",
    params(("project_id" = Uuid, Path, description = "Project id")),
    request_body = CreateCiConfig,
    responses((status = 200, description = "Created", body = CiConfig)),
    security(("bearer_auth" = []))
)]
pub async fn create_ci_config(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<CreateCiConfig>,
) -> AppResult<Json<CiConfig>> {
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
    Ok(Json(row))
}

#[utoipa::path(
    get,
    path = "/api/v1/ci-configs/{id}",
    tag = "ci",
    params(("id" = Uuid, Path, description = "CI config id")),
    responses((status = 200, description = "CI config", body = CiConfig)),
    security(("bearer_auth" = []))
)]
pub async fn get_ci_config(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<CiConfig>> {
    let cfg = load_config(&state, id).await?;
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        cfg.project_id,
        Permission::ViewProject,
    )
    .await?;
    Ok(Json(cfg))
}

#[utoipa::path(
    put,
    path = "/api/v1/ci-configs/{id}",
    tag = "ci",
    params(("id" = Uuid, Path, description = "CI config id")),
    request_body = UpdateCiConfig,
    responses((status = 200, description = "Updated", body = CiConfig)),
    security(("bearer_auth" = []))
)]
pub async fn update_ci_config(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateCiConfig>,
) -> AppResult<Json<CiConfig>> {
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
    let config = body.config.unwrap_or(existing.config);
    validate_config(&existing.provider, &config)?;
    let webhook_secret = body.webhook_secret.or(existing.webhook_secret);
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
    Ok(Json(row))
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
        let run = sqlx::query_as::<_, TestRun>(
            r#"SELECT id, project_id, plan_id, name, description, status, source,
                      external_ref, meta, started_at, finished_at, created_at, updated_at
               FROM test_runs WHERE id = $1"#,
        )
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
            status: Some("running".into()),
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
        let run = sqlx::query_as::<_, TestRun>(
            r#"INSERT INTO test_runs (project_id, name, status, source, meta, started_at)
               VALUES ($1, $2, $3, $4, $5, NOW())
               RETURNING id, project_id, plan_id, name, description, status, source,
                         external_ref, meta, started_at, finished_at, created_at, updated_at"#,
        )
        .bind(cfg.project_id)
        .bind(create.name)
        .bind(create.status.as_deref().unwrap_or("running"))
        .bind(create.source)
        .bind(create.meta)
        .fetch_one(&state.pool)
        .await?;
        Some(run.id)
    };

    let inputs = if body.inputs.is_null() {
        json!({})
    } else {
        body.inputs
    };

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
