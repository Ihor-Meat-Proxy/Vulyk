use axum::{extract::State, routing::get, Json, Router};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::auth::{role_matrix, RequireSystemAdmin, RoleMatrixRow};
use crate::error::AppResult;
use crate::routes::AppState;
use crate::settings::{self, InstanceSettings};

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/api/v1/admin/settings",
        get(get_settings).put(put_settings),
    )
}

/// Public role matrix (no admin required).
pub fn public_router() -> Router<AppState> {
    Router::new().route("/api/v1/roles/matrix", get(get_role_matrix))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateInstanceSettings {
    pub instance_name: Option<String>,
    pub base_url: Option<String>,
    pub default_locale: Option<String>,
    pub default_timezone: Option<String>,
    pub default_theme: Option<String>,
    pub registration_mode: Option<crate::settings::RegistrationMode>,
    pub default_role: Option<crate::auth::ProjectRole>,
    pub allowed_email_domains: Option<Vec<String>>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RoleMatrixResponse {
    pub roles: Vec<RoleMatrixRow>,
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/settings",
    tag = "admin",
    responses((status = 200, description = "Instance settings", body = InstanceSettings)),
    security(("bearer_auth" = []))
)]
pub async fn get_settings(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
) -> AppResult<Json<InstanceSettings>> {
    Ok(Json(settings::load_settings(&state.pool).await?))
}

#[utoipa::path(
    put,
    path = "/api/v1/admin/settings",
    tag = "admin",
    request_body = UpdateInstanceSettings,
    responses((status = 200, description = "Updated settings", body = InstanceSettings)),
    security(("bearer_auth" = []))
)]
pub async fn put_settings(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
    Json(body): Json<UpdateInstanceSettings>,
) -> AppResult<Json<InstanceSettings>> {
    let mut current = settings::load_settings(&state.pool).await?;
    if let Some(v) = body.instance_name.filter(|s| !s.trim().is_empty()) {
        current.instance_name = v;
    }
    if let Some(v) = body.base_url {
        current.base_url = v;
    }
    if let Some(v) = body.default_locale.filter(|s| !s.is_empty()) {
        current.default_locale = v;
    }
    if let Some(v) = body.default_timezone.filter(|s| !s.is_empty()) {
        current.default_timezone = v;
    }
    if let Some(v) = body.default_theme.filter(|s| !s.is_empty()) {
        current.default_theme = crate::models::preferences::validate_theme(&v)
            .map_err(crate::error::AppError::BadRequest)?;
    }
    if let Some(v) = body.registration_mode {
        current.registration_mode = v;
    }
    if let Some(v) = body.default_role {
        current.default_role = v;
    }
    if let Some(v) = body.allowed_email_domains {
        current.allowed_email_domains = v;
    }
    // Never allow clearing setup_completed via this endpoint.
    settings::save_settings(&state.pool, &current).await?;
    Ok(Json(current))
}

#[utoipa::path(
    get,
    path = "/api/v1/roles/matrix",
    tag = "auth",
    responses((status = 200, description = "Role permission matrix", body = RoleMatrixResponse))
)]
pub async fn get_role_matrix() -> AppResult<Json<RoleMatrixResponse>> {
    Ok(Json(RoleMatrixResponse {
        roles: role_matrix(),
    }))
}
