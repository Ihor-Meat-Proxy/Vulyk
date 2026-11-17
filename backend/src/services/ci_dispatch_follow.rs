//! Background follow-up for Jenkins / Woodpecker dispatches until a terminal status.
//!
//! Provider `dispatch()` only waits long enough to leave the queue / pending state.
//! This module keeps polling the concrete build/pipeline and updates `ci_dispatches`
//! (and the linked run when present).

use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppResult;
use crate::integrations::{jenkins, woodpecker};
use crate::models::CiDispatch;
use crate::services::jobs as jobs_svc;

const FOLLOW_KIND: &str = "ci_dispatch_follow";
/// Enough attempts for multi-minute builds (retry backoff grows with attempts).
const FOLLOW_MAX_ATTEMPTS: i32 = 90;

#[derive(Debug, Deserialize)]
struct FollowPayload {
    dispatch_id: Uuid,
}

/// Enqueue a follow-up job when Jenkins/Woodpecker returned a non-terminal status
/// with a concrete external id (build/pipeline number).
pub async fn enqueue_if_needed(
    pool: &PgPool,
    dispatch: &CiDispatch,
    provider: &str,
) -> AppResult<()> {
    if !matches!(provider, "jenkins" | "woodpecker") {
        return Ok(());
    }
    if !matches!(
        dispatch.status.as_str(),
        "running" | "dispatched" | "queued"
    ) {
        return Ok(());
    }
    let Some(ext) = dispatch.external_id.as_deref() else {
        return Ok(());
    };
    // Queue items are not builds yet — wait until we have a numeric build id.
    if provider == "jenkins" && ext.starts_with("queue:") {
        return Ok(());
    }
    let _ = jobs_svc::enqueue(
        pool,
        FOLLOW_KIND,
        json!({ "dispatch_id": dispatch.id }),
        FOLLOW_MAX_ATTEMPTS,
    )
    .await?;
    Ok(())
}

/// One poll tick for a follow job. Returns `Ok` when terminal (or nothing to do);
/// returns `Err` with a retryable message while the build is still running.
pub async fn handle(pool: &PgPool, payload: &Value) -> Result<Value, String> {
    let FollowPayload { dispatch_id } = serde_json::from_value(payload.clone())
        .map_err(|e| format!("invalid ci_dispatch_follow payload: {e}"))?;

    let row = sqlx::query_as::<_, CiDispatch>(
        r#"SELECT id, project_id, ci_config_id, run_id, status, external_id, external_url,
                  inputs, last_error, created_at, updated_at
           FROM ci_dispatches WHERE id = $1"#,
    )
    .bind(dispatch_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    let Some(dispatch) = row else {
        return Ok(json!({ "skipped": "missing_dispatch" }));
    };
    if matches!(
        dispatch.status.as_str(),
        "succeeded" | "failed" | "cancelled"
    ) {
        return Ok(json!({ "skipped": "already_terminal", "status": dispatch.status }));
    }

    let cfg_row = sqlx::query_as::<_, (String, Value)>(
        r#"SELECT provider, config FROM ci_configs WHERE id = $1"#,
    )
    .bind(dispatch.ci_config_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| e.to_string())?;

    let Some((provider, config)) = cfg_row else {
        return Ok(json!({ "skipped": "missing_config" }));
    };

    let policy = crate::settings::load_ci_url_policy(pool)
        .await
        .map_err(|e| e.to_string())?;

    let external_id = dispatch
        .external_id
        .clone()
        .ok_or_else(|| "dispatch has no external_id".to_string())?;
    let external_url = dispatch.external_url.clone().unwrap_or_default();

    let (status, url) = match provider.as_str() {
        "jenkins" => {
            let build_url = if external_url.is_empty() {
                return Err("jenkins dispatch missing build url".into());
            } else {
                external_url.clone()
            };
            jenkins::poll_build_once(&config, &build_url, &policy)
                .await
                .map_err(|e| e.to_string())?
        }
        "woodpecker" => woodpecker::poll_pipeline_once(&config, &external_id, &policy)
            .await
            .map_err(|e| e.to_string())?,
        other => {
            return Ok(json!({ "skipped": "unsupported_provider", "provider": other }));
        }
    };

    apply_status(pool, &dispatch, &status, Some(url.as_str()))
        .await
        .map_err(|e| e.to_string())?;

    if matches!(status.as_str(), "succeeded" | "failed" | "cancelled") {
        Ok(json!({ "status": status, "external_url": url }))
    } else {
        Err(format!("still_{status}"))
    }
}

async fn apply_status(
    pool: &PgPool,
    dispatch: &CiDispatch,
    status: &str,
    url: Option<&str>,
) -> AppResult<()> {
    let row = sqlx::query_as::<_, CiDispatch>(
        r#"UPDATE ci_dispatches
           SET status = $2,
               external_url = COALESCE($3, external_url),
               updated_at = NOW()
           WHERE id = $1
           RETURNING id, project_id, ci_config_id, run_id, status, external_id, external_url,
                     inputs, last_error, created_at, updated_at"#,
    )
    .bind(dispatch.id)
    .bind(status)
    .bind(url)
    .fetch_one(pool)
    .await?;

    sync_run_status(pool, &row).await?;
    Ok(())
}

async fn sync_run_status(pool: &PgPool, dispatch: &CiDispatch) -> AppResult<()> {
    let Some(run_id) = dispatch.run_id else {
        return Ok(());
    };
    let run_status = match dispatch.status.as_str() {
        "succeeded" => "completed",
        "failed" | "cancelled" => "failed",
        "running" => "running",
        _ => return Ok(()),
    };
    // Do not reopen a run the reporter already closed.
    sqlx::query(
        r#"UPDATE test_runs
           SET status = CASE
                 WHEN status IN ('completed', 'failed', 'cancelled', 'passed') THEN status
                 ELSE $2
               END,
               external_ref = COALESCE($3, external_ref),
               pipeline_url = COALESCE($4, pipeline_url),
               finished_at = CASE
                 WHEN $2 IN ('completed', 'failed')
                      AND status NOT IN ('completed', 'failed', 'cancelled', 'passed')
                 THEN COALESCE(finished_at, NOW())
                 ELSE finished_at
               END,
               updated_at = NOW()
           WHERE id = $1"#,
    )
    .bind(run_id)
    .bind(run_status)
    .bind(&dispatch.external_id)
    .bind(&dispatch.external_url)
    .execute(pool)
    .await?;
    Ok(())
}

/// Helper for unit tests / callers that need the constant.
pub fn job_kind() -> &'static str {
    FOLLOW_KIND
}
