use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use uuid::Uuid;

use crate::auth::{
    require_permission, require_project_role, AuthVia, OptionalAuthUser, Permission, ProjectRole,
};
use crate::error::{AppError, AppResult};
use crate::models::{CreateProject, Project, UpdateProject};
use crate::routes::AppState;
use crate::services::case_keys::{
    unique_key_for_name, unique_prefix, unique_prefix_for_name, validate_prefix,
    validate_project_key,
};

const PROJECT_COLS: &str =
    "id, key, name, description, case_key_prefix, next_case_number, is_sample, created_at, updated_at";

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
    let projects = if state.auth.required {
        match &auth {
            Some(user) if user.is_system_admin() => {
                sqlx::query_as::<_, Project>(&format!(
                    "SELECT {PROJECT_COLS} FROM projects ORDER BY key"
                ))
                .fetch_all(&state.pool)
                .await?
            }
            Some(user) => {
                sqlx::query_as::<_, Project>(
                    r#"SELECT p.id, p.key, p.name, p.description, p.case_key_prefix,
                              p.next_case_number, p.is_sample, p.created_at, p.updated_at
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
        sqlx::query_as::<_, Project>(&format!("SELECT {PROJECT_COLS} FROM projects ORDER BY key"))
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
    // Project-scoped CI tokens must not create projects.
    if let Some(user) = &auth {
        if matches!(user.via, AuthVia::CiToken { .. }) {
            return Err(AppError::Forbidden(
                "CI tokens cannot create projects".into(),
            ));
        }
    }

    if body.name.trim().is_empty() {
        return Err(AppError::BadRequest("name is required".into()));
    }

    let explicit_key = body
        .key
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(validate_project_key)
        .transpose()?;
    let prefix_seed = body
        .case_key_prefix
        .as_ref()
        .map(|s| validate_prefix(s))
        .transpose()?;

    // Allocate then INSERT; on unique violation re-allocate and retry so concurrent
    // creates get a clean Conflict/retry instead of an opaque DB race (review #5).
    const MAX_INSERT_RETRIES: u32 = 8;
    let mut last_conflict: Option<String> = None;
    for attempt in 0..MAX_INSERT_RETRIES {
        let key = if let Some(ref k) = explicit_key {
            k.clone()
        } else {
            unique_key_for_name(&state.pool, body.name.trim()).await?
        };
        let prefix = if let Some(ref raw) = prefix_seed {
            unique_prefix(&state.pool, raw, None).await?
        } else {
            unique_prefix_for_name(&state.pool, body.name.trim()).await?
        };

        let inserted = sqlx::query_as::<_, Project>(&format!(
            r#"INSERT INTO projects (key, name, description, case_key_prefix, next_case_number)
               VALUES ($1, $2, $3, $4, 0)
               RETURNING {PROJECT_COLS}"#
        ))
        .bind(&key)
        .bind(body.name.trim())
        .bind(&body.description)
        .bind(&prefix)
        .fetch_one(&state.pool)
        .await;

        let project = match inserted {
            Ok(p) => p,
            Err(sqlx::Error::Database(ref db)) if db.constraint().is_some() => {
                let constraint = db.constraint().unwrap_or("projects").to_string();
                last_conflict = Some(constraint.clone());
                if explicit_key.is_some()
                    && (constraint.contains("key") || constraint == "projects_pkey")
                {
                    return Err(AppError::Conflict(format!(
                        "project key '{key}' already exists"
                    )));
                }
                // Auto key/prefix: another writer won the race — re-read uniqueness and retry.
                tracing::debug!(
                    attempt,
                    %constraint,
                    %key,
                    %prefix,
                    "project create unique conflict; retrying allocation"
                );
                continue;
            }
            Err(e) => return Err(e.into()),
        };

        if let Some(user) = auth.as_ref().filter(|u| u.id() != uuid::Uuid::nil()) {
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

        return Ok(Json(project));
    }

    Err(AppError::Conflict(format!(
        "unable to allocate unique project key/prefix after retries ({})",
        last_conflict.unwrap_or_else(|| "unique".into())
    )))
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

    let project = sqlx::query_as::<_, Project>(&format!(
        "SELECT {PROJECT_COLS} FROM projects WHERE id = $1"
    ))
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

    let existing = sqlx::query_as::<_, Project>(&format!(
        "SELECT {PROJECT_COLS} FROM projects WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("project".into()))?;

    let name = body.name.unwrap_or(existing.name);
    let description = body.description.unwrap_or(existing.description);

    let case_key_prefix = if let Some(raw) = body.case_key_prefix {
        let case_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*)::bigint FROM test_cases WHERE project_id = $1")
                .bind(id)
                .fetch_one(&state.pool)
                .await?;
        let tombstone_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)::bigint FROM case_key_tombstones WHERE project_id = $1",
        )
        .bind(id)
        .fetch_one(&state.pool)
        .await?;
        if case_count > 0 || tombstone_count > 0 || existing.next_case_number > 0 {
            return Err(AppError::BadRequest(
                "case_key_prefix can only be changed before any cases exist".into(),
            ));
        }
        let validated = validate_prefix(&raw)?;
        unique_prefix(&state.pool, &validated, Some(id)).await?
    } else {
        existing.case_key_prefix
    };

    let project = sqlx::query_as::<_, Project>(&format!(
        r#"UPDATE projects
           SET name = $2, description = $3, case_key_prefix = $4, updated_at = NOW()
           WHERE id = $1
           RETURNING {PROJECT_COLS}"#
    ))
    .bind(id)
    .bind(name)
    .bind(description)
    .bind(case_key_prefix)
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
