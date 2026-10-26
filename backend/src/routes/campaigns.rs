//! Campaigns / iterations / automated suites HTTP surface.

use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::auth::{require_permission, OptionalAuthUser, Permission};
use crate::error::{AppError, AppResult};
use crate::ingest::parse_payload;
use crate::models::{
    AutomatedSuite, AutomatedSuitesQuery, Campaign, CaseExecutionHistoryPage, CreateCampaign,
    CreateIteration, ExecutionsTreeNode, IngestRequest, IngestResponse, Iteration,
    IterationDashboard, IterationItemView, Page, PipelineIngestRequest, PipelineIngestResponse,
    SuiteResultRow, UpdateCampaign, UpdateIteration,
};
use crate::routes::AppState;
use crate::services::campaigns as camp;
use crate::services::cases::ensure_project;
use crate::services::{ingest as ingest_svc, jobs as jobs_svc};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/projects/{project_id}/executions/tree",
            get(executions_tree),
        )
        .route(
            "/api/v1/projects/{project_id}/campaigns",
            get(list_campaigns).post(create_campaign),
        )
        .route(
            "/api/v1/projects/{project_id}/campaigns/{campaign_id}",
            get(get_campaign)
                .put(update_campaign)
                .delete(delete_campaign),
        )
        .route(
            "/api/v1/projects/{project_id}/iterations",
            get(list_iterations).post(create_iteration_top),
        )
        .route(
            "/api/v1/projects/{project_id}/campaigns/{campaign_id}/iterations",
            get(list_campaign_iterations).post(create_iteration),
        )
        .route(
            "/api/v1/projects/{project_id}/iterations/{iteration_id}",
            get(get_iteration)
                .put(update_iteration)
                .delete(delete_iteration),
        )
        .route(
            "/api/v1/projects/{project_id}/iterations/{iteration_id}/dashboard",
            get(iteration_dashboard),
        )
        .route(
            "/api/v1/projects/{project_id}/iterations/{iteration_id}/items",
            get(list_iteration_items),
        )
        .route(
            "/api/v1/projects/{project_id}/iterations/{iteration_id}/suites",
            get(list_suites),
        )
        .route(
            "/api/v1/projects/{project_id}/suites-runs/{run_id}/results",
            get(list_suite_results),
        )
        .route(
            "/api/v1/projects/{project_id}/suites-runs/{run_id}",
            axum::routing::delete(delete_suite_run).post(rerun_suite),
        )
        .route(
            "/api/v1/projects/{project_id}/cases/{case_id}/executions",
            get(case_executions),
        )
        .route(
            "/api/v1/projects/{project_id}/ingest",
            post(pipeline_ingest),
        )
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/executions/tree",
    tag = "executions",
    params(("project_id" = Uuid, Path)),
    responses((status = 200, description = "Campaign/iteration tree", body = [ExecutionsTreeNode])),
    security(("bearer_auth" = []))
)]
pub async fn executions_tree(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
) -> AppResult<Json<Vec<ExecutionsTreeNode>>> {
    ensure_project(&state.pool, project_id).await?;
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    Ok(Json(camp::executions_tree(&state.pool, project_id).await?))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/campaigns",
    tag = "executions",
    params(("project_id" = Uuid, Path)),
    responses((status = 200, body = [Campaign])),
    security(("bearer_auth" = []))
)]
pub async fn list_campaigns(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
) -> AppResult<Json<Vec<Campaign>>> {
    ensure_project(&state.pool, project_id).await?;
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    Ok(Json(camp::list_campaigns(&state.pool, project_id).await?))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/campaigns",
    tag = "executions",
    params(("project_id" = Uuid, Path)),
    request_body = CreateCampaign,
    responses((status = 200, body = Campaign)),
    security(("bearer_auth" = []))
)]
pub async fn create_campaign(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<CreateCampaign>,
) -> AppResult<Json<Campaign>> {
    ensure_project(&state.pool, project_id).await?;
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ManagePlans,
    )
    .await?;
    Ok(Json(
        camp::create_campaign(&state.pool, project_id, &body).await?,
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/campaigns/{campaign_id}",
    tag = "executions",
    params(("project_id" = Uuid, Path), ("campaign_id" = Uuid, Path)),
    responses((status = 200, body = Campaign)),
    security(("bearer_auth" = []))
)]
pub async fn get_campaign(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, campaign_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<Campaign>> {
    ensure_project(&state.pool, project_id).await?;
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    Ok(Json(
        camp::get_campaign(&state.pool, project_id, campaign_id).await?,
    ))
}

#[utoipa::path(
    put,
    path = "/api/v1/projects/{project_id}/campaigns/{campaign_id}",
    tag = "executions",
    params(("project_id" = Uuid, Path), ("campaign_id" = Uuid, Path)),
    request_body = UpdateCampaign,
    responses((status = 200, body = Campaign)),
    security(("bearer_auth" = []))
)]
pub async fn update_campaign(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, campaign_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateCampaign>,
) -> AppResult<Json<Campaign>> {
    ensure_project(&state.pool, project_id).await?;
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ManagePlans,
    )
    .await?;
    Ok(Json(
        camp::update_campaign(&state.pool, project_id, campaign_id, &body).await?,
    ))
}

