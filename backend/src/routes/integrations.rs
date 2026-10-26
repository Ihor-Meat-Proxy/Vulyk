use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::auth::{require_permission, OptionalAuthUser, Permission};
use crate::error::{AppError, AppResult};
use crate::integrations::{self, DefectDraft, IntegrationConfig, RequirementLink};
use crate::models::{
    CreateDefectRequest, CreateIntegration, CreateRequirement, Defect, Integration,
    IntegrationPublic, Requirement, UpdateIntegration,
};
use crate::routes::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/projects/{project_id}/integrations",
            get(list_integrations).post(create_integration),
        )
        .route(
            "/api/v1/projects/{project_id}/integrations/{id}",
            get(get_integration)
                .put(update_integration)
                .delete(delete_integration),
        )
        .route(
            "/api/v1/projects/{project_id}/integrations/{id}/requirements:import",
            axum::routing::post(import_remote_requirements),
        )
        .route(
            "/api/v1/projects/{project_id}/integrations/{id}/requirements:search",
            get(search_remote_requirements),
        )
        .route(
            "/api/v1/projects/{project_id}/defects",
            get(list_defects).post(create_defect),
        )
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    #[serde(default)]
    pub q: String,
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/integrations",
    tag = "integrations",
    params(("project_id" = Uuid, Path, description = "Project id")),
    responses((status = 200, body = [IntegrationPublic])),
    security(("bearer_auth" = []))
)]
pub async fn list_integrations(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
) -> AppResult<Json<Vec<IntegrationPublic>>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    ensure_project(&state, project_id).await?;
    let rows = sqlx::query_as::<_, Integration>(
        r#"SELECT id, project_id, kind, name, base_url, config, secrets, enabled, created_at, updated_at
           FROM integrations WHERE project_id = $1 ORDER BY name"#,
    )
    .bind(project_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(
        rows.into_iter().map(IntegrationPublic::from).collect(),
    ))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/integrations",
    tag = "integrations",
    params(("project_id" = Uuid, Path, description = "Project id")),
    request_body = CreateIntegration,
    responses((status = 200, body = IntegrationPublic)),
    security(("bearer_auth" = []))
)]
pub async fn create_integration(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<CreateIntegration>,
) -> AppResult<Json<IntegrationPublic>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::UpdateProject,
    )
    .await?;
    ensure_project(&state, project_id).await?;
    if !integrations::known_kinds().contains(&body.kind.as_str()) {
        return Err(AppError::BadRequest(format!(
            "kind must be one of {:?}",
            integrations::known_kinds()
        )));
    }
    if body.name.trim().is_empty() {
        return Err(AppError::BadRequest("name is required".into()));
    }
    let row = sqlx::query_as::<_, Integration>(
        r#"INSERT INTO integrations (project_id, kind, name, base_url, config, secrets, enabled)
           VALUES ($1, $2, $3, $4, $5, $6, $7)
           RETURNING id, project_id, kind, name, base_url, config, secrets, enabled, created_at, updated_at"#,
    )
    .bind(project_id)
    .bind(&body.kind)
    .bind(body.name.trim())
    .bind(body.base_url.trim())
    .bind(&body.config)
    .bind(&body.secrets)
    .bind(body.enabled)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(row.into()))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/integrations/{id}",
    tag = "integrations",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("id" = Uuid, Path, description = "Integration id")
    ),
    responses((status = 200, body = IntegrationPublic)),
    security(("bearer_auth" = []))
)]
pub async fn get_integration(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<IntegrationPublic>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    Ok(Json(load_integration(&state, project_id, id).await?.into()))
}

#[utoipa::path(
    put,
    path = "/api/v1/projects/{project_id}/integrations/{id}",
    tag = "integrations",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("id" = Uuid, Path, description = "Integration id")
    ),
    request_body = UpdateIntegration,
    responses((status = 200, body = IntegrationPublic)),
    security(("bearer_auth" = []))
)]
pub async fn update_integration(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, id)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateIntegration>,
) -> AppResult<Json<IntegrationPublic>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::UpdateProject,
    )
    .await?;
    let existing = load_integration(&state, project_id, id).await?;
    let row = sqlx::query_as::<_, Integration>(
        r#"UPDATE integrations SET
             name = $3, base_url = $4, config = $5, secrets = $6, enabled = $7, updated_at = NOW()
           WHERE project_id = $1 AND id = $2
           RETURNING id, project_id, kind, name, base_url, config, secrets, enabled, created_at, updated_at"#,
    )
    .bind(project_id)
    .bind(id)
    .bind(body.name.unwrap_or(existing.name))
    .bind(body.base_url.unwrap_or(existing.base_url))
    .bind(body.config.unwrap_or(existing.config))
    .bind(body.secrets.unwrap_or(existing.secrets))
    .bind(body.enabled.unwrap_or(existing.enabled))
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(row.into()))
}

