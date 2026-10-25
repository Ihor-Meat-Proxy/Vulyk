use axum::{extract::State, routing::{get, post}, Json, Router};

use crate::auth::validate_password_strength;
use crate::error::{AppError, AppResult};
use crate::models::{CompleteSetupRequest, SetupStatus};
use crate::routes::AppState;
use crate::seed;
use crate::settings::{self, RegistrationMode};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/setup/status", get(setup_status))
        .route("/api/v1/setup/complete", post(setup_complete))
}

#[utoipa::path(
    get,
    path = "/api/v1/setup/status",
    tag = "auth",
    responses((status = 200, description = "Setup wizard status", body = SetupStatus))
)]
pub async fn setup_status(State(state): State<AppState>) -> AppResult<Json<SetupStatus>> {
    let completed = settings::get_bool(&state.pool, "setup_completed", false).await?;
    Ok(Json(SetupStatus {
        setup_completed: completed,
        setup_required: !completed,
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/setup/complete",
    tag = "auth",
    request_body = CompleteSetupRequest,
    responses(
        (status = 200, description = "Setup completed"),
        (status = 403, description = "Invalid setup token"),
        (status = 409, description = "Already completed")
    )
)]
pub async fn setup_complete(
    State(state): State<AppState>,
    Json(body): Json<CompleteSetupRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let email = body.email.trim().to_lowercase();
    if !email.contains('@') {
        return Err(AppError::BadRequest("valid email is required".into()));
    }
    validate_password_strength(&body.password)?;
    let display_name = body.display_name.trim();
    if display_name.is_empty() {
        return Err(AppError::BadRequest("display_name is required".into()));
    }
    if body.setup_token.trim().is_empty() {
        return Err(AppError::BadRequest("setup_token is required".into()));
    }

    let mut instance = settings::load_settings(&state.pool).await?;
    if let Some(name) = body.instance_name.filter(|s| !s.trim().is_empty()) {
        instance.instance_name = name;
    }
    if let Some(url) = body.base_url {
        instance.base_url = url;
    }
    if let Some(locale) = body.default_locale.filter(|s| !s.is_empty()) {
        instance.default_locale = locale;
    }
    if let Some(tz) = body.default_timezone.filter(|s| !s.is_empty()) {
        instance.default_timezone = tz;
    }
    if let Some(mode) = body.registration_mode {
        instance.registration_mode = mode;
    } else if instance.registration_mode == RegistrationMode::Disabled {
        // leave default
    }

    seed::complete_setup(
        &state.pool,
        &email,
        &body.password,
        display_name,
        &instance,
        Some(body.setup_token.trim()),
    )
    .await?;

    Ok(Json(serde_json::json!({ "ok": true, "setup_completed": true })))
}
