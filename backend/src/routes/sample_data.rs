//! Admin endpoints to inspect, remove, and re-seed sample/demo data.

use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::auth::RequireSystemAdmin;
use crate::error::AppResult;
use crate::models::Project;
use crate::routes::AppState;
use crate::sample_seed::{remove_sample_data, seed_sample_projects_forced, SampleDataSummary};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/admin/sample-data",
            get(get_sample_data).delete(delete_sample_data),
        )
        .route("/api/v1/admin/sample-data/reseed", post(reseed_sample_data))
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SampleDataStatus {
    pub present: bool,
    pub projects: Vec<SampleProjectInfo>,
    pub summary: SampleDataSummary,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SampleProjectInfo {
    pub id: Uuid,
    pub key: String,
    pub name: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ReseedSampleDataRequest {
    /// Locale for seeded strings (defaults to instance default).
    #[serde(default)]
    pub locale: Option<String>,
}

async fn load_status(pool: &sqlx::PgPool) -> AppResult<SampleDataStatus> {
    let projects = sqlx::query_as::<_, Project>(
        r#"SELECT id, key, name, description, case_key_prefix, next_case_number,
                  is_sample, created_at, updated_at
           FROM projects
           WHERE is_sample = TRUE
           ORDER BY key"#,
    )
    .fetch_all(pool)
    .await?;

    let summary = SampleDataSummary::from_pool(pool).await?;
    Ok(SampleDataStatus {
        present: !projects.is_empty() || summary.runs > 0 || summary.campaigns > 0,
        projects: projects
            .into_iter()
            .map(|p| SampleProjectInfo {
                id: p.id,
                key: p.key,
                name: p.name,
            })
            .collect(),
        summary,
    })
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/sample-data",
    tag = "admin",
    responses(
        (status = 200, description = "Sample data status", body = SampleDataStatus),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_sample_data(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
) -> AppResult<Json<SampleDataStatus>> {
    Ok(Json(load_status(&state.pool).await?))
}

#[utoipa::path(
    delete,
    path = "/api/v1/admin/sample-data",
    tag = "admin",
    responses(
        (status = 200, description = "Sample data removed", body = SampleDataSummary),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("bearer_auth" = []))
)]
pub async fn delete_sample_data(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
) -> AppResult<Json<SampleDataSummary>> {
    let summary = remove_sample_data(&state.pool).await?;
    Ok(Json(summary))
}

#[utoipa::path(
    post,
    path = "/api/v1/admin/sample-data/reseed",
    tag = "admin",
    request_body = ReseedSampleDataRequest,
    responses(
        (status = 200, description = "Sample data seeded", body = SampleDataStatus),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ),
    security(("bearer_auth" = []))
)]
pub async fn reseed_sample_data(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
    Json(body): Json<ReseedSampleDataRequest>,
) -> AppResult<Json<SampleDataStatus>> {
    let locale = match body.locale.as_deref() {
        Some(l) if !l.trim().is_empty() => l.trim().to_string(),
        _ => {
            let settings = crate::settings::load_settings(&state.pool).await?;
            settings.default_locale
        }
    };
    seed_sample_projects_forced(&state.pool, &locale).await?;
    Ok(Json(load_status(&state.pool).await?))
}
