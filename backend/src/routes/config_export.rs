use axum::{
    extract::State,
    http::{header, StatusCode},
    routing::post,
    Json, Router,
};

use crate::auth::RequireSystemAdmin;
use crate::error::AppResult;
use crate::models::{
    ConfigBundle, ConfigExportRequest, ConfigImportPreview, ConfigImportRequest,
};
use crate::routes::AppState;
use crate::services::config_bundle;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/admin/config/export", post(export_config))
        .route("/api/v1/admin/config/import", post(import_config))
        .route(
            "/api/v1/admin/config/import/preview",
            post(import_preview),
        )
}

#[utoipa::path(
    post,
    path = "/api/v1/admin/config/export",
    tag = "config",
    request_body = ConfigExportRequest,
    responses(
        (status = 200, description = "Config bundle JSON", body = ConfigBundle),
    ),
    security(("bearer_auth" = []))
)]
pub async fn export_config(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
    Json(req): Json<ConfigExportRequest>,
) -> AppResult<axum::response::Response> {
    let bundle = config_bundle::export(&state.pool, &req).await?;
    let format = req.format.to_lowercase();
    let body = config_bundle::serialize_bundle(&bundle, &format)?;
    let (ctype, ext) = if format == "yaml" || format == "yml" {
        ("application/yaml", "yaml")
    } else {
        ("application/json", "json")
    };
    let mut response = axum::response::Response::new(body.into());
    *response.status_mut() = StatusCode::OK;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        ctype.parse().unwrap(),
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        format!("attachment; filename=\"vulyk-config.{ext}\"")
            .parse()
            .unwrap(),
    );
    Ok(response)
}

#[utoipa::path(
    post,
    path = "/api/v1/admin/config/import/preview",
    tag = "config",
    request_body = ConfigImportRequest,
    responses((status = 200, body = ConfigImportPreview)),
    security(("bearer_auth" = []))
)]
pub async fn import_preview(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
    Json(mut req): Json<ConfigImportRequest>,
) -> AppResult<Json<ConfigImportPreview>> {
    req.dry_run = true;
    Ok(Json(config_bundle::import(&state.pool, req).await?))
}

#[utoipa::path(
    post,
    path = "/api/v1/admin/config/import",
    tag = "config",
    request_body = ConfigImportRequest,
    responses((status = 200, body = ConfigImportPreview)),
    security(("bearer_auth" = []))
)]
pub async fn import_config(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
    Json(req): Json<ConfigImportRequest>,
) -> AppResult<Json<ConfigImportPreview>> {
    Ok(Json(config_bundle::import(&state.pool, req).await?))
}
