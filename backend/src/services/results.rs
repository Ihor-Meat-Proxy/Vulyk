//! Append-only result history queries (case-level and run-item timelines).

use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::Row;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::{
    normalize_result_status, CaseResultsPage, CaseResultsSummary, ResultStep, TestResult,
    TestResultHistoryItem,
};

pub const RESULT_COLS: &str = "id, run_id, case_id, run_item_id, status, message, source, external_id, duration_ms, raw, assignee, elapsed_ms, executed_by, case_version, executed_at, created_at";

#[derive(Debug, Default, Deserialize)]
pub struct CaseResultsQuery {
    pub run_id: Option<Uuid>,
    pub status: Option<String>,
    pub user_id: Option<Uuid>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    /// When true, include per-result steps (default false for list speed).
    pub include_steps: Option<bool>,
}

impl CaseResultsQuery {
    pub fn limit(&self) -> i64 {
        self.limit.unwrap_or(50).clamp(1, 500)
    }

    pub fn offset(&self) -> i64 {
        self.offset.unwrap_or(0).max(0)
    }
}

pub async fn case_results_summary(
    pool: &sqlx::PgPool,
    case_id: Uuid,
) -> AppResult<CaseResultsSummary> {
    let row = sqlx::query(
        r#"SELECT
             COUNT(*)::bigint AS total,
             COUNT(*) FILTER (WHERE status = 'Passed')::bigint AS passed,
             COUNT(*) FILTER (WHERE status = 'Failed')::bigint AS failed,
             COUNT(*) FILTER (WHERE status = 'Blocked')::bigint AS blocked,
             COUNT(*) FILTER (WHERE status = 'Skipped')::bigint AS skipped,
             COUNT(*) FILTER (WHERE status = 'Retest')::bigint AS retest,
             (ARRAY_AGG(status ORDER BY executed_at DESC, created_at DESC))[1] AS latest_status,
             MAX(executed_at) AS latest_executed_at
           FROM test_results
           WHERE case_id = $1"#,
    )
    .bind(case_id)
    .fetch_one(pool)
    .await?;

    let passed: i64 = row.try_get("passed")?;
    let failed: i64 = row.try_get("failed")?;
    let decided = passed + failed;
    let pass_rate = if decided == 0 {
        0.0
    } else {
        (passed as f64) / (decided as f64) * 100.0
    };

    Ok(CaseResultsSummary {
        case_id,
        latest_status: row.try_get("latest_status")?,
        latest_executed_at: row.try_get("latest_executed_at")?,
        total_results: row.try_get("total")?,
        passed,
        failed,
        blocked: row.try_get("blocked")?,
        skipped: row.try_get("skipped")?,
        retest: row.try_get("retest")?,
        pass_rate,
    })
}

pub async fn list_case_results(
    pool: &sqlx::PgPool,
    case_id: Uuid,
    query: &CaseResultsQuery,
) -> AppResult<CaseResultsPage> {
    let exists: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM test_cases WHERE id = $1")
        .bind(case_id)
        .fetch_optional(pool)
        .await?;
    if exists.is_none() {
        return Err(AppError::NotFound("case".into()));
    }

    let status_filter = query.status.as_ref().map(|s| normalize_result_status(s));

    let limit = query.limit();
    let offset = query.offset();

    let total: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)::bigint FROM test_results
           WHERE case_id = $1
             AND ($2::uuid IS NULL OR run_id = $2)
             AND ($3::text IS NULL OR status = $3)
             AND ($4::uuid IS NULL OR executed_by = $4)
             AND ($5::timestamptz IS NULL OR executed_at >= $5)
             AND ($6::timestamptz IS NULL OR executed_at <= $6)"#,
    )
    .bind(case_id)
    .bind(query.run_id)
    .bind(status_filter.as_deref())
    .bind(query.user_id)
    .bind(query.from)
    .bind(query.to)
    .fetch_one(pool)
    .await?;

    let rows = sqlx::query(
        r#"SELECT
             tr.id, tr.run_id, tr.case_id, tr.run_item_id, tr.status, tr.message, tr.source,
             tr.external_id, tr.duration_ms, tr.raw, tr.assignee, tr.elapsed_ms,
             tr.executed_by, tr.case_version, tr.executed_at, tr.created_at,
             r.name AS run_name,
             u.display_name AS executed_by_name,
             u.email AS executed_by_email,
             (SELECT COUNT(*)::bigint FROM attachments a
                WHERE a.entity_type = 'result' AND a.entity_id = tr.id) AS attachment_count,
             (SELECT COUNT(*)::bigint FROM defects d WHERE d.result_id = tr.id) AS defect_count
           FROM test_results tr
           LEFT JOIN test_runs r ON r.id = tr.run_id
           LEFT JOIN users u ON u.id = tr.executed_by
           WHERE tr.case_id = $1
             AND ($2::uuid IS NULL OR tr.run_id = $2)
             AND ($3::text IS NULL OR tr.status = $3)
             AND ($4::uuid IS NULL OR tr.executed_by = $4)
             AND ($5::timestamptz IS NULL OR tr.executed_at >= $5)
             AND ($6::timestamptz IS NULL OR tr.executed_at <= $6)
           ORDER BY tr.executed_at DESC, tr.created_at DESC
           LIMIT $7 OFFSET $8"#,
    )
    .bind(case_id)
    .bind(query.run_id)
    .bind(status_filter.as_deref())
    .bind(query.user_id)
    .bind(query.from)
    .bind(query.to)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    let include_steps = query.include_steps.unwrap_or(false);
    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        let result = map_result_row(&row)?;
        let steps = if include_steps {
            load_result_steps(pool, result.id).await?
        } else {
            Vec::new()
        };
        items.push(TestResultHistoryItem {
            result,
            run_name: row.try_get("run_name")?,
            executed_by_name: row.try_get("executed_by_name")?,
            executed_by_email: row.try_get("executed_by_email")?,
            attachment_count: row.try_get("attachment_count")?,
            defect_count: row.try_get("defect_count")?,
            steps,
        });
    }

    let summary = case_results_summary(pool, case_id).await?;
    Ok(CaseResultsPage {
        summary,
        items,
        total,
        limit,
        offset,
    })
}

