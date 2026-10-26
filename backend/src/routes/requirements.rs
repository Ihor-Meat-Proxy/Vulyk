use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::{require_permission, OptionalAuthUser, Permission};
use crate::error::{AppError, AppResult};
use crate::models::{
    CaseRequirementLink, CreateRequirement, CreateRequirementFolder, LinkCaseRequirement,
    Requirement, RequirementFolder, UpdateRequirement,
};
use crate::routes::AppState;

const REQ_COLS: &str = "id, project_id, folder_id, external_key, external_id, title, description, \
     acceptance_criteria, source, url, metadata, created_at, updated_at";

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/projects/{project_id}/requirements",
            get(list_requirements).post(create_requirement),
        )
        .route(
            "/api/v1/projects/{project_id}/requirements/{id}",
            get(get_requirement)
                .put(update_requirement)
                .delete(delete_requirement),
        )
        .route(
            "/api/v1/projects/{project_id}/requirements:link",
            axum::routing::post(link_case),
        )
        .route(
            "/api/v1/projects/{project_id}/requirements:unlink",
            axum::routing::post(unlink_case),
        )
        .route(
            "/api/v1/projects/{project_id}/cases/{case_id}/requirements",
            get(list_case_requirements),
        )
        .route(
            "/api/v1/projects/{project_id}/requirement-folders",
            get(list_requirement_folders).post(create_requirement_folder),
        )
}

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub q: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/requirements",
    tag = "requirements",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("q" = Option<String>, Query, description = "Title search")
    ),
    responses((status = 200, description = "List requirements", body = [Requirement])),
    security(("bearer_auth" = []))
)]
pub async fn list_requirements(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<Vec<Requirement>>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    ensure_project(&state, project_id).await?;
    let rows = if let Some(q) = query.q.filter(|s| !s.trim().is_empty()) {
        let pattern = format!("%{}%", q.trim());
        sqlx::query_as::<_, Requirement>(&format!(
            r#"SELECT {REQ_COLS}
               FROM requirements
               WHERE project_id = $1 AND title ILIKE $2
               ORDER BY title"#
        ))
        .bind(project_id)
        .bind(pattern)
        .fetch_all(&state.pool)
        .await?
    } else {
        sqlx::query_as::<_, Requirement>(&format!(
            r#"SELECT {REQ_COLS} FROM requirements WHERE project_id = $1 ORDER BY title"#
        ))
        .bind(project_id)
        .fetch_all(&state.pool)
        .await?
    };
    Ok(Json(rows))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/requirements",
    tag = "requirements",
    params(("project_id" = Uuid, Path, description = "Project id")),
    request_body = CreateRequirement,
    responses((status = 200, description = "Created", body = Requirement)),
    security(("bearer_auth" = []))
)]
pub async fn create_requirement(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<CreateRequirement>,
) -> AppResult<Json<Requirement>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ManageRequirements,
    )
    .await?;
    ensure_project(&state, project_id).await?;
    if body.title.trim().is_empty() {
        return Err(AppError::BadRequest("title is required".into()));
    }
    let metadata = if body.metadata.is_null() {
        serde_json::json!({})
    } else {
        body.metadata
    };
    let row = sqlx::query_as::<_, Requirement>(&format!(
        r#"INSERT INTO requirements
           (project_id, folder_id, external_key, external_id, title, description,
            acceptance_criteria, source, url, metadata)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
           RETURNING {REQ_COLS}"#
    ))
    .bind(project_id)
    .bind(body.folder_id)
    .bind(&body.external_key)
    .bind(&body.external_id)
    .bind(body.title.trim())
    .bind(&body.description)
    .bind(&body.acceptance_criteria)
    .bind(&body.source)
    .bind(&body.url)
    .bind(metadata)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(row))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/requirements/{id}",
    tag = "requirements",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("id" = Uuid, Path, description = "Requirement id")
    ),
    responses((status = 200, body = Requirement), (status = 404, description = "Not found")),
    security(("bearer_auth" = []))
)]
pub async fn get_requirement(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<Requirement>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    let row = sqlx::query_as::<_, Requirement>(&format!(
        r#"SELECT {REQ_COLS} FROM requirements WHERE project_id = $1 AND id = $2"#
    ))
    .bind(project_id)
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("requirement".into()))?;
    Ok(Json(row))
}

