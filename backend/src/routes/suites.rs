use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::{CreateSuite, Suite, UpdateSuite};
use crate::routes::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/projects/{project_id}/suites",
            get(list_suites).post(create_suite),
        )
        .route(
            "/api/v1/projects/{project_id}/suites/{suite_id}",
            get(get_suite).put(update_suite).delete(delete_suite),
        )
}

pub async fn ensure_project(pool: &sqlx::PgPool, project_id: Uuid) -> AppResult<()> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM projects WHERE id = $1)")
        .bind(project_id)
        .fetch_one(pool)
        .await?;
    if !exists {
        return Err(AppError::NotFound("project".into()));
    }
    Ok(())
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/suites",
    tag = "suites",
    params(("project_id" = Uuid, Path, description = "Project id")),
    responses((status = 200, description = "List suites", body = [Suite]))
)]
pub async fn list_suites(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
) -> AppResult<Json<Vec<Suite>>> {
    ensure_project(&state.pool, project_id).await?;
    let suites = sqlx::query_as::<_, Suite>(
        r#"SELECT id, project_id, parent_id, name, sort_order, created_at, updated_at
           FROM suites WHERE project_id = $1
           ORDER BY sort_order, name"#,
    )
    .bind(project_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(suites))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/suites",
    tag = "suites",
    params(("project_id" = Uuid, Path, description = "Project id")),
    request_body = CreateSuite,
    responses((status = 200, description = "Created suite", body = Suite))
)]
pub async fn create_suite(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Json(body): Json<CreateSuite>,
) -> AppResult<Json<Suite>> {
    ensure_project(&state.pool, project_id).await?;
    if body.name.trim().is_empty() {
        return Err(AppError::BadRequest("name is required".into()));
    }

    let suite = sqlx::query_as::<_, Suite>(
        r#"INSERT INTO suites (project_id, parent_id, name, sort_order)
           VALUES ($1, $2, $3, $4)
           RETURNING id, project_id, parent_id, name, sort_order, created_at, updated_at"#,
    )
    .bind(project_id)
    .bind(body.parent_id)
    .bind(body.name.trim())
    .bind(body.sort_order)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(suite))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/suites/{suite_id}",
    tag = "suites",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("suite_id" = Uuid, Path, description = "Suite id")
    ),
    responses((status = 200, description = "Suite", body = Suite))
)]
pub async fn get_suite(
    State(state): State<AppState>,
    Path((project_id, suite_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<Suite>> {
    let suite = sqlx::query_as::<_, Suite>(
        r#"SELECT id, project_id, parent_id, name, sort_order, created_at, updated_at
           FROM suites WHERE project_id = $1 AND id = $2"#,
    )
    .bind(project_id)
    .bind(suite_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("suite".into()))?;
    Ok(Json(suite))
}

#[utoipa::path(
    put,
    path = "/api/v1/projects/{project_id}/suites/{suite_id}",
    tag = "suites",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("suite_id" = Uuid, Path, description = "Suite id")
    ),
    request_body = UpdateSuite,
    responses((status = 200, description = "Updated suite", body = Suite))
)]
pub async fn update_suite(
    State(state): State<AppState>,
    Path((project_id, suite_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateSuite>,
) -> AppResult<Json<Suite>> {
    let existing = sqlx::query_as::<_, Suite>(
        r#"SELECT id, project_id, parent_id, name, sort_order, created_at, updated_at
           FROM suites WHERE project_id = $1 AND id = $2"#,
    )
    .bind(project_id)
    .bind(suite_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("suite".into()))?;

    let name = body.name.unwrap_or(existing.name);
    let parent_id = body.parent_id.or(existing.parent_id);
    let sort_order = body.sort_order.unwrap_or(existing.sort_order);

    let suite = sqlx::query_as::<_, Suite>(
        r#"UPDATE suites
           SET name = $3, parent_id = $4, sort_order = $5, updated_at = NOW()
           WHERE project_id = $1 AND id = $2
           RETURNING id, project_id, parent_id, name, sort_order, created_at, updated_at"#,
    )
    .bind(project_id)
    .bind(suite_id)
    .bind(name)
    .bind(parent_id)
    .bind(sort_order)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(suite))
}

#[utoipa::path(
    delete,
    path = "/api/v1/projects/{project_id}/suites/{suite_id}",
    tag = "suites",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("suite_id" = Uuid, Path, description = "Suite id")
    ),
    responses((status = 204, description = "Deleted"))
)]
pub async fn delete_suite(
    State(state): State<AppState>,
    Path((project_id, suite_id)): Path<(Uuid, Uuid)>,
) -> AppResult<axum::http::StatusCode> {
    let result = sqlx::query("DELETE FROM suites WHERE project_id = $1 AND id = $2")
        .bind(project_id)
        .bind(suite_id)
        .execute(&state.pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("suite".into()));
    }
    Ok(axum::http::StatusCode::NO_CONTENT)
}
