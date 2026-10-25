use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use uuid::Uuid;

use crate::auth::{require_permission, OptionalAuthUser, Permission};
use crate::error::{AppError, AppResult};
use crate::models::{CreateCustomField, CustomField, UpdateCustomField};
use crate::routes::AppState;
use crate::services::cases::ensure_project;

const ALLOWED_TYPES: &[&str] = &[
    "text",
    "number",
    "boolean",
    "select",
    "multi_select",
    "date",
];

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/projects/{project_id}/custom-fields",
            get(list_fields).post(create_field),
        )
        .route(
            "/api/v1/projects/{project_id}/custom-fields/{field_id}",
            get(get_field).put(update_field).delete(delete_field),
        )
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/custom-fields",
    tag = "custom_fields",
    params(("project_id" = Uuid, Path)),
    responses((status = 200, description = "Custom fields", body = [CustomField])),
    security(("bearer_auth" = []))
)]
pub async fn list_fields(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
) -> AppResult<Json<Vec<CustomField>>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    ensure_project(&state.pool, project_id).await?;
    let fields = sqlx::query_as::<_, CustomField>(
        r#"SELECT id, project_id, name, field_type, options, required, sort_order, created_at
           FROM custom_fields WHERE project_id = $1
           ORDER BY sort_order, name"#,
    )
    .bind(project_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(fields))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/custom-fields",
    tag = "custom_fields",
    params(("project_id" = Uuid, Path)),
    request_body = CreateCustomField,
    responses((status = 200, description = "Created", body = CustomField)),
    security(("bearer_auth" = []))
)]
pub async fn create_field(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<CreateCustomField>,
) -> AppResult<Json<CustomField>> {
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
    if !ALLOWED_TYPES.contains(&body.field_type.as_str()) {
        return Err(AppError::BadRequest("invalid field_type".into()));
    }
    let field = sqlx::query_as::<_, CustomField>(
        r#"INSERT INTO custom_fields
             (project_id, name, field_type, options, required, sort_order)
           VALUES ($1, $2, $3, $4, $5, $6)
           RETURNING id, project_id, name, field_type, options, required, sort_order, created_at"#,
    )
    .bind(project_id)
    .bind(name)
    .bind(&body.field_type)
    .bind(&body.options)
    .bind(body.required)
    .bind(body.sort_order)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(field))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/custom-fields/{field_id}",
    tag = "custom_fields",
    params(("project_id" = Uuid, Path), ("field_id" = Uuid, Path)),
    responses((status = 200, description = "Field", body = CustomField)),
    security(("bearer_auth" = []))
)]
pub async fn get_field(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, field_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<CustomField>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    let field = sqlx::query_as::<_, CustomField>(
        r#"SELECT id, project_id, name, field_type, options, required, sort_order, created_at
           FROM custom_fields WHERE project_id = $1 AND id = $2"#,
    )
    .bind(project_id)
    .bind(field_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("custom field".into()))?;
    Ok(Json(field))
}

#[utoipa::path(
    put,
    path = "/api/v1/projects/{project_id}/custom-fields/{field_id}",
    tag = "custom_fields",
    params(("project_id" = Uuid, Path), ("field_id" = Uuid, Path)),
    request_body = UpdateCustomField,
    responses((status = 200, description = "Updated", body = CustomField)),
    security(("bearer_auth" = []))
)]
pub async fn update_field(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, field_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateCustomField>,
) -> AppResult<Json<CustomField>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::CreateEditCases,
    )
    .await?;
    let existing = sqlx::query_as::<_, CustomField>(
        r#"SELECT id, project_id, name, field_type, options, required, sort_order, created_at
           FROM custom_fields WHERE project_id = $1 AND id = $2"#,
    )
    .bind(project_id)
    .bind(field_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("custom field".into()))?;

    let name = body.name.unwrap_or(existing.name);
    let field_type = body.field_type.unwrap_or(existing.field_type);
    if !ALLOWED_TYPES.contains(&field_type.as_str()) {
        return Err(AppError::BadRequest("invalid field_type".into()));
    }
    let options = body.options.unwrap_or(existing.options);
    let required = body.required.unwrap_or(existing.required);
    let sort_order = body.sort_order.unwrap_or(existing.sort_order);

    let field = sqlx::query_as::<_, CustomField>(
        r#"UPDATE custom_fields
           SET name = $3, field_type = $4, options = $5, required = $6, sort_order = $7
           WHERE project_id = $1 AND id = $2
           RETURNING id, project_id, name, field_type, options, required, sort_order, created_at"#,
    )
    .bind(project_id)
    .bind(field_id)
    .bind(name.trim())
    .bind(field_type)
    .bind(options)
    .bind(required)
    .bind(sort_order)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(field))
}

#[utoipa::path(
    delete,
    path = "/api/v1/projects/{project_id}/custom-fields/{field_id}",
    tag = "custom_fields",
    params(("project_id" = Uuid, Path), ("field_id" = Uuid, Path)),
    responses((status = 204, description = "Deleted")),
    security(("bearer_auth" = []))
)]
pub async fn delete_field(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, field_id)): Path<(Uuid, Uuid)>,
) -> AppResult<axum::http::StatusCode> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::CreateEditCases,
    )
    .await?;
    let res = sqlx::query("DELETE FROM custom_fields WHERE project_id = $1 AND id = $2")
        .bind(project_id)
        .bind(field_id)
        .execute(&state.pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound("custom field".into()));
    }
    Ok(axum::http::StatusCode::NO_CONTENT)
}
