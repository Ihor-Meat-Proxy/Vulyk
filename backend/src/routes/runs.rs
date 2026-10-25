//! Plans/runs/results surface for MCP (W10) and UI.
//! W4 adds source/meta columns and keeps these project-scoped CRUD paths stable.
//! Format ingest (JUnit / Vulyk JSON) lives in `routes::ingest`.

use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::json;
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
        .route(
            "/api/v1/runs/{id}",
            get(get_run_by_id).put(update_run_by_id).delete(delete_run),
        )
        .route("/api/v1/runs/{id}/results", get(list_results_by_id))
}

#[derive(Debug, Deserialize)]
pub struct ListRunsQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

const RUN_COLS: &str = "id, project_id, name, status, source, external_ref, meta, started_at, finished_at, created_at, updated_at";
const RESULT_COLS: &str = "id, run_id, case_id, status, message, source, external_id, duration_ms, raw, executed_at, created_at";

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/runs",
    tag = "runs",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("limit" = Option<i64>, Query, description = "Max rows"),
        ("offset" = Option<i64>, Query, description = "Offset"),
    ),
    responses((status = 200, description = "List runs", body = [TestRun]))
)]
pub async fn list_runs(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Query(q): Query<ListRunsQuery>,
) -> AppResult<Json<Vec<TestRun>>> {
    ensure_project(&state.pool, project_id).await?;
    let limit = q.limit.unwrap_or(50).clamp(1, 500);
    let offset = q.offset.unwrap_or(0).max(0);
    let rows = sqlx::query_as::<_, TestRun>(&format!(
        r#"SELECT {RUN_COLS}
           FROM test_runs WHERE project_id = $1
           ORDER BY created_at DESC
           LIMIT $2 OFFSET $3"#
    ))
    .bind(project_id)
    .bind(limit)
    .bind(offset)
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
    let meta = if body.meta.is_null() {
        json!({})
    } else {
        body.meta
    };
    let started = if body.status == "running" {
        Some(Utc::now())
    } else {
        None
    };
    let row = sqlx::query_as::<_, TestRun>(&format!(
        r#"INSERT INTO test_runs (project_id, name, status, source, external_ref, meta, started_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7)
           RETURNING {RUN_COLS}"#
    ))
    .bind(project_id)
    .bind(body.name.trim())
    .bind(&body.status)
    .bind(&body.source)
    .bind(&body.external_ref)
    .bind(meta)
    .bind(started)
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
    Ok(Json(load_run(&state.pool, project_id, run_id).await?))
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
    let external_ref = body.external_ref.or(existing.external_ref);
    let meta = body.meta.unwrap_or(existing.meta);
    let started_at = body.started_at.or(existing.started_at);
    let finished_at = body.finished_at.or(existing.finished_at);
    let row = sqlx::query_as::<_, TestRun>(&format!(
        r#"UPDATE test_runs
           SET name = $1, status = $2, external_ref = $3, meta = $4,
               started_at = $5, finished_at = $6, updated_at = NOW()
           WHERE id = $7 AND project_id = $8
           RETURNING {RUN_COLS}"#
    ))
    .bind(&name)
    .bind(&status)
    .bind(external_ref)
    .bind(meta)
    .bind(started_at)
    .bind(finished_at)
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
    let row = sqlx::query_as::<_, TestRun>(&format!(
        r#"UPDATE test_runs
           SET status = 'running', started_at = COALESCE(started_at, NOW()), updated_at = NOW()
           WHERE id = $1 AND project_id = $2
           RETURNING {RUN_COLS}"#
    ))
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
    let rows = sqlx::query_as::<_, TestResult>(&format!(
        r#"SELECT {RESULT_COLS}
           FROM test_results WHERE run_id = $1 ORDER BY executed_at, created_at"#
    ))
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

        let row = sqlx::query_as::<_, TestResult>(&format!(
            r#"INSERT INTO test_results
               (run_id, case_id, status, message, source, external_id, duration_ms, raw)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
               RETURNING {RESULT_COLS}"#
        ))
        .bind(run_id)
        .bind(case_id)
        .bind(&item.status)
        .bind(&item.message)
        .bind(&item.source)
        .bind(&item.external_id)
        .bind(item.duration_ms)
        .bind(&item.raw)
        .fetch_one(&state.pool)
        .await?;
        out.push(row);
    }
    Ok(Json(out))
}

#[utoipa::path(
    get,
    path = "/api/v1/runs/{id}",
    tag = "runs",
    params(("id" = Uuid, Path, description = "Run id")),
    responses((status = 200, body = TestRun))
)]
pub async fn get_run_by_id(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<TestRun>> {
    let row = sqlx::query_as::<_, TestRun>(&format!(
        r#"SELECT {RUN_COLS} FROM test_runs WHERE id = $1"#
    ))
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("run".into()))?;
    Ok(Json(row))
}

#[utoipa::path(
    put,
    path = "/api/v1/runs/{id}",
    tag = "runs",
    request_body = UpdateRun,
    params(("id" = Uuid, Path, description = "Run id")),
    responses((status = 200, body = TestRun))
)]
pub async fn update_run_by_id(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateRun>,
) -> AppResult<Json<TestRun>> {
    let existing = sqlx::query_as::<_, TestRun>(&format!(
        r#"SELECT {RUN_COLS} FROM test_runs WHERE id = $1"#
    ))
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("run".into()))?;

    let name = body.name.unwrap_or(existing.name);
    let status = body.status.unwrap_or(existing.status);
    let external_ref = body.external_ref.or(existing.external_ref);
    let meta = body.meta.unwrap_or(existing.meta);
    let started_at = body.started_at.or(existing.started_at);
    let finished_at = body.finished_at.or(existing.finished_at);

    let row = sqlx::query_as::<_, TestRun>(&format!(
        r#"UPDATE test_runs
           SET name = $2, status = $3, external_ref = $4, meta = $5,
               started_at = $6, finished_at = $7, updated_at = NOW()
           WHERE id = $1
           RETURNING {RUN_COLS}"#
    ))
    .bind(id)
    .bind(name)
    .bind(status)
    .bind(external_ref)
    .bind(meta)
    .bind(started_at)
    .bind(finished_at)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(row))
}

#[utoipa::path(
    delete,
    path = "/api/v1/runs/{id}",
    tag = "runs",
    params(("id" = Uuid, Path, description = "Run id")),
    responses((status = 204, description = "Deleted"))
)]
pub async fn delete_run(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<axum::http::StatusCode> {
    let result = sqlx::query("DELETE FROM test_runs WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("run".into()));
    }
    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/api/v1/runs/{id}/results",
    tag = "runs",
    params(("id" = Uuid, Path, description = "Run id")),
    responses((status = 200, body = [TestResult]))
)]
pub async fn list_results_by_id(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Vec<TestResult>>> {
    let exists: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM test_runs WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?;
    if exists.is_none() {
        return Err(AppError::NotFound("run".into()));
    }
    let rows = sqlx::query_as::<_, TestResult>(&format!(
        r#"SELECT {RESULT_COLS}
           FROM test_results WHERE run_id = $1 ORDER BY executed_at, created_at"#
    ))
    .bind(id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

async fn load_run(pool: &sqlx::PgPool, project_id: Uuid, run_id: Uuid) -> AppResult<TestRun> {
    ensure_project(pool, project_id).await?;
    sqlx::query_as::<_, TestRun>(&format!(
        r#"SELECT {RUN_COLS} FROM test_runs WHERE id = $1 AND project_id = $2"#
    ))
    .bind(run_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("run".into()))
}
