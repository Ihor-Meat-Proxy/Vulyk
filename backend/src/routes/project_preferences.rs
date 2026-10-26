//! Per-user per-project preferences (case list columns).

use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use uuid::Uuid;

use crate::auth::{require_permission, OptionalAuthUser, Permission, RequireAuth};
use crate::error::{AppError, AppResult};
use crate::models::CaseListColumnsConfig;
use crate::routes::AppState;
use crate::services::case_list::{
    load_case_list_columns, reset_case_list_columns, save_case_list_columns,
};
use crate::services::cases::ensure_project;

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/api/v1/projects/{project_id}/preferences/case-list-columns",
        get(get_case_list_columns)
            .put(put_case_list_columns)
            .delete(delete_case_list_columns),
    )
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/preferences/case-list-columns",
    tag = "preferences",
    params(("project_id" = Uuid, Path, description = "Project id")),
    responses(
        (status = 200, description = "Case list column preferences (defaults when unset)", body = CaseListColumnsConfig)
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_case_list_columns(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
) -> AppResult<Json<CaseListColumnsConfig>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    ensure_project(&state.pool, project_id).await?;

    let cfg = match &auth {
        Some(user) => load_case_list_columns(&state.pool, user.id(), project_id).await?,
        None => CaseListColumnsConfig::default_config(),
    };
    Ok(Json(cfg))
}

#[utoipa::path(
    put,
    path = "/api/v1/projects/{project_id}/preferences/case-list-columns",
    tag = "preferences",
    params(("project_id" = Uuid, Path, description = "Project id")),
    request_body = CaseListColumnsConfig,
    responses(
        (status = 200, description = "Saved preferences", body = CaseListColumnsConfig)
    ),
    security(("bearer_auth" = []))
)]
pub async fn put_case_list_columns(
    State(state): State<AppState>,
    RequireAuth(auth): RequireAuth,
    Path(project_id): Path<Uuid>,
    Json(body): Json<CaseListColumnsConfig>,
) -> AppResult<Json<CaseListColumnsConfig>> {
    require_permission(
        &state.pool,
        &Some(auth.clone()),
        true,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    ensure_project(&state.pool, project_id).await?;
    let saved = save_case_list_columns(&state.pool, auth.id(), project_id, &body).await?;
    Ok(Json(saved))
}

#[utoipa::path(
    delete,
    path = "/api/v1/projects/{project_id}/preferences/case-list-columns",
    tag = "preferences",
    params(("project_id" = Uuid, Path, description = "Project id")),
    responses(
        (status = 200, description = "Reset to default", body = CaseListColumnsConfig)
    ),
    security(("bearer_auth" = []))
)]
pub async fn delete_case_list_columns(
    State(state): State<AppState>,
    RequireAuth(auth): RequireAuth,
    Path(project_id): Path<Uuid>,
) -> AppResult<Json<CaseListColumnsConfig>> {
    require_permission(
        &state.pool,
        &Some(auth.clone()),
        true,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    ensure_project(&state.pool, project_id).await?;
    let defaults = reset_case_list_columns(&state.pool, auth.id(), project_id).await?;
    Ok(Json(defaults))
}

/// Helper for unit tests / callers that need a typed error without HTTP.
#[allow(dead_code)]
pub fn validate_body(body: &CaseListColumnsConfig) -> Result<CaseListColumnsConfig, AppError> {
    CaseListColumnsConfig::validate(body).map_err(AppError::BadRequest)
}