#[utoipa::path(
    delete,
    path = "/api/v1/projects/{project_id}/campaigns/{campaign_id}",
    tag = "executions",
    params(("project_id" = Uuid, Path), ("campaign_id" = Uuid, Path)),
    responses((status = 204, description = "Deleted")),
    security(("bearer_auth" = []))
)]
pub async fn delete_campaign(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, campaign_id)): Path<(Uuid, Uuid)>,
) -> AppResult<axum::http::StatusCode> {
    ensure_project(&state.pool, project_id).await?;
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ManagePlans,
    )
    .await?;
    camp::delete_campaign(&state.pool, project_id, campaign_id).await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
pub struct ListIterationsQuery {
    pub campaign_id: Option<Uuid>,
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/iterations",
    tag = "executions",
    params(("project_id" = Uuid, Path)),
    responses((status = 200, body = [Iteration])),
    security(("bearer_auth" = []))
)]
pub async fn list_iterations(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
    Query(q): Query<ListIterationsQuery>,
) -> AppResult<Json<Vec<Iteration>>> {
    ensure_project(&state.pool, project_id).await?;
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    Ok(Json(
        camp::list_iterations(&state.pool, project_id, q.campaign_id).await?,
    ))
}

#[derive(Debug, Deserialize)]
pub struct CreateIterationBody {
    pub campaign_id: Uuid,
    #[serde(flatten)]
    pub iteration: CreateIteration,
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/iterations",
    tag = "executions",
    params(("project_id" = Uuid, Path)),
    request_body = CreateIteration,
    responses((status = 200, body = Iteration)),
    security(("bearer_auth" = []))
)]
pub async fn create_iteration_top(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<CreateIterationBody>,
) -> AppResult<Json<Iteration>> {
    ensure_project(&state.pool, project_id).await?;
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ManagePlans,
    )
    .await?;
    Ok(Json(
        camp::create_iteration(&state.pool, project_id, body.campaign_id, &body.iteration).await?,
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/campaigns/{campaign_id}/iterations",
    tag = "executions",
    params(("project_id" = Uuid, Path), ("campaign_id" = Uuid, Path)),
    responses((status = 200, body = [Iteration])),
    security(("bearer_auth" = []))
)]
pub async fn list_campaign_iterations(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, campaign_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<Vec<Iteration>>> {
    ensure_project(&state.pool, project_id).await?;
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    Ok(Json(
        camp::list_iterations(&state.pool, project_id, Some(campaign_id)).await?,
    ))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/campaigns/{campaign_id}/iterations",
    tag = "executions",
    params(("project_id" = Uuid, Path), ("campaign_id" = Uuid, Path)),
    request_body = CreateIteration,
    responses((status = 200, body = Iteration)),
    security(("bearer_auth" = []))
)]
pub async fn create_iteration(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, campaign_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<CreateIteration>,
) -> AppResult<Json<Iteration>> {
    ensure_project(&state.pool, project_id).await?;
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ManagePlans,
    )
    .await?;
    Ok(Json(
        camp::create_iteration(&state.pool, project_id, campaign_id, &body).await?,
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/iterations/{iteration_id}",
    tag = "executions",
    params(("project_id" = Uuid, Path), ("iteration_id" = Uuid, Path)),
    responses((status = 200, body = Iteration)),
    security(("bearer_auth" = []))
)]
pub async fn get_iteration(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, iteration_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<Iteration>> {
    ensure_project(&state.pool, project_id).await?;
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    Ok(Json(
        camp::get_iteration(&state.pool, project_id, iteration_id).await?,
    ))
}

#[utoipa::path(
    put,
    path = "/api/v1/projects/{project_id}/iterations/{iteration_id}",
    tag = "executions",
    params(("project_id" = Uuid, Path), ("iteration_id" = Uuid, Path)),
    request_body = UpdateIteration,
    responses((status = 200, body = Iteration)),
    security(("bearer_auth" = []))
)]
pub async fn update_iteration(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, iteration_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateIteration>,
) -> AppResult<Json<Iteration>> {
    ensure_project(&state.pool, project_id).await?;
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ManagePlans,
    )
    .await?;
    Ok(Json(
        camp::update_iteration(&state.pool, project_id, iteration_id, &body).await?,
    ))
}

