//! Persist parsed results with bulk inserts and optional case-key matching.
//! Results are append-only: every ingest inserts new history rows (never UPDATE).
//! CI lifecycle runs (`lifecycle = 'ci'`) move open → in_progress on first result
//! and do **not** auto-close; legacy runs still complete on ingest.
//!
//! Unmatched results (no key / automation_id / external_key hit) follow the
//! project's `ci_unmatched_policy`: `off`, `create_passed`, `create_all`, or
//! `fail_unmatched`. Request `dry_run` previews auto-creates without persisting them.

use std::collections::{HashMap, HashSet};

use chrono::Utc;
use serde_json::Value;
use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::{
    normalize_result_status, AutoCreatedCaseSummary, BulkResultItem, BulkResultsResponse,
    CiUnmatchedPolicy, PASSED,
};
use crate::services::case_keys::{allocate_case_key, normalize_case_key};
use crate::services::cases::filter_case_ids_in_project;

const SYNC_BATCH: usize = 500;

/// Threshold above which ingest prefers the job queue.
pub const ASYNC_THRESHOLD: usize = 200;

/// Project key used to resolve compact JUnit names like `vqa110` → `VQA-110`.
pub async fn project_key(pool: &PgPool, project_id: Uuid) -> AppResult<String> {
    sqlx::query_scalar::<_, String>("SELECT key FROM projects WHERE id = $1")
        .bind(project_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("project {project_id}")))
}

async fn load_ci_unmatched_policy(pool: &PgPool, project_id: Uuid) -> AppResult<CiUnmatchedPolicy> {
    let raw: Option<String> =
        sqlx::query_scalar("SELECT ci_unmatched_policy FROM projects WHERE id = $1")
            .bind(project_id)
            .fetch_optional(pool)
            .await?;
    let raw = raw.ok_or_else(|| AppError::NotFound(format!("project {project_id}")))?;
    Ok(CiUnmatchedPolicy::parse(&raw).unwrap_or_default())
}

fn should_auto_create(policy: CiUnmatchedPolicy, status: &str) -> bool {
    match policy {
        CiUnmatchedPolicy::CreateAll => true,
        CiUnmatchedPolicy::CreatePassed => status == PASSED,
        CiUnmatchedPolicy::Off | CiUnmatchedPolicy::FailUnmatched => false,
    }
}

fn draft_title_for(row: &BulkResultItem) -> String {
    row.title
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or_else(|| {
            row.automation_id
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
        })
        .or_else(|| {
            row.case_key
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
        })
        .unwrap_or("CI auto-created case")
        .to_string()
}

/// Create a draft case for an unmatched ingest row. Returns `(case_id, allocated_key, summary)`.
async fn create_draft_case_for_result(
    pool: &PgPool,
    project_id: Uuid,
    row: &BulkResultItem,
    matched_by: &str,
) -> AppResult<(Uuid, String, AutoCreatedCaseSummary)> {
    let title = draft_title_for(row);
    let automation_id = row
        .automation_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_ascii_uppercase());
    // Preserve the attempted case key as external_key when it differs from automation_id.
    let external_key = row
        .case_key
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_ascii_uppercase())
        .filter(|k| Some(k.as_str()) != automation_id.as_deref());

    let mut tx = pool.begin().await?;
    let (key, _) = allocate_case_key(&mut tx, project_id).await?;
    let case_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO test_cases
             (project_id, suite_id, key, title, description, precondition, priority, status,
              external_key, automation_id, sort_order)
           VALUES ($1, NULL, $2, $3, '', '', 'medium', 'draft', $4, $5, 0)
           RETURNING id"#,
    )
    .bind(project_id)
    .bind(&key)
    .bind(&title)
    .bind(external_key.as_deref())
    .bind(automation_id.as_deref())
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;

    Ok((
        case_id,
        key.clone(),
        AutoCreatedCaseSummary {
            case_id: Some(case_id),
            key: Some(key),
            title,
            automation_id,
            matched_by: matched_by.to_string(),
        },
    ))
}

