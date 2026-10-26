use axum::{extract::State, routing::get, Json, Router};
use serde::Serialize;
use sqlx::PgPool;
use utoipa::ToSchema;

use crate::db;
use crate::error::{AppError, AppResult};
use crate::routes::AppState;

#[derive(Serialize, ToSchema)]
pub struct HealthResponse {
    pub status: &'static str,
    /// Semver from the API crate (`CARGO_PKG_VERSION`).
    pub version: &'static str,
}

#[derive(Serialize, ToSchema)]
pub struct ReadyResponse {
    pub status: &'static str,
    pub database: &'static str,
    pub version: &'static str,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
}

#[utoipa::path(
    get,
    path = "/health",
    tag = "system",
    responses((status = 200, description = "Liveness probe", body = HealthResponse))
)]
pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: db::crate_version(),
    })
}

#[utoipa::path(
    get,
    path = "/ready",
    tag = "system",
    responses(
        (status = 200, description = "Readiness probe", body = ReadyResponse),
        (status = 503, description = "Database unavailable")
    )
)]
pub async fn ready(State(state): State<AppState>) -> AppResult<Json<ReadyResponse>> {
    ping_db(&state.pool).await?;
    Ok(Json(ReadyResponse {
        status: "ok",
        database: "up",
        version: db::crate_version(),
    }))
}

pub async fn ping_db(pool: &PgPool) -> AppResult<()> {
    sqlx::query("SELECT 1")
        .execute(pool)
        .await
        .map_err(AppError::from)?;
    Ok(())
}
