use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use uuid::Uuid;

use crate::auth::{require_permission, OptionalAuthUser, Permission};
use crate::error::{AppError, AppResult};
use crate::models::{CreateTag, Tag, UpdateTag};
use crate::routes::AppState;
use crate::services::cases::ensure_project;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/projects/{project_id}/tags",
            get(list_tags).post(create_tag),
        )
        .route(
            "/api/v1/projects/{project_id}/tags/{tag_id}",
            get(get_tag).put(update_tag).delete(delete_tag),
        )
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/tags",
    tag = "tags",
    params(("project_id" = Uuid, Path)),
    responses((status = 200, description = "Tags", body = [Tag])),
    security(("bearer_auth" = []))
)]
pub async fn list_tags(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
) -> AppResult<Json<Vec<Tag>>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    ensure_project(&state.pool, project_id).await?;
    let tags = sqlx::query_as::<_, Tag>(
        r#"SELECT id, project_id, name, color, created_at
           FROM tags WHERE project_id = $1 ORDER BY name"#,
    )
    .bind(project_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(tags))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/tags",
    tag = "tags",
    params(("project_id" = Uuid, Path)),
    request_body = CreateTag,
    responses((status = 200, description = "Created", body = Tag)),
    security(("bearer_auth" = []))
)]
pub async fn create_tag(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<CreateTag>,
) -> AppResult<Json<Tag>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::CreateEditCases,
    )
    .await?;
    ensure_project(&state.pool, project_id).await?;
    let name = body.name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("name required".into()));
    }
    let tag = sqlx::query_as::<_, Tag>(
        r#"INSERT INTO tags (project_id, name, color)
           VALUES ($1, $2, $3)
           RETURNING id, project_id, name, color, created_at"#,
    )
    .bind(project_id)
    .bind(name)
    .bind(body.color.trim())
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(tag))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/tags/{tag_id}",
    tag = "tags",
    params(("project_id" = Uuid, Path), ("tag_id" = Uuid, Path)),
    responses((status = 200, description = "Tag", body = Tag)),
    security(("bearer_auth" = []))
)]
pub async fn get_tag(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, tag_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<Tag>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    let tag = sqlx::query_as::<_, Tag>(
        r#"SELECT id, project_id, name, color, created_at
           FROM tags WHERE project_id = $1 AND id = $2"#,
    )
    .bind(project_id)
    .bind(tag_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("tag".into()))?;
    Ok(Json(tag))
}

#[utoipa::path(
    put,
    path = "/api/v1/projects/{project_id}/tags/{tag_id}",
    tag = "tags",
    params(("project_id" = Uuid, Path), ("tag_id" = Uuid, Path)),
    request_body = UpdateTag,
    responses((status = 200, description = "Updated", body = Tag)),
    security(("bearer_auth" = []))
)]
pub async fn update_tag(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, tag_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateTag>,
) -> AppResult<Json<Tag>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::CreateEditCases,
    )
    .await?;
    let existing = sqlx::query_as::<_, Tag>(
        r#"SELECT id, project_id, name, color, created_at
           FROM tags WHERE project_id = $1 AND id = $2"#,
    )
    .bind(project_id)
    .bind(tag_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("tag".into()))?;

    let name = body.name.unwrap_or(existing.name);
    let color = body.color.unwrap_or(existing.color);
    let tag = sqlx::query_as::<_, Tag>(
        r#"UPDATE tags SET name = $3, color = $4
           WHERE project_id = $1 AND id = $2
           RETURNING id, project_id, name, color, created_at"#,
    )
    .bind(project_id)
    .bind(tag_id)
    .bind(name.trim())
    .bind(color.trim())
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(tag))
}

#[utoipa::path(
    delete,
    path = "/api/v1/projects/{project_id}/tags/{tag_id}",
    tag = "tags",
    params(("project_id" = Uuid, Path), ("tag_id" = Uuid, Path)),
    responses((status = 204, description = "Deleted")),
    security(("bearer_auth" = []))
)]
pub async fn delete_tag(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, tag_id)): Path<(Uuid, Uuid)>,
) -> AppResult<axum::http::StatusCode> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::CreateEditCases,
    )
    .await?;
    let res = sqlx::query("DELETE FROM tags WHERE project_id = $1 AND id = $2")
        .bind(project_id)
        .bind(tag_id)
        .execute(&state.pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound("tag".into()));
    }
    Ok(axum::http::StatusCode::NO_CONTENT)
}
