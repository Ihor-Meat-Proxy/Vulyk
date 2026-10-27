//! Admin content backup destinations, policies, runs, and restore.

use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::header,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::RequireSystemAdmin;
use crate::error::{AppError, AppResult};
use crate::models::{
    BackupDestination, BackupPolicy, BackupRun, BackupTestResult, CreateBackupDestination,
    CreateBackupPolicy, ManualBackupRequest, RestorePreview, RestoreRequest,
    UpdateBackupDestination, UpdateBackupPolicy,
};
use crate::routes::AppState;
use crate::services::backup;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/admin/backups/destinations",
            get(list_destinations).post(create_destination),
        )
        .route(
            "/api/v1/admin/backups/destinations/{id}",
            get(get_destination)
                .patch(update_destination)
                .delete(delete_destination),
        )
        .route(
            "/api/v1/admin/backups/destinations/{id}/test",
            axum::routing::post(test_destination),
        )
        .route(
            "/api/v1/admin/backups/policies",
            get(list_policies).post(create_policy),
        )
        .route(
            "/api/v1/admin/backups/policies/{id}",
            axum::routing::patch(update_policy).delete(delete_policy),
        )
        .route("/api/v1/admin/backups/run", axum::routing::post(run_manual))
        .route("/api/v1/admin/backups/runs", get(list_runs))
        .route("/api/v1/admin/backups/runs/{id}", get(get_run))
        .route(
            "/api/v1/admin/backups/runs/{id}/download",
            get(download_run),
        )
        .route(
            "/api/v1/admin/backups/restore",
            axum::routing::post(restore),
        )
        .route(
            "/api/v1/admin/backups/restore/upload",
            axum::routing::post(restore_upload),
        )
}

#[derive(Debug, Deserialize)]
pub struct RunsQuery {
    pub project_id: Option<Uuid>,
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_limit() -> i64 {
    50
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/backups/destinations",
    tag = "backups",
    responses((status = 200, body = [BackupDestination])),
    security(("bearer_auth" = []))
)]
pub async fn list_destinations(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
) -> AppResult<Json<Vec<BackupDestination>>> {
    Ok(Json(backup::list_destinations(&state.pool).await?))
}

#[utoipa::path(
    post,
    path = "/api/v1/admin/backups/destinations",
    tag = "backups",
    request_body = CreateBackupDestination,
    responses((status = 200, body = BackupDestination)),
    security(("bearer_auth" = []))
)]
pub async fn create_destination(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
    Json(body): Json<CreateBackupDestination>,
) -> AppResult<Json<BackupDestination>> {
    Ok(Json(
        backup::create_destination(&state.pool, &state.secrets, body).await?,
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/backups/destinations/{id}",
    tag = "backups",
    params(("id" = Uuid, Path)),
    responses((status = 200, body = BackupDestination)),
    security(("bearer_auth" = []))
)]
pub async fn get_destination(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
    Path(id): Path<Uuid>,
) -> AppResult<Json<BackupDestination>> {
    Ok(Json(backup::get_destination(&state.pool, id).await?))
}

#[utoipa::path(
    patch,
    path = "/api/v1/admin/backups/destinations/{id}",
    tag = "backups",
    params(("id" = Uuid, Path)),
    request_body = UpdateBackupDestination,
    responses((status = 200, body = BackupDestination)),
    security(("bearer_auth" = []))
)]
pub async fn update_destination(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateBackupDestination>,
) -> AppResult<Json<BackupDestination>> {
    Ok(Json(
        backup::update_destination(&state.pool, &state.secrets, id, body).await?,
    ))
}

#[utoipa::path(
    delete,
    path = "/api/v1/admin/backups/destinations/{id}",
    tag = "backups",
    params(("id" = Uuid, Path)),
    responses((status = 204, description = "Deleted")),
    security(("bearer_auth" = []))
)]
pub async fn delete_destination(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
    Path(id): Path<Uuid>,
) -> AppResult<impl IntoResponse> {
    backup::delete_destination(&state.pool, id).await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/api/v1/admin/backups/destinations/{id}/test",
    tag = "backups",
    params(("id" = Uuid, Path)),
    responses((status = 200, body = BackupTestResult)),
    security(("bearer_auth" = []))
)]
pub async fn test_destination(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
    Path(id): Path<Uuid>,
) -> AppResult<Json<BackupTestResult>> {
    Ok(Json(
        backup::test_destination(&state.pool, &state.secrets, id).await?,
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/backups/policies",
    tag = "backups",
    responses((status = 200, body = [BackupPolicy])),
    security(("bearer_auth" = []))
)]
pub async fn list_policies(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
) -> AppResult<Json<Vec<BackupPolicy>>> {
    Ok(Json(backup::list_policies(&state.pool).await?))
}

#[utoipa::path(
    post,
    path = "/api/v1/admin/backups/policies",
    tag = "backups",
    request_body = CreateBackupPolicy,
    responses((status = 200, body = BackupPolicy)),
    security(("bearer_auth" = []))
)]
pub async fn create_policy(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
    Json(body): Json<CreateBackupPolicy>,
) -> AppResult<Json<BackupPolicy>> {
    Ok(Json(backup::create_policy(&state.pool, body).await?))
}