pub async fn list_run_item_history(
    pool: &sqlx::PgPool,
    run_item_id: Uuid,
    include_steps: bool,
) -> AppResult<Vec<TestResultHistoryItem>> {
    let rows = sqlx::query(
        r#"SELECT
             tr.id, tr.run_id, tr.case_id, tr.run_item_id, tr.status, tr.message, tr.source,
             tr.external_id, tr.duration_ms, tr.raw, tr.assignee, tr.elapsed_ms,
             tr.executed_by, tr.case_version, tr.executed_at, tr.created_at,
             r.name AS run_name,
             u.display_name AS executed_by_name,
             u.email AS executed_by_email,
             (SELECT COUNT(*)::bigint FROM attachments a
                WHERE a.entity_type = 'result' AND a.entity_id = tr.id) AS attachment_count,
             (SELECT COUNT(*)::bigint FROM defects d WHERE d.result_id = tr.id) AS defect_count
           FROM test_results tr
           LEFT JOIN test_runs r ON r.id = tr.run_id
           LEFT JOIN users u ON u.id = tr.executed_by
           WHERE tr.run_item_id = $1
           ORDER BY tr.executed_at ASC, tr.created_at ASC"#,
    )
    .bind(run_item_id)
    .fetch_all(pool)
    .await?;

    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        let result = map_result_row(&row)?;
        let steps = if include_steps {
            load_result_steps(pool, result.id).await?
        } else {
            Vec::new()
        };
        items.push(TestResultHistoryItem {
            result,
            run_name: row.try_get("run_name")?,
            executed_by_name: row.try_get("executed_by_name")?,
            executed_by_email: row.try_get("executed_by_email")?,
            attachment_count: row.try_get("attachment_count")?,
            defect_count: row.try_get("defect_count")?,
            steps,
        });
    }
    Ok(items)
}

pub async fn list_run_results_history(
    pool: &sqlx::PgPool,
    run_id: Uuid,
    case_id: Option<Uuid>,
) -> AppResult<Vec<TestResultHistoryItem>> {
    let rows = sqlx::query(
        r#"SELECT
             tr.id, tr.run_id, tr.case_id, tr.run_item_id, tr.status, tr.message, tr.source,
             tr.external_id, tr.duration_ms, tr.raw, tr.assignee, tr.elapsed_ms,
             tr.executed_by, tr.case_version, tr.executed_at, tr.created_at,
             r.name AS run_name,
             u.display_name AS executed_by_name,
             u.email AS executed_by_email,
             (SELECT COUNT(*)::bigint FROM attachments a
                WHERE a.entity_type = 'result' AND a.entity_id = tr.id) AS attachment_count,
             (SELECT COUNT(*)::bigint FROM defects d WHERE d.result_id = tr.id) AS defect_count
           FROM test_results tr
           LEFT JOIN test_runs r ON r.id = tr.run_id
           LEFT JOIN users u ON u.id = tr.executed_by
           WHERE tr.run_id = $1
             AND ($2::uuid IS NULL OR tr.case_id = $2)
           ORDER BY tr.executed_at ASC, tr.created_at ASC"#,
    )
    .bind(run_id)
    .bind(case_id)
    .fetch_all(pool)
    .await?;

    let mut items = Vec::with_capacity(rows.len());
    for row in rows {
        items.push(TestResultHistoryItem {
            result: map_result_row(&row)?,
            run_name: row.try_get("run_name")?,
            executed_by_name: row.try_get("executed_by_name")?,
            executed_by_email: row.try_get("executed_by_email")?,
            attachment_count: row.try_get("attachment_count")?,
            defect_count: row.try_get("defect_count")?,
            steps: Vec::new(),
        });
    }
    Ok(items)
}

fn map_result_row(row: &sqlx::postgres::PgRow) -> AppResult<TestResult> {
    Ok(TestResult {
        id: row.try_get("id")?,
        run_id: row.try_get("run_id")?,
        case_id: row.try_get("case_id")?,
        run_item_id: row.try_get("run_item_id")?,
        status: row.try_get("status")?,
        message: row.try_get("message")?,
        source: row.try_get("source")?,
        external_id: row.try_get("external_id")?,
        duration_ms: row.try_get("duration_ms")?,
        raw: row.try_get("raw")?,
        assignee: row.try_get("assignee")?,
        elapsed_ms: row.try_get("elapsed_ms")?,
        executed_by: row.try_get("executed_by")?,
        case_version: row.try_get("case_version")?,
        executed_at: row.try_get("executed_at")?,
        created_at: row.try_get("created_at")?,
    })
}

pub async fn load_result_steps(pool: &sqlx::PgPool, result_id: Uuid) -> AppResult<Vec<ResultStep>> {
    Ok(sqlx::query_as::<_, ResultStep>(
        r#"SELECT id, result_id, step_id, position, action, expected_result,
                  status, actual_result, comment
           FROM result_steps WHERE result_id = $1 ORDER BY position"#,
    )
    .bind(result_id)
    .fetch_all(pool)
    .await?)
}
