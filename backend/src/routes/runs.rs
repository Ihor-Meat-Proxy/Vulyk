//! Minimal plans/runs/results surface for MCP (W10) and UI stubs.
//! W3 owns deeper plan workflows; keep these CRUD paths stable.

use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::{BulkIngestResults, CreateRun, TestResult, TestRun, UpdateRun};
use crate::routes::cases::ensure_project;
use crate::routes::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/projects/{project_id}/runs",
            get(list_runs).post(create_run),
        )
        .route(
            "/api/v1/projects/{project_id}/runs/{run_id}",
            get(get_run).put(update_run),
        )
        .route(
            "/api/v1/projects/{project_id}/runs/{run_id}/trigger",
            post(trigger_run),
        )
        .route(
            "/api/v1/projects/{project_id}/runs/{run_id}/results",
            get(list_results).post(ingest_results),
        )
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/runs",
    tag = "runs",
    params(("project_id" = Uuid, Path, description = "Project id")),
    responses((status = 200, body = [TestRun]))
)]
pub async fn list_runs(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
) -> AppResult<Json<Vec<TestRun>>> {
    ensure_project(&state.pool, project_id).await?;
    let rows = sqlx::query_as::<_, TestRun>(
        r#"SELECT id, project_id, name, status, started_at, finished_at, created_at, updated_at
           FROM test_runs WHERE project_id = $1 ORDER BY created_at DESC"#,
    )
    .bind(project_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/runs",
    tag = "runs",
    request_body = CreateRun,
    params(("project_id" = Uuid, Path, description = "Project id")),
    responses((status = 200, body = TestRun))
)]
pub async fn create_run(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Json(body): Json<CreateRun>,
) -> AppResult<Json<TestRun>> {
    ensure_project(&state.pool, project_id).await?;
    if body.name.trim().is_empty() {
        return Err(AppError::BadRequest("name required".into()));
    }
    let row = sqlx::query_as::<_, TestRun>(
        r#"INSERT INTO test_runs (project_id, name, status)
           VALUES ($1, $2, $3)
           RETURNING id, project_id, name, status, started_at, finished_at, created_at, updated_at"#,
    )
    .bind(project_id)
    .bind(body.name.trim())
    .bind(&body.status)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(row))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/runs/{run_id}",
    tag = "runs",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("run_id" = Uuid, Path, description = "Run id")
    ),
    responses((status = 200, body = TestRun))
)]
pub async fn get_run(
    State(state): State<AppState>,
    Path((project_id, run_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<TestRun>> {
    let row = load_run(&state.pool, project_id, run_id).await?;
    Ok(Json(row))
}

#[utoipa::path(
    put,
    path = "/api/v1/projects/{project_id}/runs/{run_id}",
    tag = "runs",
    request_body = UpdateRun,
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("run_id" = Uuid, Path, description = "Run id")
    ),
    responses((status = 200, body = TestRun))
)]
pub async fn update_run(
    State(state): State<AppState>,
    Path((project_id, run_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateRun>,
) -> AppResult<Json<TestRun>> {
    let existing = load_run(&state.pool, project_id, run_id).await?;
    let name = body.name.unwrap_or(existing.name);
    let status = body.status.unwrap_or(existing.status);
    let row = sqlx::query_as::<_, TestRun>(
        r#"UPDATE test_runs SET name = $1, status = $2, updated_at = NOW()
           WHERE id = $3 AND project_id = $4
           RETURNING id, project_id, name, status, started_at, finished_at, created_at, updated_at"#,
    )
    .bind(&name)
    .bind(&status)
    .bind(run_id)
    .bind(project_id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(row))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/runs/{run_id}/trigger",
    tag = "runs",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("run_id" = Uuid, Path, description = "Run id")
    ),
    responses((status = 200, body = TestRun))
)]
pub async fn trigger_run(
    State(state): State<AppState>,
    Path((project_id, run_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<TestRun>> {
    let _ = load_run(&state.pool, project_id, run_id).await?;
    let row = sqlx::query_as::<_, TestRun>(
        r#"UPDATE test_runs
           SET status = 'running', started_at = COALESCE(started_at, NOW()), updated_at = NOW()
           WHERE id = $1 AND project_id = $2
           RETURNING id, project_id, name, status, started_at, finished_at, created_at, updated_at"#,
    )
    .bind(run_id)
    .bind(project_id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(row))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/runs/{run_id}/results",
    tag = "runs",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("run_id" = Uuid, Path, description = "Run id")
    ),
    responses((status = 200, body = [TestResult]))
)]
pub async fn list_results(
    State(state): State<AppState>,
    Path((project_id, run_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<Vec<TestResult>>> {
    let _ = load_run(&state.pool, project_id, run_id).await?;
    let rows = sqlx::query_as::<_, TestResult>(
        r#"SELECT id, run_id, case_id, status, message, source, raw, executed_at, created_at
           FROM test_results WHERE run_id = $1 ORDER BY executed_at"#,
    )
    .bind(run_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/runs/{run_id}/results",
    tag = "runs",
    request_body = BulkIngestResults,
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("run_id" = Uuid, Path, description = "Run id")
    ),
    responses((status = 200, body = [TestResult]))
)]
pub async fn ingest_results(
    State(state): State<AppState>,
    Path((project_id, run_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<BulkIngestResults>,
) -> AppResult<Json<Vec<TestResult>>> {
    let _ = load_run(&state.pool, project_id, run_id).await?;
    let mut out = Vec::with_capacity(body.results.len());
    for item in body.results {
        let case_id = if let Some(id) = item.case_id {
            Some(id)
        } else if let Some(key) = item.case_key.as_ref() {
            sqlx::query_scalar::<_, Uuid>(
                "SELECT id FROM test_cases WHERE project_id = $1 AND key = $2",
            )
            .bind(project_id)
            .bind(key)
            .fetch_optional(&state.pool)
            .await?
        } else {
            None
        };

        let row = sqlx::query_as::<_, TestResult>(
            r#"INSERT INTO test_results (run_id, case_id, status, message, source, raw)
               VALUES ($1, $2, $3, $4, $5, $6)
               RETURNING id, run_id, case_id, status, message, source, raw, executed_at, created_at"#,
        )
        .bind(run_id)
        .bind(case_id)
        .bind(&item.status)
        .bind(&item.message)
        .bind(&item.source)
        .bind(&item.raw)
        .fetch_one(&state.pool)
        .await?;
        out.push(row);
    }
    Ok(Json(out))
}

async fn load_run(pool: &sqlx::PgPool, project_id: Uuid, run_id: Uuid) -> AppResult<TestRun> {
    ensure_project(pool, project_id).await?;
    sqlx::query_as::<_, TestRun>(
        r#"SELECT id, project_id, name, status, started_at, finished_at, created_at, updated_at
           FROM test_runs WHERE id = $1 AND project_id = $2"#,
    )
    .bind(run_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("run".into()))
}
