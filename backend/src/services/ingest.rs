//! Persist parsed results with bulk inserts and optional case-key matching.

use chrono::Utc;
use serde_json::Value;
use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::{BulkResultItem, BulkResultsResponse};

const SYNC_BATCH: usize = 500;

/// Threshold above which ingest prefers the job queue.
pub const ASYNC_THRESHOLD: usize = 200;

pub async fn persist_results(
    pool: &PgPool,
    run_id: Uuid,
    project_id: Uuid,
    items: &[BulkResultItem],
    source: &str,
    upsert: bool,
) -> AppResult<BulkResultsResponse> {
    ensure_run(pool, run_id, project_id).await?;

    let mut unmatched_case_keys = Vec::new();
    let mut inserted: i64 = 0;
    let mut updated: i64 = 0;

    for chunk in items.chunks(SYNC_BATCH) {
        let mut resolved = Vec::with_capacity(chunk.len());
        for item in chunk {
            let mut row = item.clone();
            row.status = normalize_status(&row.status);
            if row.case_id.is_none() {
                if let Some(ref key) = row.case_key {
                    match find_case_id(pool, project_id, key).await? {
                        Some(id) => row.case_id = Some(id),
                        None => unmatched_case_keys.push(key.clone()),
                    }
                }
            }
            resolved.push(row);
        }

        if upsert {
            for row in &resolved {
                if let Some(ref ext) = row.external_id {
                    let existing: Option<(Uuid,)> = sqlx::query_as(
                        r#"SELECT id FROM test_results
                           WHERE run_id = $1 AND external_id = $2
                           LIMIT 1"#,
                    )
                    .bind(run_id)
                    .bind(ext)
                    .fetch_optional(pool)
                    .await?;

                    if let Some((id,)) = existing {
                        sqlx::query(
                            r#"UPDATE test_results
                               SET case_id = $2, status = $3, message = $4, source = $5,
                                   duration_ms = $6, raw = $7, executed_at = $8
                               WHERE id = $1"#,
                        )
                        .bind(id)
                        .bind(row.case_id)
                        .bind(&row.status)
                        .bind(&row.message)
                        .bind(source)
                        .bind(row.duration_ms)
                        .bind(row.raw.clone().unwrap_or(Value::Null))
                        .bind(row.executed_at.unwrap_or_else(Utc::now))
                        .execute(pool)
                        .await?;
                        updated += 1;
                        continue;
                    }
                }
                insert_one(pool, run_id, row, source).await?;
                inserted += 1;
            }
        } else {
            inserted += insert_batch(pool, run_id, &resolved, source).await?;
        }
    }

    if inserted + updated > 0 {
        sqlx::query(
            r#"UPDATE test_runs
               SET status = CASE WHEN status IN ('draft', 'running', 'queued') THEN 'completed' ELSE status END,
                   finished_at = COALESCE(finished_at, NOW()),
                   updated_at = NOW()
               WHERE id = $1"#,
        )
        .bind(run_id)
        .execute(pool)
        .await?;
    }

    unmatched_case_keys.sort();
    unmatched_case_keys.dedup();

    Ok(BulkResultsResponse {
        inserted,
        updated,
        unmatched_case_keys,
        job_id: None,
    })
}

pub async fn ensure_run_exists(pool: &PgPool, run_id: Uuid) -> AppResult<Uuid> {
    let row: Option<(Uuid,)> = sqlx::query_as("SELECT project_id FROM test_runs WHERE id = $1")
        .bind(run_id)
        .fetch_optional(pool)
        .await?;
    row.map(|r| r.0)
        .ok_or_else(|| AppError::NotFound("run".into()))
}

async fn ensure_run(pool: &PgPool, run_id: Uuid, project_id: Uuid) -> AppResult<()> {
    let row: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM test_runs WHERE id = $1 AND project_id = $2")
            .bind(run_id)
            .bind(project_id)
            .fetch_optional(pool)
            .await?;
    if row.is_none() {
        return Err(AppError::NotFound("run".into()));
    }
    Ok(())
}

async fn find_case_id(pool: &PgPool, project_id: Uuid, key: &str) -> AppResult<Option<Uuid>> {
    let row: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM test_cases WHERE project_id = $1 AND key = $2")
            .bind(project_id)
            .bind(key)
            .fetch_optional(pool)
            .await?;
    Ok(row.map(|r| r.0))
}

async fn insert_one(
    pool: &PgPool,
    run_id: Uuid,
    row: &BulkResultItem,
    source: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO test_results
           (run_id, case_id, status, message, source, external_id, duration_ms, raw, executed_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"#,
    )
    .bind(run_id)
    .bind(row.case_id)
    .bind(&row.status)
    .bind(&row.message)
    .bind(source)
    .bind(&row.external_id)
    .bind(row.duration_ms)
    .bind(row.raw.clone().unwrap_or(Value::Null))
    .bind(row.executed_at.unwrap_or_else(Utc::now))
    .execute(pool)
    .await?;
    Ok(())
}

async fn insert_batch(
    pool: &PgPool,
    run_id: Uuid,
    rows: &[BulkResultItem],
    source: &str,
) -> AppResult<i64> {
    if rows.is_empty() {
        return Ok(0);
    }

    let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
        "INSERT INTO test_results (run_id, case_id, status, message, source, external_id, duration_ms, raw, executed_at) ",
    );
    qb.push_values(rows, |mut b, row| {
        b.push_bind(run_id)
            .push_bind(row.case_id)
            .push_bind(row.status.clone())
            .push_bind(row.message.clone())
            .push_bind(source.to_string())
            .push_bind(row.external_id.clone())
            .push_bind(row.duration_ms)
            .push_bind(row.raw.clone().unwrap_or(Value::Null))
            .push_bind(row.executed_at.unwrap_or_else(Utc::now));
    });

    let result = qb.build().execute(pool).await?;
    Ok(result.rows_affected() as i64)
}

fn normalize_status(s: &str) -> String {
    match s.trim().to_lowercase().as_str() {
        "pass" | "passed" | "success" | "successful" | "ok" => "passed".into(),
        "fail" | "failed" | "failure" => "failed".into(),
        "skip" | "skipped" | "ignored" => "skipped".into(),
        "error" | "errored" => "error".into(),
        "blocked" => "blocked".into(),
        other => other.to_string(),
    }
}