#[utoipa::path(
    delete,
    path = "/api/v1/projects/{project_id}/iterations/{iteration_id}",
    tag = "executions",
    params(("project_id" = Uuid, Path), ("iteration_id" = Uuid, Path)),
    responses((status = 204, description = "Deleted")),
    security(("bearer_auth" = []))
)]
pub async fn delete_iteration(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, iteration_id)): Path<(Uuid, Uuid)>,
) -> AppResult<axum::http::StatusCode> {
    ensure_project(&state.pool, project_id).await?;
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ManagePlans,
    )
    .await?;
    camp::delete_iteration(&state.pool, project_id, iteration_id).await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/iterations/{iteration_id}/dashboard",
    tag = "executions",
    params(("project_id" = Uuid, Path), ("iteration_id" = Uuid, Path)),
    responses((status = 200, body = IterationDashboard)),
    security(("bearer_auth" = []))
)]
pub async fn iteration_dashboard(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, iteration_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<IterationDashboard>> {
    ensure_project(&state.pool, project_id).await?;
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    Ok(Json(
        camp::iteration_dashboard(&state.pool, project_id, iteration_id).await?,
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/iterations/{iteration_id}/items",
    tag = "executions",
    params(("project_id" = Uuid, Path), ("iteration_id" = Uuid, Path)),
    responses((status = 200, body = [IterationItemView])),
    security(("bearer_auth" = []))
)]
pub async fn list_iteration_items(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, iteration_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<Vec<IterationItemView>>> {
    ensure_project(&state.pool, project_id).await?;
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    camp::get_iteration(&state.pool, project_id, iteration_id).await?;
    Ok(Json(
        camp::list_iteration_items(&state.pool, iteration_id).await?,
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/iterations/{iteration_id}/suites",
    tag = "executions",
    params(("project_id" = Uuid, Path), ("iteration_id" = Uuid, Path)),
    responses((status = 200, body = Page<AutomatedSuite>)),
    security(("bearer_auth" = []))
)]
pub async fn list_suites(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, iteration_id)): Path<(Uuid, Uuid)>,
    Query(q): Query<AutomatedSuitesQuery>,
) -> AppResult<Json<Page<AutomatedSuite>>> {
    ensure_project(&state.pool, project_id).await?;
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    camp::get_iteration(&state.pool, project_id, iteration_id).await?;
    let (items, total) = camp::list_automated_suites(
        &state.pool,
        project_id,
        iteration_id,
        q.status.as_deref(),
        q.created_by.as_deref(),
        q.q.as_deref(),
        q.limit,
        q.offset,
        &q.sort,
    )
    .await?;
    Ok(Json(Page {
        items,
        total,
        limit: q.limit,
        offset: q.offset,
    }))
}

#[derive(Debug, Deserialize)]
pub struct SuiteResultsQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 {
    25
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/suites-runs/{run_id}/results",
    tag = "executions",
    params(("project_id" = Uuid, Path), ("run_id" = Uuid, Path)),
    responses((status = 200, body = Page<SuiteResultRow>)),
    security(("bearer_auth" = []))
)]
pub async fn list_suite_results(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, run_id)): Path<(Uuid, Uuid)>,
    Query(q): Query<SuiteResultsQuery>,
) -> AppResult<Json<Page<SuiteResultRow>>> {
    ensure_project(&state.pool, project_id).await?;
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    let run_project = ingest_svc::ensure_run_exists(&state.pool, run_id).await?;
    if run_project != project_id {
        return Err(AppError::NotFound("run".into()));
    }
    let (items, total) = camp::list_suite_results(&state.pool, run_id, q.limit, q.offset).await?;
    Ok(Json(Page {
        items,
        total,
        limit: q.limit,
        offset: q.offset,
    }))
}