#[utoipa::path(
    delete,
    path = "/api/v1/projects/{project_id}/integrations/{id}",
    tag = "integrations",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("id" = Uuid, Path, description = "Integration id")
    ),
    responses((status = 204, description = "Deleted")),
    security(("bearer_auth" = []))
)]
pub async fn delete_integration(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, id)): Path<(Uuid, Uuid)>,
) -> AppResult<axum::http::StatusCode> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::UpdateProject,
    )
    .await?;
    let result = sqlx::query("DELETE FROM integrations WHERE project_id = $1 AND id = $2")
        .bind(project_id)
        .bind(id)
        .execute(&state.pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("integration".into()));
    }
    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/integrations/{id}/requirements:search",
    tag = "integrations",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("id" = Uuid, Path, description = "Integration id"),
        ("q" = Option<String>, Query, description = "Search query")
    ),
    responses((status = 200, description = "Remote requirements")),
    security(("bearer_auth" = []))
)]
pub async fn search_remote_requirements(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, id)): Path<(Uuid, Uuid)>,
    Query(query): Query<SearchQuery>,
) -> AppResult<Json<Vec<crate::integrations::RemoteRequirement>>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    let integration = load_integration(&state, project_id, id).await?;
    let adapter = integrations::adapter_for(&integration.kind)?;
    let cfg = config_from(&integration);
    let list = adapter.list_requirements(&cfg, &query.q).await?;
    Ok(Json(list))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/integrations/{id}/requirements:import",
    tag = "integrations",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("id" = Uuid, Path, description = "Integration id")
    ),
    responses((status = 200, body = [Requirement])),
    security(("bearer_auth" = []))
)]
pub async fn import_remote_requirements(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, id)): Path<(Uuid, Uuid)>,
    Query(query): Query<SearchQuery>,
) -> AppResult<Json<Vec<Requirement>>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::UpdateProject,
    )
    .await?;
    let integration = load_integration(&state, project_id, id).await?;
    let adapter = integrations::adapter_for(&integration.kind)?;
    let cfg = config_from(&integration);
    let remote = adapter.list_requirements(&cfg, &query.q).await?;
    let mut saved = Vec::new();
    for item in remote {
        let body = CreateRequirement {
            title: item.title.clone(),
            description: item.description.clone(),
            acceptance_criteria: String::new(),
            folder_id: None,
            source: integration.kind.clone(),
            external_key: Some(item.external_key.clone()),
            external_id: Some(item.external_id.clone()),
            url: item.url.clone(),
            metadata: item.raw.clone(),
        };
        let row = upsert_requirement(&state, project_id, body).await?;
        // Best-effort remote link annotation (no-op for fixtures).
        let _ = adapter
            .link_requirement(
                &cfg,
                &RequirementLink {
                    requirement_id: row.id,
                    case_id: Uuid::nil(),
                    external_id: item.external_id,
                    external_key: item.external_key,
                },
            )
            .await;
        saved.push(row);
    }
    Ok(Json(saved))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/defects",
    tag = "integrations",
    params(("project_id" = Uuid, Path, description = "Project id")),
    responses((status = 200, body = [Defect])),
    security(("bearer_auth" = []))
)]
pub async fn list_defects(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
) -> AppResult<Json<Vec<Defect>>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    ensure_project(&state, project_id).await?;
    let rows = sqlx::query_as::<_, Defect>(
        r#"SELECT id, project_id, integration_id, result_id, case_id, external_key, external_url,
                  title, status, metadata, created_at, updated_at
           FROM defects WHERE project_id = $1 ORDER BY created_at DESC"#,
    )
    .bind(project_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/defects",
    tag = "integrations",
    params(("project_id" = Uuid, Path, description = "Project id")),
    request_body = CreateDefectRequest,
    responses((status = 200, body = Defect)),
    security(("bearer_auth" = []))
)]
pub async fn create_defect(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<CreateDefectRequest>,
) -> AppResult<Json<Defect>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::UpdateProject,
    )
    .await?;
    ensure_project(&state, project_id).await?;
    if body.title.trim().is_empty() {
        return Err(AppError::BadRequest("title is required".into()));
    }
    let integration = load_integration(&state, project_id, body.integration_id).await?;
    let adapter = integrations::adapter_for(&integration.kind)?;
    let cfg = config_from(&integration);

    let mut case_key = None;
    let mut result_status = None;
    let mut steps = body.steps_to_reproduce.clone();

    if let Some(result_id) = body.result_id {
        let row: Option<(String, Option<String>, Option<String>)> = sqlx::query_as(
            r#"SELECT r.status, c.key, r.message
               FROM test_results r
               LEFT JOIN test_cases c ON c.id = r.case_id
               JOIN test_runs tr ON tr.id = r.run_id
               WHERE r.id = $1 AND tr.project_id = $2"#,
        )
        .bind(result_id)
        .bind(project_id)
        .fetch_optional(&state.pool)
        .await?;
        if let Some((status, key, message)) = row {
            result_status = Some(status);
            case_key = key;
            if steps.is_empty() {
                steps = message.unwrap_or_default();
            }
        }
    } else if let Some(case_id) = body.case_id {
        let key: Option<(String,)> =
            sqlx::query_as("SELECT key FROM test_cases WHERE id = $1 AND project_id = $2")
                .bind(case_id)
                .bind(project_id)
                .fetch_optional(&state.pool)
                .await?;
        case_key = key.map(|k| k.0);
    }

    let remote = adapter
        .create_defect(
            &cfg,
            DefectDraft {
                title: body.title.trim().to_string(),
                description: body.description.clone(),
                steps_to_reproduce: steps,
                case_key,
                result_status,
            },
        )
        .await?;

    let defect = sqlx::query_as::<_, Defect>(
        r#"INSERT INTO defects
           (project_id, integration_id, result_id, case_id, external_key, external_url, title, status, metadata)
           VALUES ($1, $2, $3, $4, $5, $6, $7, 'open', $8)
           RETURNING id, project_id, integration_id, result_id, case_id, external_key, external_url,
                     title, status, metadata, created_at, updated_at"#,
    )
    .bind(project_id)
    .bind(body.integration_id)
    .bind(body.result_id)
    .bind(body.case_id)
    .bind(&remote.external_key)
    .bind(&remote.url)
    .bind(body.title.trim())
    .bind(remote.raw)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(defect))
}