#[utoipa::path(
    put,
    path = "/api/v1/projects/{project_id}/requirements/{id}",
    tag = "requirements",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("id" = Uuid, Path, description = "Requirement id")
    ),
    request_body = UpdateRequirement,
    responses((status = 200, body = Requirement)),
    security(("bearer_auth" = []))
)]
pub async fn update_requirement(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, id)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateRequirement>,
) -> AppResult<Json<Requirement>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ManageRequirements,
    )
    .await?;
    let existing = sqlx::query_as::<_, Requirement>(&format!(
        r#"SELECT {REQ_COLS} FROM requirements WHERE project_id = $1 AND id = $2"#
    ))
    .bind(project_id)
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("requirement".into()))?;

    let folder_id = if body.clear_folder {
        None
    } else {
        body.folder_id.or(existing.folder_id)
    };

    let row = sqlx::query_as::<_, Requirement>(&format!(
        r#"UPDATE requirements SET
             title = $3,
             description = $4,
             acceptance_criteria = $5,
             folder_id = $6,
             external_key = $7,
             external_id = $8,
             url = $9,
             metadata = $10,
             updated_at = NOW()
           WHERE project_id = $1 AND id = $2
           RETURNING {REQ_COLS}"#
    ))
    .bind(project_id)
    .bind(id)
    .bind(body.title.unwrap_or(existing.title))
    .bind(body.description.unwrap_or(existing.description))
    .bind(
        body.acceptance_criteria
            .unwrap_or(existing.acceptance_criteria),
    )
    .bind(folder_id)
    .bind(body.external_key.or(existing.external_key))
    .bind(body.external_id.or(existing.external_id))
    .bind(body.url.unwrap_or(existing.url))
    .bind(body.metadata.unwrap_or(existing.metadata))
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(row))
}

