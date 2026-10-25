use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use uuid::Uuid;

use crate::auth::{
    require_permission, require_project_role, OptionalAuthUser, Permission, ProjectRole,
};
use crate::error::{AppError, AppResult};
use crate::models::{CreateProject, Project, UpdateProject};
use crate::routes::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/projects", get(list_projects).post(create_project))
        .route(
            "/api/v1/projects/{id}",
            get(get_project).put(update_project).delete(delete_project),
        )
}

#[utoipa::path(
    get,
    path = "/api/v1/projects",
    tag = "projects",
    responses((status = 200, description = "List projects", body = [Project]))
)]
pub async fn list_projects(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
) -> AppResult<Json<Vec<Project>>> {
    // When auth is required and the caller is not a system admin, only show memberships.
    let projects = if state.auth.required {
        match &auth {
            Some(user) if user.is_system_admin() => {
                sqlx::query_as::<_, Project>(
                    r#"SELECT id, key, name, description, created_at, updated_at
                       FROM projects ORDER BY key"#,
                )
                .fetch_all(&state.pool)
                .await?
            }
            Some(user) => {
                sqlx::query_as::<_, Project>(
                    r#"SELECT p.id, p.key, p.name, p.description, p.created_at, p.updated_at
                       FROM projects p
                       INNER JOIN memberships m ON m.project_id = p.id
                       WHERE m.user_id = $1
                       ORDER BY p.key"#,
                )
                .bind(user.id())
                .fetch_all(&state.pool)
                .await?
            }
            None => {
                return Err(AppError::Unauthorized("authentication required".into()));
            }
        }
    } else {
        sqlx::query_as::<_, Project>(
            r#"SELECT id, key, name, description, created_at, updated_at
               FROM projects ORDER BY key"#,
        )
        .fetch_all(&state.pool)
        .await?
    };
    Ok(Json(projects))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects",
    tag = "projects",
    request_body = CreateProject,
    responses(
        (status = 200, description = "Created project", body = Project),
        (status = 400, description = "Invalid request"),
        (status = 409, description = "Key conflict")
    )
)]
pub async fn create_project(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Json(body): Json<CreateProject>,
) -> AppResult<Json<Project>> {
    if state.auth.required && auth.is_none() {
        return Err(AppError::Unauthorized("authentication required".into()));
    }

    let key = body.key.trim().to_uppercase();
    if key.len() < 2 {
        return Err(AppError::BadRequest(
            "project key must be at least 2 characters".into(),
        ));
    }
    if body.name.trim().is_empty() {
        return Err(AppError::BadRequest("name is required".into()));
    }

    let project = sqlx::query_as::<_, Project>(
        r#"INSERT INTO projects (key, name, description)
           VALUES ($1, $2, $3)
           RETURNING id, key, name, description, created_at, updated_at"#,
    )
    .bind(&key)
    .bind(body.name.trim())
    .bind(&body.description)
    .fetch_one(&state.pool)
    .await?;

    // Headless MCP API tokens may authenticate as a synthetic principal (nil id).
    if let Some(user) = auth.filter(|u| u.id() != uuid::Uuid::nil()) {
        sqlx::query(
            r#"INSERT INTO memberships (project_id, user_id, role)
               VALUES ($1, $2, 'admin')
               ON CONFLICT (project_id, user_id) DO NOTHING"#,
        )
        .bind(project.id)
        .bind(user.id())
        .execute(&state.pool)
        .await?;
    }

    Ok(Json(project))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{id}",
    tag = "projects",
    params(("id" = Uuid, Path, description = "Project id")),
    responses(
        (status = 200, description = "Project", body = Project),
        (status = 404, description = "Not found")
    )
)]
pub async fn get_project(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Project>> {
    require_project_role(
        &state.pool,
        &auth,
        state.auth.required,
        id,
        ProjectRole::Viewer,
    )
    .await?;

    let project = sqlx::query_as::<_, Project>(
        r#"SELECT id, key, name, description, created_at, updated_at
           FROM projects WHERE id = $1"#,
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("project".into()))?;
    Ok(Json(project))
}

#[utoipa::path(
    put,
    path = "/api/v1/projects/{id}",
    tag = "projects",
    params(("id" = Uuid, Path, description = "Project id")),
    request_body = UpdateProject,
    responses(
        (status = 200, description = "Updated project", body = Project),
        (status = 404, description = "Not found")
    )
)]
pub async fn update_project(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateProject>,
) -> AppResult<Json<Project>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        id,
        Permission::UpdateProject,
    )
    .await?;

    let existing = sqlx::query_as::<_, Project>(
        r#"SELECT id, key, name, description, created_at, updated_at
           FROM projects WHERE id = $1"#,
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("project".into()))?;

    let name = body.name.unwrap_or(existing.name);
    let description = body.description.unwrap_or(existing.description);

    let project = sqlx::query_as::<_, Project>(
        r#"UPDATE projects
           SET name = $2, description = $3, updated_at = NOW()
           WHERE id = $1
           RETURNING id, key, name, description, created_at, updated_at"#,
    )
    .bind(id)
    .bind(name)
    .bind(description)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(project))
}

#[utoipa::path(
    delete,
    path = "/api/v1/projects/{id}",
    tag = "projects",
    params(("id" = Uuid, Path, description = "Project id")),
    responses(
        (status = 204, description = "Deleted"),
        (status = 404, description = "Not found")
    )
)]
pub async fn delete_project(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<axum::http::StatusCode> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        id,
        Permission::DeleteProject,
    )
    .await?;

    let result = sqlx::query("DELETE FROM projects WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("project".into()));
    }
    Ok(axum::http::StatusCode::NO_CONTENT)
}
