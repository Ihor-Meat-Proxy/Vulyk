//! Background poller: cheap branch-head checks with ETag, re-index on SHA change.

use std::time::Duration;

use sqlx::PgPool;

use crate::crypto::SecretsKey;
use crate::services::automation_repo as repo_svc;
use crate::services::automation_repo_index;
use crate::services::automation_repo_remote::{self as remote_svc, HeadCheck};

/// Wake interval for the poller loop (individual repos use their own `poll_interval_minutes`).
fn tick_secs() -> u64 {
    std::env::var("VULYK_AUTOMATION_REPO_POLL_TICK_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|v| *v >= 15)
        .unwrap_or(60)
}

pub fn spawn_poller(pool: PgPool, secrets: SecretsKey) {
    tokio::spawn(async move {
        tracing::info!("automation-repo poller started");
        loop {
            if let Err(e) = tick(&pool, &secrets).await {
                tracing::warn!(error = %e, "automation-repo poller tick failed");
            }
            tokio::time::sleep(Duration::from_secs(tick_secs())).await;
        }
    });
}

async fn tick(pool: &PgPool, secrets: &SecretsKey) -> Result<(), String> {
    let due = repo_svc::list_repos_due_for_poll(pool)
        .await
        .map_err(|e| e.to_string())?;
    for row in due {
        if let Err(e) = poll_one(pool, secrets, &row).await {
            tracing::warn!(repo_id = %row.id, error = %e, "automation-repo poll failed");
        }
    }
    Ok(())
}

async fn poll_one(
    pool: &PgPool,
    secrets: &SecretsKey,
    row: &crate::models::AutomationRepoRow,
) -> Result<(), String> {
    let token = match (&row.token_ciphertext, &row.token_nonce) {
        (Some(ct), Some(nonce)) => Some(secrets.decrypt(ct, nonce).map_err(|e| e.to_string())?),
        _ => None,
    };
    let head = remote_svc::check_branch_head(
        &row.provider,
        &row.remote_url,
        &row.branch,
        token.as_deref(),
        row.poll_etag.as_deref(),
    )
    .await
    .map_err(|e| e.to_string())?;

    match head {
        HeadCheck::NotModified { etag } => {
            repo_svc::update_poll_meta(pool, row.id, etag.as_deref())
                .await
                .map_err(|e| e.to_string())?;
        }
        HeadCheck::Modified { commit_sha, etag } => {
            repo_svc::update_poll_meta(pool, row.id, etag.as_deref())
                .await
                .map_err(|e| e.to_string())?;
            if row.commit_sha.as_deref() == Some(commit_sha.as_str()) {
                return Ok(());
            }
            if row.index_status == "indexing" {
                return Ok(());
            }
            tracing::info!(
                repo_id = %row.id,
                old = ?row.commit_sha,
                new = %commit_sha,
                "automation-repo poll detected branch move; re-indexing"
            );
            let _ = automation_repo_index::run_reindex(pool, secrets, row, "poll")
                .await
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
