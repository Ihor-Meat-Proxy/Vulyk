//! Background worker loop for ingest and CI-related jobs.

use std::time::Duration;

use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::ingest::parse_payload;
use crate::services::{ingest as ingest_svc, jobs as jobs_svc};

#[derive(Debug, Deserialize)]
struct IngestJobPayload {
    run_id: Uuid,
    project_id: Uuid,
    format: String,
    content: String,
    source: String,
    #[serde(default)]
    upsert: bool,
}

pub fn spawn_worker(pool: PgPool) {
    let worker_id = format!("api-{}", Uuid::new_v4());
    tokio::spawn(async move {
        tracing::info!(%worker_id, "job worker started");
        loop {
            match tick(&pool, &worker_id).await {
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

async fn tick(pool: &PgPool, worker_id: &str) -> Result<bool, String> {
    let Some(job) = jobs_svc::claim_next(pool, worker_id)
        .await
        .map_err(|e| e.to_string())?
    else {
        return Ok(false);
    };

    tracing::info!(job_id = %job.id, kind = %job.kind, "claimed job");
    let result = match job.kind.as_str() {
        "ingest_junit" | "ingest_json" | "ingest" => handle_ingest(pool, &job.payload).await,
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

async fn handle_ingest(
    pool: &PgPool,
    payload: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let payload: IngestJobPayload =
        serde_json::from_value(payload.clone()).map_err(|e| e.to_string())?;
    let parsed = parse_payload(&payload.format, &payload.content)?;
    let report = ingest_svc::persist_results(
        pool,
        payload.run_id,
        payload.project_id,
        &parsed.results,
        &payload.source,
        payload.upsert,
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(json!({
        "inserted": report.inserted,
        "updated": report.updated,
        "unmatched_case_keys": report.unmatched_case_keys,
        "parsed": parsed.results.len(),
    }))
}