#[utoipa::path(
    delete,
    path = "/api/v1/projects/{project_id}/requirements/{id}",
    tag = "requirements",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("id" = Uuid, Path, description = "Requirement id")
    ),
    responses((status = 204, description = "Deleted")),
    security(("bearer_auth" = []))
)]
pub async fn delete_requirement(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, id)): Path<(Uuid, Uuid)>,
) -> AppResult<axum::http::StatusCode> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ManageRequirements,
    )
    .await?;
    let result = sqlx::query("DELETE FROM requirements WHERE project_id = $1 AND id = $2")
        .bind(project_id)
        .bind(id)
        .execute(&state.pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("requirement".into()));
    }
    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/requirements:link",
    tag = "requirements",
    params(("project_id" = Uuid, Path, description = "Project id")),
    request_body = LinkCaseRequirement,
    responses((status = 200, body = CaseRequirementLink)),
    security(("bearer_auth" = []))
)]
pub async fn link_case(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<LinkCaseRequirement>,
) -> AppResult<Json<CaseRequirementLink>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ManageRequirements,
    )
    .await?;
    ensure_project(&state, project_id).await?;
    let case_ok: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM test_cases WHERE id = $1 AND project_id = $2")
            .bind(body.case_id)
            .bind(project_id)
            .fetch_optional(&state.pool)
            .await?;
    if case_ok.is_none() {
        return Err(AppError::NotFound("case".into()));
    }
    let req_ok: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM requirements WHERE id = $1 AND project_id = $2")
            .bind(body.requirement_id)
            .bind(project_id)
            .fetch_optional(&state.pool)
            .await?;
    if req_ok.is_none() {
        return Err(AppError::NotFound("requirement".into()));
    }

    let link = sqlx::query_as::<_, CaseRequirementLink>(
        r#"INSERT INTO case_requirements (case_id, requirement_id)
           VALUES ($1, $2)
           ON CONFLICT (case_id, requirement_id) DO UPDATE SET case_id = EXCLUDED.case_id
           RETURNING case_id, requirement_id, created_at"#,
    )
    .bind(body.case_id)
    .bind(body.requirement_id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(link))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/requirements:unlink",
    tag = "requirements",
    params(("project_id" = Uuid, Path, description = "Project id")),
    request_body = LinkCaseRequirement,
    responses((status = 204, description = "Unlinked")),
    security(("bearer_auth" = []))
)]
pub async fn unlink_case(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<LinkCaseRequirement>,
) -> AppResult<axum::http::StatusCode> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ManageRequirements,
    )
    .await?;
    ensure_project(&state, project_id).await?;
    sqlx::query(
        r#"DELETE FROM case_requirements cr
           USING test_cases c
           WHERE cr.case_id = c.id AND c.project_id = $1
             AND cr.case_id = $2 AND cr.requirement_id = $3"#,
    )
    .bind(project_id)
    .bind(body.case_id)
    .bind(body.requirement_id)
    .execute(&state.pool)
    .await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/cases/{case_id}/requirements",
    tag = "requirements",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("case_id" = Uuid, Path, description = "Case id")
    ),
    responses((status = 200, body = [Requirement])),
    security(("bearer_auth" = []))
)]
pub async fn list_case_requirements(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, case_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<Vec<Requirement>>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    let rows = sqlx::query_as::<_, Requirement>(
        r#"SELECT r.id, r.project_id, r.folder_id, r.external_key, r.external_id, r.title,
                  r.description, r.acceptance_criteria, r.source, r.url, r.metadata,
                  r.created_at, r.updated_at
           FROM requirements r
           JOIN case_requirements cr ON cr.requirement_id = r.id
           JOIN test_cases c ON c.id = cr.case_id
           WHERE r.project_id = $1 AND c.id = $2
           ORDER BY r.title"#,
    )
    .bind(project_id)
    .bind(case_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/requirement-folders",
    tag = "requirements",
    params(("project_id" = Uuid, Path, description = "Project id")),
    responses((status = 200, body = [RequirementFolder])),
    security(("bearer_auth" = []))
)]
pub async fn list_requirement_folders(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
) -> AppResult<Json<Vec<RequirementFolder>>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    ensure_project(&state, project_id).await?;
    let rows = sqlx::query_as::<_, RequirementFolderRow>(
        r#"SELECT f.id, f.project_id, f.parent_id, f.name, f.sort_order, f.created_at, f.updated_at,
                  (SELECT COUNT(*)::bigint FROM requirements r WHERE r.folder_id = f.id) AS requirement_count
           FROM requirement_folders f
           WHERE f.project_id = $1
           ORDER BY f.sort_order, f.name"#,
    )
    .bind(project_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|r| RequirementFolder {
                id: r.id,
                project_id: r.project_id,
                parent_id: r.parent_id,
                name: r.name,
                sort_order: r.sort_order,
                requirement_count: r.requirement_count,
                created_at: r.created_at,
                updated_at: r.updated_at,
            })
            .collect(),
    ))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/requirement-folders",
    tag = "requirements",
    params(("project_id" = Uuid, Path, description = "Project id")),
    request_body = CreateRequirementFolder,
    responses((status = 200, body = RequirementFolder)),
    security(("bearer_auth" = []))
)]
pub async fn create_requirement_folder(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<CreateRequirementFolder>,
) -> AppResult<Json<RequirementFolder>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ManageRequirements,
    )
    .await?;
    ensure_project(&state, project_id).await?;
    if body.name.trim().is_empty() {
        return Err(AppError::BadRequest("name is required".into()));
    }
    let row = sqlx::query_as::<_, RequirementFolderRow>(
        r#"INSERT INTO requirement_folders (project_id, parent_id, name, sort_order)
           VALUES ($1, $2, $3, $4)
           RETURNING id, project_id, parent_id, name, sort_order, created_at, updated_at, 0::bigint AS requirement_count"#,
    )
    .bind(project_id)
    .bind(body.parent_id)
    .bind(body.name.trim())
    .bind(body.sort_order)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(RequirementFolder {
        id: row.id,
        project_id: row.project_id,
        parent_id: row.parent_id,
        name: row.name,
        sort_order: row.sort_order,
        requirement_count: row.requirement_count,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }))
}

#[derive(Debug, sqlx::FromRow)]
struct RequirementFolderRow {
    id: Uuid,
    project_id: Uuid,
    parent_id: Option<Uuid>,
    name: String,
    sort_order: i32,
    requirement_count: i64,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
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
