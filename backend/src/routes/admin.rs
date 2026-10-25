use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use uuid::Uuid;

use crate::auth::RequireSystemAdmin;
use crate::error::{AppError, AppResult};
use crate::models::{IntegrationSetting, UpsertIntegrationSetting};
use crate::routes::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/admin/integrations",
            get(list_integrations).post(create_integration),
        )
        .route(
            "/api/v1/admin/integrations/{id}",
            axum::routing::delete(delete_integration),
        )
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/integrations",
    tag = "admin",
    responses((status = 200, description = "Integration settings placeholders", body = [IntegrationSetting])),
    security(("bearer_auth" = []))
)]
pub async fn list_integrations(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
) -> AppResult<Json<Vec<IntegrationSetting>>> {
    let rows = sqlx::query_as::<_, IntegrationSetting>(
        r#"SELECT id, project_id, kind, name, config, enabled, created_at, updated_at
           FROM integration_settings
           ORDER BY kind, name"#,
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

#[utoipa::path(
    post,
    path = "/api/v1/admin/integrations",
    tag = "admin",
    request_body = UpsertIntegrationSetting,
    responses((status = 200, description = "Created integration setting", body = IntegrationSetting)),
    security(("bearer_auth" = []))
)]
pub async fn create_integration(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
    Json(body): Json<UpsertIntegrationSetting>,
) -> AppResult<Json<IntegrationSetting>> {
    let kind = body.kind.trim();
    let name = body.name.trim();
    if kind.is_empty() || name.is_empty() {
        return Err(AppError::BadRequest("kind and name are required".into()));
    }
    let config = if body.config.is_null() {
        serde_json::json!({})
    } else {
        body.config
    };

    let row = sqlx::query_as::<_, IntegrationSetting>(
        r#"INSERT INTO integration_settings (project_id, kind, name, config, enabled)
           VALUES ($1, $2, $3, $4, $5)
           RETURNING id, project_id, kind, name, config, enabled, created_at, updated_at"#,
    )
    .bind(body.project_id)
    .bind(kind)
    .bind(name)
    .bind(config)
    .bind(body.enabled)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(row))
}

#[utoipa::path(
    delete,
    path = "/api/v1/admin/integrations/{id}",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Integration setting id")),
    responses((status = 204, description = "Deleted")),
    security(("bearer_auth" = []))
)]
pub async fn delete_integration(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
    Path(id): Path<Uuid>,
) -> AppResult<axum::http::StatusCode> {
    let result = sqlx::query("DELETE FROM integration_settings WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("integration setting".into()));
    }
    Ok(axum::http::StatusCode::NO_CONTENT)
}
