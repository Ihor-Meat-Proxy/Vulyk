use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::Job;
use crate::routes::AppState;
use crate::services::jobs as jobs_svc;

pub fn router() -> Router<AppState> {
    Router::new().route("/api/v1/jobs/{id}", get(get_job))
}

#[utoipa::path(
    get,
    path = "/api/v1/jobs/{id}",
    tag = "jobs",
    params(("id" = Uuid, Path, description = "Job id")),
    responses((status = 200, description = "Job", body = Job))
)]
pub async fn get_job(State(state): State<AppState>, Path(id): Path<Uuid>) -> AppResult<Json<Job>> {
    let job = jobs_svc::get_job(&state.pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound("job".into()))?;
    Ok(Json(job))
}
