//! Persist parsed results with bulk inserts and optional case-key matching.
//! Results are append-only: every ingest inserts new history rows (never UPDATE).
//! CI lifecycle runs (`lifecycle = 'ci'`) move open → in_progress on first result
//! and do **not** auto-close; legacy runs still complete on ingest.

use std::collections::{HashMap, HashSet};

use chrono::Utc;
use serde_json::Value;
use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::{normalize_result_status, BulkResultItem, BulkResultsResponse};
use crate::services::case_keys::normalize_case_key;
use crate::services::cases::filter_case_ids_in_project;

const SYNC_BATCH: usize = 500;

/// Threshold above which ingest prefers the job queue.
pub const ASYNC_THRESHOLD: usize = 200;

pub async fn persist_results(
    pool: &PgPool,
    run_id: Uuid,
    project_id: Uuid,
    items: &[BulkResultItem],
    source: &str,
    _upsert: bool,
) -> AppResult<BulkResultsResponse> {
    let (status, lifecycle) = ensure_run(pool, run_id, project_id).await?;

    // Closed CI runs reject further result posts.
    if lifecycle == "ci" && matches!(status.as_str(), "closed" | "completed" | "finished") {
        return Err(AppError::Conflict(
            "run is closed; further results are rejected".into(),
        ));
    }

    let mut unmatched_case_keys = Vec::new();
    let mut inserted: i64 = 0;
    let mut idempotent_replays: i64 = 0;
    // Results are immutable; `updated` stays 0 for API compatibility.
    let updated: i64 = 0;
    let mut matched_case_updates: Vec<(Uuid, String)> = Vec::new();

    for chunk in items.chunks(SYNC_BATCH) {
        let mut prepared: Vec<BulkResultItem> = Vec::with_capacity(chunk.len());
        let mut keys_needed: Vec<String> = Vec::new();
        for item in chunk {
            let mut row = item.clone();
            row.status = normalize_result_status(&row.status);
            if row.case_id.is_none() {
                if row.case_key.is_none() {
                    row.case_key = crate::ingest::resolve_case_key(
                        None,
                        row.title.as_deref(),
                        row.external_id.as_deref(),
                    );
                }
                if let Some(ref key) = row.case_key {
                    let key = normalize_case_key(key);
                    row.case_key = Some(key.clone());
                    keys_needed.push(key);
                }
            }
            prepared.push(row);
        }

        let by_key = lookup_cases_by_keys(pool, project_id, &keys_needed).await?;
        let provided_ids: Vec<Uuid> = prepared.iter().filter_map(|r| r.case_id).collect();
        let allowed_ids: HashSet<Uuid> =
            filter_case_ids_in_project(pool, project_id, &provided_ids)
                .await?
                .into_iter()
                .collect();
        let mut resolved = Vec::with_capacity(prepared.len());
        for mut row in prepared {
            if let Some(cid) = row.case_id {
                // Reject cross-project / unknown case UUIDs (review #37).
                if !allowed_ids.contains(&cid) {
                    unmatched_case_keys.push(format!("case_id:{cid}"));
                    continue;
                }
            } else if let Some(ref key) = row.case_key {
                match by_key.get(key) {
                    Some(id) => row.case_id = Some(*id),
                    None => {
                        unmatched_case_keys.push(key.clone());
                        // Design: do not persist unmatched keys as orphan results.
                        continue;
                    }
                }
            }
            resolved.push(row);
        }

        let (ins, replays, matched) = insert_batch(pool, run_id, &resolved, source).await?;
        inserted += ins;
        idempotent_replays += replays;
        matched_case_updates.extend(matched);
    }

    refresh_last_automation(pool, run_id, &matched_case_updates).await?;

    if inserted > 0 || idempotent_replays > 0 {
        if lifecycle == "ci" {
            // First successful persist while open → in_progress; never auto-close.
            sqlx::query(
                r#"UPDATE test_runs
                   SET status = CASE
                         WHEN status IN ('open', 'draft', 'queued') THEN 'in_progress'
                         ELSE status
                       END,
                       started_at = COALESCE(started_at, NOW()),
                       updated_at = NOW()
                   WHERE id = $1"#,
            )
            .bind(run_id)
            .execute(pool)
            .await?;
        } else {
            sqlx::query(
                r#"UPDATE test_runs
                   SET status = CASE WHEN status IN ('draft', 'running', 'queued', 'open', 'in_progress')
                                     THEN 'completed' ELSE status END,
                       finished_at = COALESCE(finished_at, NOW()),
                       updated_at = NOW()
                   WHERE id = $1"#,
            )
            .bind(run_id)
            .execute(pool)
            .await?;
        }
        if let Err(e) = crate::services::campaigns::refresh_run_counters(pool, run_id).await {
            tracing::warn!(error = %e, %run_id, "refresh_run_counters after ingest failed");
        }
        if let Some(iteration_id) = sqlx::query_scalar::<_, Option<Uuid>>(
            "SELECT iteration_id FROM test_runs WHERE id = $1",
        )
        .bind(run_id)
        .fetch_optional(pool)
        .await?
        .flatten()
        {
            if let Err(e) =
                crate::services::campaigns::sync_iteration_item_statuses(pool, iteration_id).await
            {
                tracing::warn!(
                    error = %e,
                    %run_id,
                    %iteration_id,
                    "sync_iteration_item_statuses after ingest failed"
                );
            }
        }
    }

    unmatched_case_keys.sort();
    unmatched_case_keys.dedup();

    Ok(BulkResultsResponse {
        inserted,
        updated,
        unmatched_case_keys,
        job_id: None,
        idempotent_replays,
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

/// Returns `(status, lifecycle)`.
async fn ensure_run(pool: &PgPool, run_id: Uuid, project_id: Uuid) -> AppResult<(String, String)> {
    let row: Option<(String, String)> = sqlx::query_as(
        r#"SELECT status, COALESCE(lifecycle, 'legacy')
           FROM test_runs WHERE id = $1 AND project_id = $2"#,
    )
    .bind(run_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await?;
    row.ok_or_else(|| AppError::NotFound("run".into()))
}

/// Resolve case keys (and external_key fallback) in two queries per chunk.
/// Prefer matching `test_cases.key`; fall back to `external_key` only when the
/// lookup token was not already claimed by a primary key (same as the prior
/// per-row lookup order). Keys are matched case-insensitively (uppercase).
async fn lookup_cases_by_keys(
    pool: &PgPool,
    project_id: Uuid,
    keys: &[String],
) -> AppResult<HashMap<String, Uuid>> {
    if keys.is_empty() {
        return Ok(HashMap::new());
    }
    let mut unique: Vec<String> = keys.to_vec();
    unique.sort();
    unique.dedup();

    let by_key: Vec<(Uuid, String)> = sqlx::query_as(
        r#"SELECT id, upper(key) FROM test_cases
           WHERE project_id = $1 AND upper(key) = ANY($2)"#,
    )
    .bind(project_id)
    .bind(&unique)
    .fetch_all(pool)
    .await?;

    let mut map: HashMap<String, Uuid> = by_key.into_iter().map(|(id, k)| (k, id)).collect();

    let missing: Vec<String> = unique
        .into_iter()
        .filter(|k| !map.contains_key(k))
        .collect();
    if !missing.is_empty() {
        let by_external: Vec<(Uuid, String)> = sqlx::query_as(
            r#"SELECT id, upper(external_key) FROM test_cases
               WHERE project_id = $1 AND upper(external_key) = ANY($2)"#,
        )
        .bind(project_id)
        .bind(&missing)
        .fetch_all(pool)
        .await?;
        for (id, ext) in by_external {
            map.entry(ext).or_insert(id);
        }
    }
    Ok(map)
}

/// Last status per case wins (matches the previous sequential UPDATE loop).
async fn refresh_last_automation(
    pool: &PgPool,
    run_id: Uuid,
    matched: &[(Uuid, String)],
) -> AppResult<()> {
    if matched.is_empty() {
        return Ok(());
    }
    let mut by_case: HashMap<Uuid, String> = HashMap::new();
    for (case_id, status) in matched {
        by_case.insert(*case_id, status.clone());
    }
    let (case_ids, statuses): (Vec<Uuid>, Vec<String>) = by_case.into_iter().unzip();

    sqlx::query(
        r#"UPDATE test_cases AS tc SET
             last_automation_status = v.status,
             last_automation_run_id = $3,
             last_automation_at = NOW(),
             updated_at = NOW()
           FROM UNNEST($1::uuid[], $2::text[]) AS v(id, status)
           WHERE tc.id = v.id"#,
    )
    .bind(&case_ids)
    .bind(&statuses)
    .bind(run_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Returns `(inserted, idempotent_replays, matched_case_status_updates)`.
async fn insert_batch(
    pool: &PgPool,
    run_id: Uuid,
    rows: &[BulkResultItem],
    source: &str,
) -> AppResult<(i64, i64, Vec<(Uuid, String)>)> {
    if rows.is_empty() {
        return Ok((0, 0, vec![]));
    }

    let mut to_insert = Vec::with_capacity(rows.len());
    let mut replays = 0i64;
    for row in rows {
        if let Some(ref ext) = row.external_id {
            if !ext.is_empty() {
                let exists: Option<(Uuid,)> = sqlx::query_as(
                    r#"SELECT id FROM test_results
                       WHERE run_id = $1 AND external_id = $2 AND status = $3
                       LIMIT 1"#,
                )
                .bind(run_id)
                .bind(ext)
                .bind(&row.status)
                .fetch_optional(pool)
                .await?;
                if exists.is_some() {
                    replays += 1;
                    continue;
                }
            }
        }
        to_insert.push(row);
    }

    if to_insert.is_empty() {
        return Ok((0, replays, vec![]));
    }

    let case_ids: Vec<Uuid> = to_insert.iter().filter_map(|r| r.case_id).collect();
    let version_map: std::collections::HashMap<Uuid, i32> = if case_ids.is_empty() {
        std::collections::HashMap::new()
    } else {
        let pairs: Vec<(Uuid, i32)> =
            sqlx::query_as("SELECT id, version FROM test_cases WHERE id = ANY($1)")
                .bind(&case_ids)
                .fetch_all(pool)
                .await?;
        pairs.into_iter().collect()
    };

    let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(
        "INSERT INTO test_results (run_id, case_id, status, message, source, external_id, duration_ms, raw, executed_at, case_version) ",
    );
    qb.push_values(&to_insert, |mut b, row| {
        let ver = row.case_id.and_then(|id| version_map.get(&id).copied());
        b.push_bind(run_id)
            .push_bind(row.case_id)
            .push_bind(row.status.clone())
            .push_bind(row.message.clone())
            .push_bind(source.to_string())
            .push_bind(row.external_id.clone())
            .push_bind(row.duration_ms)
            .push_bind(row.raw.clone().unwrap_or(Value::Null))
            .push_bind(row.executed_at.unwrap_or_else(Utc::now))
            .push_bind(ver);
    });

    let result = qb.build().execute(pool).await?;
    let matched: Vec<(Uuid, String)> = to_insert
        .iter()
        .filter_map(|r| r.case_id.map(|id| (id, r.status.clone())))
        .collect();
    Ok((result.rows_affected() as i64, replays, matched))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn last_automation_dedupes_keeping_last_status() {
        let c1 = Uuid::nil();
        let c2 = Uuid::from_u128(2);
        let matched: Vec<(Uuid, String)> = vec![
            (c1, "passed".into()),
            (c2, "failed".into()),
            (c1, "failed".into()),
        ];
        let mut by_case: HashMap<Uuid, String> = HashMap::new();
        for (case_id, status) in &matched {
            by_case.insert(*case_id, status.clone());
        }
        assert_eq!(by_case.get(&c1).map(String::as_str), Some("failed"));
        assert_eq!(by_case.get(&c2).map(String::as_str), Some("failed"));
        assert_eq!(by_case.len(), 2);
    }
}