async fn upsert_requirement(
    state: &AppState,
    project_id: Uuid,
    body: CreateRequirement,
) -> AppResult<Requirement> {
    if let Some(ref key) = body.external_key {
        let existing: Option<Requirement> = sqlx::query_as(
            r#"SELECT id, project_id, folder_id, external_key, external_id, title, description,
                      acceptance_criteria, source, url, metadata, created_at, updated_at
               FROM requirements
               WHERE project_id = $1 AND source = $2 AND external_key = $3"#,
        )
        .bind(project_id)
        .bind(&body.source)
        .bind(key)
        .fetch_optional(&state.pool)
        .await?;
        if let Some(existing) = existing {
            let row = sqlx::query_as::<_, Requirement>(
                r#"UPDATE requirements SET
                     title = $3, description = $4, external_id = $5, url = $6, metadata = $7, updated_at = NOW()
                   WHERE id = $1 AND project_id = $2
                   RETURNING id, project_id, folder_id, external_key, external_id, title, description,
                             acceptance_criteria, source, url, metadata, created_at, updated_at"#,
            )
            .bind(existing.id)
            .bind(project_id)
            .bind(&body.title)
            .bind(&body.description)
            .bind(&body.external_id)
            .bind(&body.url)
            .bind(&body.metadata)
            .fetch_one(&state.pool)
            .await?;
            return Ok(row);
        }
    }
    let metadata = if body.metadata.is_null() {
        json!({})
    } else {
        body.metadata
    };
    let row = sqlx::query_as::<_, Requirement>(
        r#"INSERT INTO requirements
           (project_id, folder_id, external_key, external_id, title, description,
            acceptance_criteria, source, url, metadata)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
           RETURNING id, project_id, folder_id, external_key, external_id, title, description,
                     acceptance_criteria, source, url, metadata, created_at, updated_at"#,
    )
    .bind(project_id)
    .bind(body.folder_id)
    .bind(&body.external_key)
    .bind(&body.external_id)
    .bind(&body.title)
    .bind(&body.description)
    .bind(&body.acceptance_criteria)
    .bind(&body.source)
    .bind(&body.url)
    .bind(metadata)
    .fetch_one(&state.pool)
    .await?;
    Ok(row)
}

fn config_from(integration: &Integration) -> IntegrationConfig {
    IntegrationConfig {
        base_url: integration.base_url.clone(),
        config: integration.config.clone(),
        secrets: integration.secrets.clone(),
    }
}

async fn load_integration(state: &AppState, project_id: Uuid, id: Uuid) -> AppResult<Integration> {
    sqlx::query_as::<_, Integration>(
        r#"SELECT id, project_id, kind, name, base_url, config, secrets, enabled, created_at, updated_at
           FROM integrations WHERE project_id = $1 AND id = $2"#,
    )
    .bind(project_id)
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("integration".into()))
}

async fn ensure_project(state: &AppState, project_id: Uuid) -> AppResult<()> {
    let ok: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM projects WHERE id = $1")
        .bind(project_id)
        .fetch_optional(&state.pool)
        .await?;
    if ok.is_none() {
        return Err(AppError::NotFound("project".into()));
    }
    Ok(())
}
