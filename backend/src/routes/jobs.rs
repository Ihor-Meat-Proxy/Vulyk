use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use uuid::Uuid;

use crate::auth::{require_permission, OptionalAuthUser, Permission};
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
    responses((status = 200, description = "Job", body = Job)),
    security(("bearer_auth" = []))
)]
pub async fn get_job(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Job>> {
    let job = jobs_svc::get_job(&state.pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound("job".into()))?;

    // Authorize via project_id in the payload when present. Jobs without a
    // project_id require a system admin (or open mode).
    if let Some(project_id) = job
        .payload
        .get("project_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
    {
        require_permission(
            &state.pool,
            &auth,
            state.auth.required,
            project_id,
            Permission::ViewProject,
        )
        .await?;
    } else if state.auth.required {
        match &auth {
            Some(user) if user.is_system_admin() => {}
            Some(_) => {
                return Err(AppError::Forbidden(
                    "job is not scoped to a project you can view".into(),
                ));
            }
            None => {
                return Err(AppError::Unauthorized("authentication required".into()));
            }
        }
    }

    Ok(Json(job))
}
