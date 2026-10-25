use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::{require_permission, OptionalAuthUser, Permission};
use crate::error::{AppError, AppResult};
use crate::models::{AttachmentMeta, CreateAttachment};
use crate::routes::AppState;
use crate::services::cases::ensure_project;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/projects/{project_id}/attachments",
            get(list_attachments).post(create_attachment),
        )
        .route(
            "/api/v1/projects/{project_id}/attachments/{attachment_id}",
            get(download_attachment).delete(delete_attachment),
        )
}

#[derive(Debug, Deserialize)]
pub struct ListAttachmentsQuery {
    pub entity_type: Option<String>,
    pub entity_id: Option<Uuid>,
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/attachments",
    tag = "attachments",
    params(
        ("project_id" = Uuid, Path),
        ("entity_type" = Option<String>, Query),
        ("entity_id" = Option<Uuid>, Query)
    ),
    responses((status = 200, description = "Attachments", body = [AttachmentMeta])),
    security(("bearer_auth" = []))
)]
pub async fn list_attachments(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
    Query(query): Query<ListAttachmentsQuery>,
) -> AppResult<Json<Vec<AttachmentMeta>>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    ensure_project(&state.pool, project_id).await?;
    let rows = sqlx::query_as::<_, AttachmentMeta>(
        r#"SELECT id, project_id, entity_type, entity_id, filename, content_type,
                  size_bytes, created_at
           FROM attachments
           WHERE project_id = $1
             AND ($2::text IS NULL OR entity_type = $2)
             AND ($3::uuid IS NULL OR entity_id = $3)
           ORDER BY created_at DESC"#,
    )
    .bind(project_id)
    .bind(query.entity_type.as_deref())
    .bind(query.entity_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/attachments",
    tag = "attachments",
    params(("project_id" = Uuid, Path)),
    request_body = CreateAttachment,
    responses((status = 200, description = "Created", body = AttachmentMeta)),
    security(("bearer_auth" = []))
)]
pub async fn create_attachment(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<CreateAttachment>,
) -> AppResult<Json<AttachmentMeta>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ManageAttachments,
    )
    .await?;
    ensure_project(&state.pool, project_id).await?;
    let allowed = ["case", "result", "run"];
    if !allowed.contains(&body.entity_type.as_str()) {
        return Err(AppError::BadRequest("invalid entity_type".into()));
    }
    if body.filename.trim().is_empty() {
        return Err(AppError::BadRequest("filename required".into()));
    }

    let cleaned: String = body
        .content_base64
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let bytes = STANDARD
        .decode(cleaned.as_bytes())
        .map_err(|_| AppError::BadRequest("invalid base64 content".into()))?;
    if bytes.len() > 5 * 1024 * 1024 {
        return Err(AppError::BadRequest(
            "attachment exceeds 5 MiB limit".into(),
        ));
    }

    let row = sqlx::query_as::<_, AttachmentMeta>(
        r#"INSERT INTO attachments
             (project_id, entity_type, entity_id, filename, content_type, size_bytes, content)
           VALUES ($1, $2, $3, $4, $5, $6, $7)
           RETURNING id, project_id, entity_type, entity_id, filename, content_type,
                     size_bytes, created_at"#,
    )
    .bind(project_id)
    .bind(&body.entity_type)
    .bind(body.entity_id)
    .bind(body.filename.trim())
    .bind(&body.content_type)
    .bind(bytes.len() as i64)
    .bind(&bytes)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(row))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/attachments/{attachment_id}",
    tag = "attachments",
    params(("project_id" = Uuid, Path), ("attachment_id" = Uuid, Path)),
    responses((status = 200, description = "Raw file bytes")),
    security(("bearer_auth" = []))
)]
pub async fn download_attachment(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, attachment_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Response> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    let row = sqlx::query_as::<_, (String, String, Option<Vec<u8>>)>(
        r#"SELECT filename, content_type, content
           FROM attachments WHERE project_id = $1 AND id = $2"#,
    )
    .bind(project_id)
    .bind(attachment_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("attachment".into()))?;

    let (filename, content_type, content) = row;
    let bytes = content.unwrap_or_default();
    let disposition = format!("attachment; filename=\"{}\"", filename.replace('"', ""));
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, content_type),
            (header::CONTENT_DISPOSITION, disposition),
        ],
        bytes,
    )
        .into_response())
}

#[utoipa::path(
    delete,
    path = "/api/v1/projects/{project_id}/attachments/{attachment_id}",
    tag = "attachments",
    params(("project_id" = Uuid, Path), ("attachment_id" = Uuid, Path)),
    responses((status = 204, description = "Deleted")),
    security(("bearer_auth" = []))
)]
pub async fn delete_attachment(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, attachment_id)): Path<(Uuid, Uuid)>,
) -> AppResult<StatusCode> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ManageAttachments,
    )
    .await?;
    let res = sqlx::query("DELETE FROM attachments WHERE project_id = $1 AND id = $2")
        .bind(project_id)
        .bind(attachment_id)
        .execute(&state.pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound("attachment".into()));
    }
    Ok(StatusCode::NO_CONTENT)
}