pub async fn persist_results(
    pool: &PgPool,
    run_id: Uuid,
    project_id: Uuid,
    items: &[BulkResultItem],
    source: &str,
    _upsert: bool,
    dry_run: bool,
) -> AppResult<BulkResultsResponse> {
    let (status, lifecycle) = ensure_run(pool, run_id, project_id).await?;
    let policy = load_ci_unmatched_policy(pool, project_id).await?;

    // Closed CI runs reject further result posts.
    if lifecycle == "ci" && matches!(status.as_str(), "closed" | "completed" | "finished") {
        return Err(AppError::Conflict(
            "run is closed; further results are rejected".into(),
        ));
    }

    let mut unmatched_case_keys = Vec::new();
    let mut auto_created_cases: Vec<AutoCreatedCaseSummary> = Vec::new();
    // Labels already auto-created in this request → reuse case id.
    let mut created_by_label: HashMap<String, Uuid> = HashMap::new();
    // Labels already previewed under dry_run (no DB row).
    let mut dry_run_labels: HashSet<String> = HashSet::new();
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
            if let Some(ref aid) = row.automation_id {
                let aid = aid.trim();
                if aid.is_empty() {
                    row.automation_id = None;
                } else {
                    let aid = aid.to_ascii_uppercase();
                    row.automation_id = Some(aid.clone());
                    if row.case_id.is_none() {
                        keys_needed.push(aid);
                    }
                }
            }
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

        let mut by_key = lookup_cases_by_keys(pool, project_id, &keys_needed).await?;
        // Merge cases created earlier in this request.
        for (label, id) in &created_by_label {
            by_key.entry(label.clone()).or_insert(*id);
        }
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
            } else if row.case_key.is_some() || row.automation_id.is_some() {
                // Prefer case key match, then automation_id (same order as lookup).
                let matched = row
                    .case_key
                    .as_ref()
                    .and_then(|k| by_key.get(k))
                    .or_else(|| row.automation_id.as_ref().and_then(|a| by_key.get(a)));
                match matched {
                    Some(id) => row.case_id = Some(*id),
                    None => {
                        let label = row
                            .case_key
                            .clone()
                            .or_else(|| row.automation_id.clone())
                            .unwrap_or_else(|| "unknown".into());

                        if should_auto_create(policy, &row.status) {
                            if let Some(existing) = created_by_label.get(&label) {
                                row.case_id = Some(*existing);
                            } else if dry_run {
                                if dry_run_labels.insert(label.clone()) {
                                    auto_created_cases.push(AutoCreatedCaseSummary {
                                        case_id: None,
                                        key: None,
                                        title: draft_title_for(&row),
                                        automation_id: row.automation_id.clone(),
                                        matched_by: label.clone(),
                                    });
                                }
                                // Preview only — do not persist create or attach result.
                                continue;
                            } else {
                                let (case_id, _key, summary) =
                                    create_draft_case_for_result(pool, project_id, &row, &label)
                                        .await?;
                                created_by_label.insert(label.clone(), case_id);
                                // Index under all lookup tokens so later rows in the chunk hit.
                                by_key.insert(label.clone(), case_id);
                                if let Some(ref aid) = row.automation_id {
                                    by_key.entry(aid.clone()).or_insert(case_id);
                                }
                                if let Some(ref ck) = row.case_key {
                                    by_key.entry(ck.clone()).or_insert(case_id);
                                }
                                auto_created_cases.push(summary);
                                row.case_id = Some(case_id);
                            }
                        } else {
                            unmatched_case_keys.push(label);
                            // Design: do not persist unmatched keys as orphan results.
                            continue;
                        }
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
    // CI ingest historically wrote only test_results; keep run_items in sync so
    // Run detail progress/items match Reports (audit F1).
    if let Err(e) =
        crate::services::runs::upsert_run_items_for_statuses(pool, run_id, &matched_case_updates)
            .await
    {
        tracing::warn!(error = %e, %run_id, "upsert_run_items_for_statuses after ingest failed");
    }
    // Keep denormalized case-list column in sync (review #11).
    let case_ids: Vec<Uuid> = matched_case_updates.iter().map(|(id, _)| *id).collect();
    crate::services::results::refresh_last_run_status(pool, &case_ids).await?;

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
    }

    // Always recompute iteration status (including zero-match / empty ingest)
    // so find-or-create no longer leaves a stuck Running shell.
    if let Some(iteration_id) =
        sqlx::query_scalar::<_, Option<Uuid>>("SELECT iteration_id FROM test_runs WHERE id = $1")
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

    unmatched_case_keys.sort();
    unmatched_case_keys.dedup();

    if policy == CiUnmatchedPolicy::FailUnmatched && !unmatched_case_keys.is_empty() {
        return Err(AppError::BadRequest(format!(
            "unmatched case keys (ci_unmatched_policy=fail_unmatched): {}",
            unmatched_case_keys.join(", ")
        )));
    }

    if inserted > 0 {
        crate::services::outbound_webhooks::enqueue_event(
            pool,
            project_id,
            crate::services::outbound_webhooks::EVENT_RESULT_CREATED,
            serde_json::json!({ "inserted": inserted, "run_id": run_id }),
        )
        .await;
    }

    Ok(BulkResultsResponse {
        inserted,
        updated,
        unmatched_case_keys,
        job_id: None,
        idempotent_replays,
        auto_created_cases,
        dry_run,
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

/// Resolve lookup tokens in order: `test_cases.key`, then `automation_id`,
/// then `external_key` for still-unmatched tokens. Case-insensitive (uppercase).
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

    let missing_after_key: Vec<String> = unique
        .iter()
        .filter(|k| !map.contains_key(*k))
        .cloned()
        .collect();
    if !missing_after_key.is_empty() {
        let by_automation: Vec<(Uuid, String)> = sqlx::query_as(
            r#"SELECT id, upper(automation_id) FROM test_cases
               WHERE project_id = $1 AND upper(automation_id) = ANY($2)"#,
        )
        .bind(project_id)
        .bind(&missing_after_key)
        .fetch_all(pool)
        .await?;
        for (id, aid) in by_automation {
            map.entry(aid).or_insert(id);
        }
    }

    let missing_after_automation: Vec<String> = unique
        .into_iter()
        .filter(|k| !map.contains_key(k))
        .collect();
    if !missing_after_automation.is_empty() {
        let by_external: Vec<(Uuid, String)> = sqlx::query_as(
            r#"SELECT id, upper(external_key) FROM test_cases
               WHERE project_id = $1 AND upper(external_key) = ANY($2)"#,
        )
        .bind(project_id)
        .bind(&missing_after_automation)
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

    // Deduplicate within the batch first (last row wins for matched status).
    let mut seen_ext: HashSet<(String, String)> = HashSet::new();
    let mut to_insert = Vec::with_capacity(rows.len());
    let mut replays = 0i64;
    for row in rows.iter().rev() {
        if let Some(ref ext) = row.external_id {
            if !ext.is_empty() {
                let key = (ext.clone(), row.status.clone());
                if !seen_ext.insert(key) {
                    replays += 1;
                    continue;
                }
            }
        }
        to_insert.push(row);
    }
    to_insert.reverse();

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

    let attempted = to_insert.len() as i64;
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
    // Unique index `test_results_run_external_status_uidx` (review #6) — concurrent
    // workers racing the old SELECT-then-INSERT path become no-op replays.
    qb.push(
        " ON CONFLICT (run_id, external_id, status) \
          WHERE (external_id IS NOT NULL AND btrim(external_id) <> '') \
          DO NOTHING \
          RETURNING case_id, status",
    );

    let inserted_rows: Vec<(Option<Uuid>, String)> = qb.build_query_as().fetch_all(pool).await?;
    let inserted = inserted_rows.len() as i64;
    replays += attempted - inserted;
    let matched: Vec<(Uuid, String)> = inserted_rows
        .into_iter()
        .filter_map(|(id, status)| id.map(|id| (id, status)))
        .collect();
    Ok((inserted, replays, matched))
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

    #[test]
    fn should_auto_create_respects_policy() {
        assert!(!should_auto_create(CiUnmatchedPolicy::Off, PASSED));
        assert!(should_auto_create(CiUnmatchedPolicy::CreatePassed, PASSED));
        assert!(!should_auto_create(
            CiUnmatchedPolicy::CreatePassed,
            "Failed"
        ));
        assert!(should_auto_create(CiUnmatchedPolicy::CreateAll, "Failed"));
        assert!(!should_auto_create(
            CiUnmatchedPolicy::FailUnmatched,
            PASSED
        ));
    }
}
