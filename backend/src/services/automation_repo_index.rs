//! Shared re-index orchestration for manual, webhook, and poll triggers.

use sqlx::PgPool;

use crate::crypto::SecretsKey;
use crate::error::{AppError, AppResult};
use crate::models::{AutomationRepoIndexResult, AutomationRepoRow};
use crate::services::automation_repo as repo_svc;
use crate::services::automation_repo_remote as remote_svc;

pub fn index_result_from_err(e: &AppError) -> AutomationRepoIndexResult {
    let (error_code, rate_limit_reset_at) = match e {
        AppError::GitHostRateLimit { reset_at, .. } => {
            (Some("git_host_rate_limit".into()), *reset_at)
        }
        _ => (None, None),
    };
    AutomationRepoIndexResult {
        status: "error".into(),
        commit_sha: None,
        file_count: 0,
        chunk_count: 0,
        error: Some(e.to_string()),
        error_code,
        rate_limit_reset_at,
        last_index_trigger: None,
    }
}

fn decrypt_token(secrets: &SecretsKey, row: &AutomationRepoRow) -> AppResult<Option<String>> {
    match (&row.token_ciphertext, &row.token_nonce) {
        (Some(ct), Some(nonce)) => Ok(Some(secrets.decrypt(ct, nonce)?)),
        _ => Ok(None),
    }
}

/// Run a re-index with the given trigger. Skips if an index is already running.
pub async fn run_reindex(
    pool: &PgPool,
    secrets: &SecretsKey,
    row: &AutomationRepoRow,
    trigger: &str,
) -> AppResult<AutomationRepoIndexResult> {
    if !repo_svc::try_mark_indexing(pool, row.id).await? {
        return Ok(AutomationRepoIndexResult {
            status: "indexing".into(),
            commit_sha: row.commit_sha.clone(),
            file_count: row.file_count,
            chunk_count: row.chunk_count,
            error: Some("An index is already running for this repository".into()),
            error_code: Some("already_indexing".into()),
            rate_limit_reset_at: None,
            last_index_trigger: row.last_index_trigger.clone(),
        });
    }
    let token = decrypt_token(secrets, row)?;
    match sync_and_index(pool, row, token.as_deref(), trigger).await {
        Ok((sha, files, chunks)) => Ok(AutomationRepoIndexResult {
            status: "ready".into(),
            commit_sha: Some(sha),
            file_count: files,
            chunk_count: chunks,
            error: None,
            error_code: None,
            rate_limit_reset_at: None,
            last_index_trigger: Some(trigger.to_string()),
        }),
        Err(e) => {
            let msg = e.to_string();
            let _ = repo_svc::mark_index_error(pool, row.id, &msg).await;
            let mut result = index_result_from_err(&e);
            result.last_index_trigger = Some(trigger.to_string());
            Ok(result)
        }
    }
}

pub async fn sync_and_index(
    pool: &PgPool,
    row: &AutomationRepoRow,
    token: Option<&str>,
    trigger: &str,
) -> AppResult<(String, i32, i32)> {
    let provider = row.provider.to_lowercase();
    if matches!(provider.as_str(), "github" | "gitlab") {
        let known = repo_svc::load_content_hashes(pool, row.id).await?;
        let (commit_sha, changed, tree_paths) = remote_svc::index_from_archive(
            &provider,
            &row.remote_url,
            &row.branch,
            &row.subpath,
            token,
            &known,
        )
        .await?;
        let (fc, cc) = repo_svc::persist_index_incremental(
            pool,
            row.id,
            &tree_paths,
            &changed,
            &commit_sha,
            trigger,
        )
        .await?;
        return Ok((commit_sha, fc, cc));
    }

    if !repo_svc::git_available() {
        return Err(AppError::BadRequest(
            "git is not installed in this environment. The generic provider needs the git binary \
             in the API image; use the github or gitlab provider for REST indexing without git."
                .into(),
        ));
    }
    let repo_id = row.id;
    let remote_url = row.remote_url.clone();
    let branch = row.branch.clone();
    let subpath = row.subpath.clone();
    let token_owned = token.map(str::to_string);
    let sha = tokio::task::spawn_blocking(move || {
        repo_svc::sync_repo(repo_id, &remote_url, &branch, token_owned.as_deref())
    })
    .await
    .map_err(|e| AppError::Internal(format!("git sync task: {e}")))??;
    let files = tokio::task::spawn_blocking(move || repo_svc::index_workdir(repo_id, &subpath))
        .await
        .map_err(|e| AppError::Internal(format!("git index task: {e}")))??;
    let (fc, cc) = repo_svc::persist_index(pool, row.id, &files, &sha, trigger).await?;
    Ok((sha, fc, cc))
}