#[utoipa::path(
    delete,
    path = "/api/v1/projects/{project_id}/suites-runs/{run_id}",
    tag = "executions",
    params(("project_id" = Uuid, Path), ("run_id" = Uuid, Path)),
    responses((status = 204, description = "Deleted")),
    security(("bearer_auth" = []))
)]
pub async fn delete_suite_run(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, run_id)): Path<(Uuid, Uuid)>,
) -> AppResult<axum::http::StatusCode> {
    ensure_project(&state.pool, project_id).await?;
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ExecuteRuns,
    )
    .await?;
    let res = sqlx::query("DELETE FROM test_runs WHERE project_id = $1 AND id = $2")
        .bind(project_id)
        .bind(run_id)
        .execute(&state.pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound("run".into()));
    }
    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/suites-runs/{run_id}",
    tag = "executions",
    params(("project_id" = Uuid, Path), ("run_id" = Uuid, Path)),
    responses((status = 200, description = "Rerun stub / new suite shell", body = AutomatedSuite)),
    security(("bearer_auth" = []))
)]
pub async fn rerun_suite(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, run_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<AutomatedSuite>> {
    ensure_project(&state.pool, project_id).await?;
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::TriggerCi,
    )
    .await?;
    let run = crate::services::runs::load_run(&state.pool, project_id, run_id).await?;
    let iteration_id = run
        .iteration_id
        .ok_or_else(|| AppError::BadRequest("run is not an automated suite".into()))?;
    let created_by = auth
        .as_ref()
        .map(|a| a.user.display_name.clone())
        .or(run.created_by_name.clone());
    let new_run = camp::create_automated_suite_run(
        &state.pool,
        project_id,
        iteration_id,
        &format!("Rerun of {}", run.name),
        "ci",
        created_by.as_deref(),
        run.external_ref.as_deref(),
        run.environment.clone(),
        run.artifacts.clone(),
    )
    .await?;
    Ok(Json(AutomatedSuite {
        id: new_run.id,
        project_id: new_run.project_id,
        iteration_id: new_run.iteration_id,
        name: new_run.name,
        status: new_run.status,
        source: new_run.source,
        created_by_name: new_run.created_by_name,
        created_at: new_run.created_at,
        started_at: new_run.started_at,
        finished_at: new_run.finished_at,
        duration_ms: new_run.duration_ms,
        pass_count: new_run.pass_count,
        fail_count: new_run.fail_count,
        skip_count: new_run.skip_count,
        blocked_count: new_run.blocked_count,
        environment: new_run.environment,
        artifacts: new_run.artifacts,
        external_ref: new_run.external_ref,
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/cases/{case_id}/executions",
    tag = "executions",
    params(("project_id" = Uuid, Path), ("case_id" = Uuid, Path)),
    responses((status = 200, body = CaseExecutionHistoryPage)),
    security(("bearer_auth" = []))
)]
pub async fn case_executions(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, case_id)): Path<(Uuid, Uuid)>,
    Query(q): Query<SuiteResultsQuery>,
) -> AppResult<Json<CaseExecutionHistoryPage>> {
    ensure_project(&state.pool, project_id).await?;
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    Ok(Json(
        camp::case_execution_history(&state.pool, project_id, case_id, q.limit, q.offset).await?,
    ))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/ingest",
    tag = "ingest",
    params(("project_id" = Uuid, Path)),
    request_body = PipelineIngestRequest,
    responses((status = 200, description = "Pipeline ingest", body = PipelineIngestResponse)),
    security(("bearer_auth" = []))
)]
pub async fn pipeline_ingest(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<PipelineIngestRequest>,
) -> AppResult<Json<PipelineIngestResponse>> {
    ensure_project(&state.pool, project_id).await?;
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::UploadResults,
    )
    .await?;
    let format = body.format.trim().to_lowercase();
    if format.is_empty() || body.content.trim().is_empty() {
        return Err(AppError::BadRequest(
            "format and content are required".into(),
        ));
    }
    let parsed = parse_payload(&format, &body.content).map_err(AppError::BadRequest)?;

    let campaign_name = body.campaign.as_deref().unwrap_or("CI").trim().to_string();
    let campaign =
        camp::find_or_create_campaign(&state.pool, project_id, &campaign_name, None).await?;
    let iteration_name = body
        .iteration
        .clone()
        .or_else(|| parsed.run_name.clone())
        .unwrap_or_else(|| "Pipeline".into());
    let iteration = camp::find_or_create_iteration(
        &state.pool,
        project_id,
        campaign.id,
        &iteration_name,
        body.iteration_reference.as_deref().unwrap_or(""),
        body.cadence.as_deref().unwrap_or("on-push"),
    )
    .await?;

    let run_name = body
        .run_name
        .clone()
        .or_else(|| parsed.run_name.clone())
        .unwrap_or_else(|| format!("Suite {}", chrono::Utc::now().format("%Y-%m-%d %H:%M")));
    let created_by = body
        .created_by_name
        .clone()
        .or_else(|| auth.as_ref().map(|a| a.user.display_name.clone()));

    let run = camp::create_automated_suite_run(
        &state.pool,
        project_id,
        iteration.id,
        &run_name,
        &body.source,
        created_by.as_deref(),
        body.external_ref.as_deref(),
        body.environment.clone(),
        body.artifacts.clone(),
    )
    .await?;

    if body.async_mode || parsed.results.len() >= ingest_svc::ASYNC_THRESHOLD {
        let job = jobs_svc::enqueue(
            &state.pool,
            "ingest",
            json!({
                "run_id": run.id,
                "project_id": project_id,
                "iteration_id": iteration.id,
                "format": format,
                "content": body.content,
                "source": body.source,
                "upsert": false,
            }),
            5,
        )
        .await?;
        return Ok(Json(PipelineIngestResponse {
            campaign_id: campaign.id,
            iteration_id: iteration.id,
            run_id: run.id,
            inserted: 0,
            updated: 0,
            unmatched_case_keys: vec![],
            job_id: Some(job.id),
            parsed: parsed.results.len() as i64,
        }));
    }

    let report = ingest_svc::persist_results(
        &state.pool,
        run.id,
        project_id,
        &parsed.results,
        &body.source,
        false,
    )
    .await?;
    camp::refresh_run_counters(&state.pool, run.id).await?;
    camp::sync_iteration_item_statuses(&state.pool, iteration.id).await?;

    Ok(Json(PipelineIngestResponse {
        campaign_id: campaign.id,
        iteration_id: iteration.id,
        run_id: run.id,
        inserted: report.inserted,
        updated: report.updated,
        unmatched_case_keys: report.unmatched_case_keys,
        job_id: None,
        parsed: parsed.results.len() as i64,
    }))
}

/// Re-export shape for OpenAPI parity with legacy ingest.
#[allow(dead_code)]
pub async fn _ingest_compat_doc(_: IngestRequest) -> IngestResponse {
    unreachable!()
}
