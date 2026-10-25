//! Plans/runs/results surface for UI (W3), MCP (W10), and CI ingest (W4).
//! Format ingest (JUnit / Vulyk JSON) lives in `routes::ingest`.

use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::{
    BulkIngestResults, CreateResult, CreateRun, Page, PageQuery, ResultStep, RunItem,
    RunItemExecution, RunItemView, TestResult, TestResultDetail, TestRun, TestRunDetail, TestStep,
    UpdateRun, UpdateRunItem,
};
use crate::routes::AppState;
use crate::services::cases::{ensure_project, load_steps};
use crate::services::runs::{
    all_project_case_ids, collect_case_ids_for_run, load_run, load_run_items, run_progress,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/projects/{project_id}/runs",
            get(list_runs).post(create_run),
        )
        .route(
            "/api/v1/projects/{project_id}/runs/{run_id}",
            get(get_run).put(update_run).delete(delete_run),
        )
        .route(
            "/api/v1/projects/{project_id}/runs/{run_id}/trigger",
            post(trigger_run),
        )
        .route(
            "/api/v1/projects/{project_id}/runs/{run_id}/items",
            get(list_run_items),
        )
        .route(
            "/api/v1/projects/{project_id}/runs/{run_id}/items/{item_id}",
            get(get_run_item).put(update_run_item),
        )
        .route(
            "/api/v1/projects/{project_id}/runs/{run_id}/results",
            get(list_results).post(ingest_results),
        )
        .route(
            "/api/v1/projects/{project_id}/runs/{run_id}/results/execute",
            post(create_result),
        )
        .route(
            "/api/v1/runs/{id}",
            get(get_run_by_id).put(update_run_by_id).delete(delete_run_by_id),
        )
        .route("/api/v1/runs/{id}/results", get(list_results_by_id))
}

const RUN_COLS: &str = "id, project_id, plan_id, name, description, status, source, external_ref, meta, started_at, finished_at, created_at, updated_at";
const RESULT_COLS: &str = "id, run_id, case_id, run_item_id, status, message, source, external_id, duration_ms, raw, assignee, elapsed_ms, executed_at, created_at";

