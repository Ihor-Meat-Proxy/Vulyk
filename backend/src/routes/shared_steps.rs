use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use uuid::Uuid;

use crate::auth::{require_permission, OptionalAuthUser, Permission};
use crate::error::AppResult;
use crate::models::{CreateSharedStep, SharedStepDetail, SharedStepSummary, UpdateSharedStep};
use crate::routes::AppState;
use crate::services::cases::ensure_project;
use crate::services::shared_steps;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/projects/{project_id}/shared-steps",
            get(list_shared_steps).post(create_shared_step),
        )
        .route(
            "/api/v1/projects/{project_id}/shared-steps/{shared_step_id}",
            get(get_shared_step)
                .put(update_shared_step)
                .delete(delete_shared_step),
        )
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/shared-steps",
    tag = "shared_steps",
    params(("project_id" = Uuid, Path)),
    responses((status = 200, description = "Shared steps with usage counts", body = [SharedStepSummary])),
    security(("bearer_auth" = []))
)]
pub async fn list_shared_steps(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
) -> AppResult<Json<Vec<SharedStepSummary>>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    ensure_project(&state.pool, project_id).await?;
    Ok(Json(shared_steps::list(&state.pool, project_id).await?))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/shared-steps",
    tag = "shared_steps",
    params(("project_id" = Uuid, Path)),
    request_body = CreateSharedStep,
    responses(
        (status = 200, description = "Created", body = SharedStepDetail),
        (status = 409, description = "Name already used in this project"),
    ),
    security(("bearer_auth" = []))
)]
pub async fn create_shared_step(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<CreateSharedStep>,
) -> AppResult<Json<SharedStepDetail>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::CreateEditCases,
    )
    .await?;
    ensure_project(&state.pool, project_id).await?;
    Ok(Json(
        shared_steps::create(&state.pool, project_id, &body).await?,
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/shared-steps/{shared_step_id}",
    tag = "shared_steps",
    params(("project_id" = Uuid, Path), ("shared_step_id" = Uuid, Path)),
    responses((status = 200, description = "Shared step with lines", body = SharedStepDetail)),
    security(("bearer_auth" = []))
)]
pub async fn get_shared_step(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, shared_step_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<SharedStepDetail>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    Ok(Json(
        shared_steps::get_detail(&state.pool, project_id, shared_step_id).await?,
    ))
}

#[utoipa::path(
    put,
    path = "/api/v1/projects/{project_id}/shared-steps/{shared_step_id}",
    tag = "shared_steps",
    params(("project_id" = Uuid, Path), ("shared_step_id" = Uuid, Path)),
    request_body = UpdateSharedStep,
    responses(
        (status = 200, description = "Updated", body = SharedStepDetail),
        (status = 409, description = "Name already used in this project"),
    ),
    security(("bearer_auth" = []))
)]
pub async fn update_shared_step(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, shared_step_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateSharedStep>,
) -> AppResult<Json<SharedStepDetail>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::CreateEditCases,
    )
    .await?;
    Ok(Json(
        shared_steps::update(&state.pool, project_id, shared_step_id, &body).await?,
    ))
}

#[utoipa::path(
    delete,
    path = "/api/v1/projects/{project_id}/shared-steps/{shared_step_id}",
    tag = "shared_steps",
    params(("project_id" = Uuid, Path), ("shared_step_id" = Uuid, Path)),
    responses(
        (status = 204, description = "Deleted"),
        (status = 409, description = "Shared step is still used by one or more cases"),
    ),
    security(("bearer_auth" = []))
)]
pub async fn delete_shared_step(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, shared_step_id)): Path<(Uuid, Uuid)>,
) -> AppResult<axum::http::StatusCode> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::CreateEditCases,
    )
    .await?;
    shared_steps::delete(&state.pool, project_id, shared_step_id).await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}
