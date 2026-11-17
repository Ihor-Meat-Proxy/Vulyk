//! Background worker loop for ingest and CI-related jobs.

use std::time::Duration;

use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::crypto::SecretsKey;
use crate::ingest::parse_payload_with_project;
use crate::services::{ingest as ingest_svc, jobs as jobs_svc, outbound_webhooks};

#[derive(Debug, Deserialize)]
struct IngestJobPayload {
    run_id: Uuid,
    project_id: Uuid,
    format: String,
    content: String,
    source: String,
    #[serde(default)]
    upsert: bool,
    #[serde(default)]
    dry_run: bool,
}

pub fn spawn_worker(pool: PgPool, secrets: SecretsKey) {
    let worker_id = format!("api-{}", Uuid::new_v4());
    tokio::spawn(async move {
        tracing::info!(%worker_id, "job worker started");
        // Startup sweep: hard-killed reporters leave in_progress forever.
        let _ = expire_stale_in_progress_runs(&pool).await;
        let _ = expire_stale_queued_ci_runs(&pool).await;
        loop {
            match tick(&pool, &secrets, &worker_id).await {
                Ok(true) => { /* keep draining */ }
                Ok(false) => {
                    tokio::time::sleep(Duration::from_millis(750)).await;
                }
                Err(err) => {
                    tracing::error!(error = %err, "job worker tick failed");
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }
        }
    });
}

/// Claim and run at most one pending job (used by integration tests that share
/// a Postgres with other test binaries whose workers may die mid-job).
pub async fn process_one(
    pool: &PgPool,
    secrets: &SecretsKey,
    worker_id: &str,
) -> Result<bool, String> {
    tick(pool, secrets, worker_id).await
}

async fn tick(pool: &PgPool, secrets: &SecretsKey, worker_id: &str) -> Result<bool, String> {
    // Best-effort: mark abandoned Queued CI runs (never started by the workflow)
    // and age out stale in_progress / running runs (hard-killed reporters).
    let _ = expire_stale_queued_ci_runs(pool).await;
    let _ = expire_stale_in_progress_runs(pool).await;

    let Some(job) = jobs_svc::claim_next(pool, worker_id)
        .await
        .map_err(|e| e.to_string())?
    else {
        return Ok(false);
    };

    tracing::info!(job_id = %job.id, kind = %job.kind, "claimed job");
    let result = match job.kind.as_str() {
        "ingest_junit" | "ingest_json" | "ingest" => handle_ingest(pool, &job.payload).await,
        "outbound_webhook_deliver" => {
            let delivery_id = job
                .payload
                .get("delivery_id")
                .and_then(|v| v.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
                .ok_or_else(|| "missing delivery_id".to_string())?;
            outbound_webhooks::deliver_one(pool, secrets, job.id, delivery_id)
                .await
                .map(|()| json!({ "delivery_id": delivery_id }))
        }
        other => Err(format!("unknown job kind '{other}'")),
    };

    match result {
        Ok(value) => {
            jobs_svc::complete(pool, job.id, value)
                .await
                .map_err(|e| e.to_string())?;
        }
        Err(err) => {
            tracing::warn!(job_id = %job.id, error = %err, "job failed");
            jobs_svc::fail(pool, job.id, &err, true)
                .await
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(true)
}

/// Default: 2 hours. Override with `VULYK_CI_QUEUED_TIMEOUT_SECS`.
fn queued_ci_timeout_secs() -> i64 {
    std::env::var("VULYK_CI_QUEUED_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|v| *v > 0)
        .unwrap_or(7200)
}

/// Hours of inactivity before an `in_progress`/`running` run is auto-cancelled.
/// Default 24. Set `VULYK_STALE_RUN_TTL_HOURS=0` to disable.
pub fn stale_run_ttl_hours() -> i64 {
    std::env::var("VULYK_STALE_RUN_TTL_HOURS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(24)
}

async fn expire_stale_queued_ci_runs(pool: &PgPool) -> Result<u64, String> {
    let secs = queued_ci_timeout_secs();
    let result = sqlx::query(
        r#"
        WITH stale AS (
            UPDATE test_runs
            SET status = 'failed',
                finished_at = COALESCE(finished_at, NOW()),
                updated_at = NOW(),
                meta = COALESCE(meta, '{}'::jsonb) || jsonb_build_object(
                    'error', 'queued CI run timed out waiting for workflow'
                )
            WHERE lifecycle = 'ci'
              AND status = 'queued'
              AND created_at < NOW() - make_interval(secs => $1::int)
            RETURNING id
        )
        UPDATE ci_dispatches d
        SET status = 'failed',
            last_error = COALESCE(last_error, 'queued CI run timed out waiting for workflow'),
            updated_at = NOW()
        FROM stale
        WHERE d.run_id = stale.id
          AND d.status IN ('queued', 'dispatched')
        "#,
    )
    .bind(secs as i32)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    Ok(result.rows_affected())
}

/// Cancel `in_progress` / `running` runs whose `updated_at` is older than the TTL.
/// Used by the job worker, startup sweep, and lazy cancel on list/get.
/// Returns the number of runs cancelled. No-op when TTL ≤ 0.
pub async fn expire_stale_in_progress_runs(pool: &PgPool) -> Result<u64, String> {
    let hours = stale_run_ttl_hours();
    if hours <= 0 {
        return Ok(0);
    }
    let result = sqlx::query(
        r#"
        UPDATE test_runs
        SET status = 'cancelled',
            finished_at = COALESCE(finished_at, NOW()),
            updated_at = NOW(),
            meta = COALESCE(meta, '{}'::jsonb) || jsonb_build_object(
                'cancelled', true,
                'cancel_reason', 'stale_ttl',
                'cancel_message', 'auto-cancelled: no activity within stale-run TTL'
            )
        WHERE status IN ('in_progress', 'running')
          AND updated_at < NOW() - make_interval(hours => $1::int)
        "#,
    )
    .bind(hours as i32)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;
    let n = result.rows_affected();
    if n > 0 {
        tracing::info!(
            count = n,
            ttl_hours = hours,
            "auto-cancelled stale in_progress runs"
        );
    }
    Ok(n)
}

async fn handle_ingest(
    pool: &PgPool,
    payload: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let payload: IngestJobPayload =
        serde_json::from_value(payload.clone()).map_err(|e| e.to_string())?;
    let project_key = ingest_svc::project_key(pool, payload.project_id)
        .await
        .map_err(|e| e.to_string())?;
    let parsed = parse_payload_with_project(&payload.format, &payload.content, Some(&project_key))?;
    let report = ingest_svc::persist_results(
        pool,
        payload.run_id,
        payload.project_id,
        &parsed.results,
        &payload.source,
        payload.upsert,
        payload.dry_run,
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(json!({
        "inserted": report.inserted,
        "updated": report.updated,
        "unmatched_case_keys": report.unmatched_case_keys,
        "parsed": parsed.results.len(),
        "auto_created_cases": report.auto_created_cases,
        "dry_run": report.dry_run,
    }))
}