#[derive(Debug, Deserialize)]
pub struct ListRunsQuery {
    pub status: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/runs",
    tag = "runs",
    params(
        ("project_id" = Uuid, Path),
        ("status" = Option<String>, Query),
        ("limit" = Option<i64>, Query),
        ("offset" = Option<i64>, Query)
    ),
    responses((status = 200, description = "Runs", body = Page<TestRun>))
)]
pub async fn list_runs(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Query(query): Query<ListRunsQuery>,
) -> AppResult<Json<Page<TestRun>>> {
    ensure_project(&state.pool, project_id).await?;
    let page = PageQuery {
        limit: query.limit,
        offset: query.offset,
    };
    let limit = page.limit();
    let offset = page.offset();

    let total: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)::bigint FROM test_runs
           WHERE project_id = $1 AND ($2::text IS NULL OR status = $2)"#,
    )
    .bind(project_id)
    .bind(query.status.as_deref())
    .fetch_one(&state.pool)
    .await?;

    let runs = sqlx::query_as::<_, TestRun>(&format!(
        r#"SELECT {RUN_COLS}
           FROM test_runs
           WHERE project_id = $1 AND ($2::text IS NULL OR status = $2)
           ORDER BY created_at DESC
           LIMIT $3 OFFSET $4"#
    ))
    .bind(project_id)
    .bind(query.status.as_deref())
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(Page::new(runs, total, limit, offset)))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/runs",
    tag = "runs",
    params(("project_id" = Uuid, Path)),
    request_body = CreateRun,
    responses((status = 200, description = "Created run", body = TestRunDetail))
)]
pub async fn create_run(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Json(body): Json<CreateRun>,
) -> AppResult<Json<TestRunDetail>> {
    ensure_project(&state.pool, project_id).await?;
    if body.name.trim().is_empty() {
        return Err(AppError::BadRequest("name required".into()));
    }

    let has_selectors = body.plan_id.is_some()
        || !body.suite_ids.is_empty()
        || !body.case_ids.is_empty()
        || !body.case_statuses.is_empty()
        || !body.tag_names.is_empty();

    let mut case_ids = if has_selectors {
        collect_case_ids_for_run(
            &state.pool,
            project_id,
            body.plan_id,
            &body.suite_ids,
            &body.case_ids,
            &body.case_statuses,
            &body.tag_names,
        )
        .await?
    } else {
        Vec::new()
    };

    if body.include_all_cases && case_ids.is_empty() {
        case_ids = all_project_case_ids(&state.pool, project_id).await?;
    }

    if has_selectors && case_ids.is_empty() {
        return Err(AppError::BadRequest(
            "no cases matched the selection filters".into(),
        ));
    }

    let status = if body.start && !case_ids.is_empty() {
        "in_progress".to_string()
    } else {
        body.status
            .unwrap_or_else(|| "draft".to_string())
            .trim()
            .to_string()
    };
    let started_at = if status == "in_progress" || status == "running" {
        Some(chrono::Utc::now())
    } else {
        None
    };

    let meta = if body.meta.is_null() {
        json!({})
    } else {
        body.meta
    };
    let source = if body.source.trim().is_empty() {
        "manual".to_string()
    } else {
        body.source.trim().to_string()
    };

    let mut tx = state.pool.begin().await?;
    let run = sqlx::query_as::<_, TestRun>(&format!(
        r#"INSERT INTO test_runs
             (project_id, plan_id, name, description, status, source, external_ref, meta, started_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
           RETURNING {RUN_COLS}"#
    ))
    .bind(project_id)
    .bind(body.plan_id)
    .bind(body.name.trim())
    .bind(&body.description)
    .bind(&status)
    .bind(&source)
    .bind(&body.external_ref)
    .bind(meta)
    .bind(started_at)
    .fetch_one(&mut *tx)
    .await?;

    for (idx, case_id) in case_ids.iter().enumerate() {
        sqlx::query(
            r#"INSERT INTO run_items (run_id, case_id, sort_order)
               VALUES ($1, $2, $3)
               ON CONFLICT (run_id, case_id) DO NOTHING"#,
        )
        .bind(run.id)
        .bind(case_id)
        .bind(idx as i32)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    let progress = run_progress(&state.pool, run.id).await?;
    let items = load_run_items(&state.pool, run.id).await?;
    Ok(Json(TestRunDetail {
        run,
        progress,
        items,
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/runs/{run_id}",
    tag = "runs",
    params(("project_id" = Uuid, Path), ("run_id" = Uuid, Path)),
    responses((status = 200, description = "Run detail", body = TestRunDetail))
)]
pub async fn get_run(
    State(state): State<AppState>,
    Path((project_id, run_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<TestRunDetail>> {
    let run = load_run(&state.pool, project_id, run_id).await?;
    let progress = run_progress(&state.pool, run.id).await?;
    let items = load_run_items(&state.pool, run.id).await?;
    Ok(Json(TestRunDetail {
        run,
        progress,
        items,
    }))
}

#[utoipa::path(
    put,
    path = "/api/v1/projects/{project_id}/runs/{run_id}",
    tag = "runs",
    params(("project_id" = Uuid, Path), ("run_id" = Uuid, Path)),
    request_body = UpdateRun,
    responses((status = 200, description = "Updated", body = TestRunDetail))
)]
pub async fn update_run(
    State(state): State<AppState>,
    Path((project_id, run_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateRun>,
) -> AppResult<Json<TestRunDetail>> {
    let existing = load_run(&state.pool, project_id, run_id).await?;
    let status = body.status.unwrap_or(existing.status);
    let finished_at = body.finished_at.or_else(|| {
        if status == "completed" || status == "closed" {
            Some(chrono::Utc::now())
        } else {
            existing.finished_at
        }
    });
    let started_at = body.started_at.or_else(|| {
        if (status == "in_progress" || status == "running") && existing.started_at.is_none() {
            Some(chrono::Utc::now())
        } else {
            existing.started_at
        }
    });
    let external_ref = body.external_ref.or(existing.external_ref);
    let meta = body.meta.unwrap_or(existing.meta);

    let run = sqlx::query_as::<_, TestRun>(&format!(
        r#"UPDATE test_runs
           SET name = $3, description = $4, status = $5,
               external_ref = $6, meta = $7,
               started_at = $8, finished_at = $9, updated_at = NOW()
           WHERE project_id = $1 AND id = $2
           RETURNING {RUN_COLS}"#
    ))
    .bind(project_id)
    .bind(run_id)
    .bind(body.name.unwrap_or(existing.name))
    .bind(body.description.unwrap_or(existing.description))
    .bind(status)
    .bind(external_ref)
    .bind(meta)
    .bind(started_at)
    .bind(finished_at)
    .fetch_one(&state.pool)
    .await?;

    let progress = run_progress(&state.pool, run.id).await?;
    let items = load_run_items(&state.pool, run.id).await?;
    Ok(Json(TestRunDetail {
        run,
        progress,
        items,
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/runs/{run_id}/trigger",
    tag = "runs",
    params(("project_id" = Uuid, Path), ("run_id" = Uuid, Path)),
    responses((status = 200, description = "Triggered", body = TestRun))
)]
pub async fn trigger_run(
    State(state): State<AppState>,
    Path((project_id, run_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<TestRun>> {
    let _ = load_run(&state.pool, project_id, run_id).await?;
    let row = sqlx::query_as::<_, TestRun>(&format!(
        r#"UPDATE test_runs
           SET status = 'running',
               started_at = COALESCE(started_at, NOW()),
               updated_at = NOW()
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
    delete,
    path = "/api/v1/projects/{project_id}/runs/{run_id}",
    tag = "runs",
    params(("project_id" = Uuid, Path), ("run_id" = Uuid, Path)),
    responses((status = 204, description = "Deleted"))
)]
pub async fn delete_run(
    State(state): State<AppState>,
    Path((project_id, run_id)): Path<(Uuid, Uuid)>,
) -> AppResult<axum::http::StatusCode> {
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
    get,
    path = "/api/v1/projects/{project_id}/runs/{run_id}/items",
    tag = "runs",
    params(("project_id" = Uuid, Path), ("run_id" = Uuid, Path)),
    responses((status = 200, description = "Run items", body = [RunItemView]))
)]
pub async fn list_run_items(
    State(state): State<AppState>,
    Path((project_id, run_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<Vec<RunItemView>>> {
    let _ = load_run(&state.pool, project_id, run_id).await?;
    Ok(Json(load_run_items(&state.pool, run_id).await?))
}

async fn load_item(pool: &sqlx::PgPool, run_id: Uuid, item_id: Uuid) -> AppResult<RunItem> {
    sqlx::query_as::<_, RunItem>(
        r#"SELECT id, run_id, case_id, assignee, status, sort_order, comment,
                  executed_at, created_at, updated_at
           FROM run_items WHERE run_id = $1 AND id = $2"#,
    )
    .bind(run_id)
    .bind(item_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("run item".into()))
}

async fn latest_result_for_item(
    pool: &sqlx::PgPool,
    run_item_id: Uuid,
) -> AppResult<Option<TestResultDetail>> {
    let result = sqlx::query_as::<_, TestResult>(&format!(
        r#"SELECT {RESULT_COLS}
           FROM test_results WHERE run_item_id = $1
           ORDER BY executed_at DESC LIMIT 1"#
    ))
    .bind(run_item_id)
    .fetch_optional(pool)
    .await?;

    if let Some(result) = result {
        let steps = sqlx::query_as::<_, ResultStep>(
            r#"SELECT id, result_id, step_id, position, action, expected_result,
                      status, actual_result, comment
               FROM result_steps WHERE result_id = $1 ORDER BY position"#,
        )
        .bind(result.id)
        .fetch_all(pool)
        .await?;
        Ok(Some(TestResultDetail { result, steps }))
    } else {
        Ok(None)
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/runs/{run_id}/items/{item_id}",
    tag = "runs",
    params(
        ("project_id" = Uuid, Path),
        ("run_id" = Uuid, Path),
        ("item_id" = Uuid, Path)
    ),
    responses((status = 200, description = "Execution view", body = RunItemExecution))
)]
pub async fn get_run_item(
    State(state): State<AppState>,
    Path((project_id, run_id, item_id)): Path<(Uuid, Uuid, Uuid)>,
) -> AppResult<Json<RunItemExecution>> {
    let _ = load_run(&state.pool, project_id, run_id).await?;
    let item = load_item(&state.pool, run_id, item_id).await?;
    let case_row = sqlx::query_as::<_, (String, String, String)>(
        "SELECT key, title, priority FROM test_cases WHERE id = $1",
    )
    .bind(item.case_id)
    .fetch_one(&state.pool)
    .await?;
    let steps = load_steps(&state.pool, item.case_id).await?;
    let latest_result = latest_result_for_item(&state.pool, item.id).await?;
    Ok(Json(RunItemExecution {
        item: RunItemView {
            item,
            case_key: case_row.0,
            case_title: case_row.1,
            case_priority: case_row.2,
        },
        steps,
        latest_result,
    }))
}

#[utoipa::path(
    put,
    path = "/api/v1/projects/{project_id}/runs/{run_id}/items/{item_id}",
    tag = "runs",
    params(
        ("project_id" = Uuid, Path),
        ("run_id" = Uuid, Path),
        ("item_id" = Uuid, Path)
    ),
    request_body = UpdateRunItem,
    responses((status = 200, description = "Updated item", body = RunItemView))
)]
pub async fn update_run_item(
    State(state): State<AppState>,
    Path((project_id, run_id, item_id)): Path<(Uuid, Uuid, Uuid)>,
    Json(body): Json<UpdateRunItem>,
) -> AppResult<Json<RunItemView>> {
    let _ = load_run(&state.pool, project_id, run_id).await?;
    let existing = load_item(&state.pool, run_id, item_id).await?;
    let status = body.status.unwrap_or(existing.status);
    let allowed = [
        "untested", "passed", "failed", "blocked", "skipped", "retest",
    ];
    if !allowed.contains(&status.as_str()) {
        return Err(AppError::BadRequest("invalid status".into()));
    }
    let executed_at = if status == "untested" {
        None
    } else {
        Some(chrono::Utc::now())
    };

    let item = sqlx::query_as::<_, RunItem>(
        r#"UPDATE run_items
           SET status = $3, assignee = COALESCE($4, assignee),
               comment = COALESCE($5, comment), executed_at = $6, updated_at = NOW()
           WHERE run_id = $1 AND id = $2
           RETURNING id, run_id, case_id, assignee, status, sort_order, comment,
                     executed_at, created_at, updated_at"#,
    )
    .bind(run_id)
    .bind(item_id)
    .bind(&status)
    .bind(body.assignee.as_deref())
    .bind(body.comment.as_deref())
    .bind(executed_at)
    .fetch_one(&state.pool)
    .await?;

    let remaining: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)::bigint FROM run_items
           WHERE run_id = $1 AND status = 'untested'"#,
    )
    .bind(run_id)
    .fetch_one(&state.pool)
    .await?;
    if remaining == 0 {
        sqlx::query(
            r#"UPDATE test_runs
               SET status = 'completed', finished_at = NOW(), updated_at = NOW()
               WHERE id = $1 AND status IN ('in_progress', 'running')"#,
        )
        .bind(run_id)
        .execute(&state.pool)
        .await?;
    }

    let case_row = sqlx::query_as::<_, (String, String, String)>(
        "SELECT key, title, priority FROM test_cases WHERE id = $1",
    )
    .bind(item.case_id)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(RunItemView {
        item,
        case_key: case_row.0,
        case_title: case_row.1,
        case_priority: case_row.2,
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/runs/{run_id}/results",
    tag = "runs",
    params(("project_id" = Uuid, Path), ("run_id" = Uuid, Path)),
    responses((status = 200, description = "Results", body = [TestResult]))
)]
pub async fn list_results(
    State(state): State<AppState>,
    Path((project_id, run_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<Vec<TestResult>>> {
    let _ = load_run(&state.pool, project_id, run_id).await?;
    let results = sqlx::query_as::<_, TestResult>(&format!(
        r#"SELECT {RESULT_COLS}
           FROM test_results WHERE run_id = $1 ORDER BY executed_at DESC"#
    ))
    .bind(run_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(results))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/runs/{run_id}/results",
    tag = "runs",
    params(("project_id" = Uuid, Path), ("run_id" = Uuid, Path)),
    request_body = BulkIngestResults,
    responses((status = 200, description = "Ingested results", body = [TestResult]))
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
    post,
    path = "/api/v1/projects/{project_id}/runs/{run_id}/results/execute",
    tag = "runs",
    params(("project_id" = Uuid, Path), ("run_id" = Uuid, Path)),
    request_body = CreateResult,
    responses((status = 200, description = "Manual execution result", body = TestResultDetail))
)]
pub async fn create_result(
    State(state): State<AppState>,
    Path((project_id, run_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<CreateResult>,
) -> AppResult<Json<TestResultDetail>> {
    let _ = load_run(&state.pool, project_id, run_id).await?;

    let mut case_id = body.case_id;
    if let Some(run_item_id) = body.run_item_id {
        let item = load_item(&state.pool, run_id, run_item_id).await?;
        case_id = Some(item.case_id);
    }

    let mut tx = state.pool.begin().await?;
    let result = sqlx::query_as::<_, TestResult>(&format!(
        r#"INSERT INTO test_results
             (run_id, case_id, run_item_id, status, message, source, raw, assignee, elapsed_ms)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
           RETURNING {RESULT_COLS}"#
    ))
    .bind(run_id)
    .bind(case_id)
    .bind(body.run_item_id)
    .bind(&body.status)
    .bind(&body.message)
    .bind(&body.source)
    .bind(&body.raw)
    .bind(body.assignee.as_deref())
    .bind(body.elapsed_ms)
    .fetch_one(&mut *tx)
    .await?;

    let mut steps = Vec::new();
    if body.steps.is_empty() {
        if let Some(cid) = case_id {
            let case_steps = sqlx::query_as::<_, TestStep>(
                r#"SELECT id, case_id, position, action, expected_result, data
                   FROM test_steps WHERE case_id = $1 ORDER BY position"#,
            )
            .bind(cid)
            .fetch_all(&mut *tx)
            .await?;
            for step in case_steps {
                let row = sqlx::query_as::<_, ResultStep>(
                    r#"INSERT INTO result_steps
                         (result_id, step_id, position, action, expected_result, status)
                       VALUES ($1, $2, $3, $4, $5, 'untested')
                       RETURNING id, result_id, step_id, position, action, expected_result,
                                 status, actual_result, comment"#,
                )
                .bind(result.id)
                .bind(step.id)
                .bind(step.position)
                .bind(&step.action)
                .bind(&step.expected_result)
                .fetch_one(&mut *tx)
                .await?;
                steps.push(row);
            }
        }
    } else {
        for step in &body.steps {
            let row = sqlx::query_as::<_, ResultStep>(
                r#"INSERT INTO result_steps
                     (result_id, step_id, position, action, expected_result, status,
                      actual_result, comment)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                   RETURNING id, result_id, step_id, position, action, expected_result,
                             status, actual_result, comment"#,
            )
            .bind(result.id)
            .bind(step.step_id)
            .bind(step.position)
            .bind(&step.action)
            .bind(&step.expected_result)
            .bind(&step.status)
            .bind(&step.actual_result)
            .bind(&step.comment)
            .fetch_one(&mut *tx)
            .await?;
            steps.push(row);
        }
    }

    if body.sync_run_item {
        if let Some(run_item_id) = body.run_item_id {
            sqlx::query(
                r#"UPDATE run_items
                   SET status = $2, assignee = COALESCE($3, assignee),
                       comment = CASE WHEN $4 = '' THEN comment ELSE $4 END,
                       executed_at = NOW(), updated_at = NOW()
                   WHERE id = $1"#,
            )
            .bind(run_item_id)
            .bind(&body.status)
            .bind(body.assignee.as_deref())
            .bind(&body.message)
            .execute(&mut *tx)
            .await?;
        }
    }

    tx.commit().await?;
    Ok(Json(TestResultDetail { result, steps }))
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
    let description = body.description.unwrap_or(existing.description);
    let status = body.status.unwrap_or(existing.status);
    let external_ref = body.external_ref.or(existing.external_ref);
    let meta = body.meta.unwrap_or(existing.meta);
    let started_at = body.started_at.or(existing.started_at);
    let finished_at = body.finished_at.or(existing.finished_at);

    let row = sqlx::query_as::<_, TestRun>(&format!(
        r#"UPDATE test_runs
           SET name = $2, description = $3, status = $4, external_ref = $5, meta = $6,
               started_at = $7, finished_at = $8, updated_at = NOW()
           WHERE id = $1
           RETURNING {RUN_COLS}"#
    ))
    .bind(id)
    .bind(name)
    .bind(description)
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
pub async fn delete_run_by_id(
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