#[utoipa::path(
    patch,
    path = "/api/v1/admin/backups/policies/{id}",
    tag = "backups",
    params(("id" = Uuid, Path)),
    request_body = UpdateBackupPolicy,
    responses((status = 200, body = BackupPolicy)),
    security(("bearer_auth" = []))
)]
pub async fn update_policy(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateBackupPolicy>,
) -> AppResult<Json<BackupPolicy>> {
    Ok(Json(backup::update_policy(&state.pool, id, body).await?))
}

#[utoipa::path(
    delete,
    path = "/api/v1/admin/backups/policies/{id}",
    tag = "backups",
    params(("id" = Uuid, Path)),
    responses((status = 204, description = "Deleted")),
    security(("bearer_auth" = []))
)]
pub async fn delete_policy(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
    Path(id): Path<Uuid>,
) -> AppResult<impl IntoResponse> {
    backup::delete_policy(&state.pool, id).await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/api/v1/admin/backups/run",
    tag = "backups",
    request_body = ManualBackupRequest,
    responses((status = 200, body = BackupRun)),
    security(("bearer_auth" = []))
)]
pub async fn run_manual(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
    Json(body): Json<ManualBackupRequest>,
) -> AppResult<Json<BackupRun>> {
    Ok(Json(
        backup::run_manual(&state.pool, &state.secrets, body).await?,
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/backups/runs",
    tag = "backups",
    params(
        ("project_id" = Option<Uuid>, Query),
        ("limit" = Option<i64>, Query),
    ),
    responses((status = 200, body = [BackupRun])),
    security(("bearer_auth" = []))
)]
pub async fn list_runs(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
    Query(q): Query<RunsQuery>,
) -> AppResult<Json<Vec<BackupRun>>> {
    Ok(Json(
        backup::list_runs(&state.pool, q.project_id, q.limit).await?,
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/backups/runs/{id}",
    tag = "backups",
    params(("id" = Uuid, Path)),
    responses((status = 200, body = BackupRun)),
    security(("bearer_auth" = []))
)]
pub async fn get_run(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
    Path(id): Path<Uuid>,
) -> AppResult<Json<BackupRun>> {
    Ok(Json(backup::get_run(&state.pool, id).await?))
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/backups/runs/{id}/download",
    tag = "backups",
    params(("id" = Uuid, Path)),
    responses((status = 200, description = "ZIP archive")),
    security(("bearer_auth" = []))
)]
pub async fn download_run(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
    Path(id): Path<Uuid>,
) -> AppResult<impl IntoResponse> {
    let (name, bytes) = backup::download_run_bytes(&state.pool, &state.secrets, id).await?;
    Ok((
        [
            (header::CONTENT_TYPE, "application/zip".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{name}\""),
            ),
        ],
        bytes,
    ))
}

#[utoipa::path(
    post,
    path = "/api/v1/admin/backups/restore",
    tag = "backups",
    request_body = RestoreRequest,
    responses((status = 200, body = RestorePreview)),
    security(("bearer_auth" = []))
)]
pub async fn restore(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
    Json(body): Json<RestoreRequest>,
) -> AppResult<Json<RestorePreview>> {
    Ok(Json(
        backup::restore(&state.pool, &state.secrets, body).await?,
    ))
}

/// Upload a ZIP/JSON archive body; query params control restore mode.
#[derive(Debug, Deserialize)]
pub struct RestoreUploadQuery {
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default = "default_conflict")]
    pub conflict: String,
    pub target_project_id: Option<Uuid>,
    pub new_project_key: Option<String>,
    #[serde(default)]
    pub dry_run: bool,
}

fn default_mode() -> String {
    "merge".into()
}
fn default_conflict() -> String {
    "skip".into()
}

#[utoipa::path(
    post,
    path = "/api/v1/admin/backups/restore/upload",
    tag = "backups",
    params(
        ("mode" = Option<String>, Query, description = "Restore mode (default: merge)"),
        ("conflict" = Option<String>, Query, description = "Conflict strategy (default: skip)"),
        ("target_project_id" = Option<Uuid>, Query, description = "Existing project to restore into"),
        ("new_project_key" = Option<String>, Query, description = "Key when creating a new project"),
        ("dry_run" = Option<bool>, Query, description = "Preview only when true")
    ),
    request_body(
        content = Vec<u8>,
        description = "ZIP or JSON content archive bytes",
        content_type = "application/octet-stream"
    ),
    responses((status = 200, body = RestorePreview)),
    security(("bearer_auth" = []))
)]
pub async fn restore_upload(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
    Query(q): Query<RestoreUploadQuery>,
    body: Bytes,
) -> AppResult<Json<RestorePreview>> {
    if body.is_empty() {
        return Err(AppError::BadRequest("empty upload body".into()));
    }
    let archive = backup::archive::parse_archive_bytes(&body)?;
    let req = RestoreRequest {
        run_id: None,
        archive: Some(archive),
        mode: q.mode,
        conflict: q.conflict,
        target_project_id: q.target_project_id,
        new_project_key: q.new_project_key,
        dry_run: q.dry_run,
    };
    Ok(Json(
        backup::restore(&state.pool, &state.secrets, req).await?,
    ))
}
