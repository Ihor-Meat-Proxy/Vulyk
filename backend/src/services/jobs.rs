//! Postgres-backed job queue helpers (`SKIP LOCKED`).

use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppResult;
use crate::models::Job;

/// Running jobs whose lock is older than this are treated as abandoned
/// (worker crash / deploy) and returned to `pending` for another claim.
pub const STALE_RUNNING_SECS: i64 = 15 * 60;

pub async fn enqueue(
    pool: &PgPool,
    kind: &str,
    payload: Value,
    max_attempts: i32,
) -> AppResult<Job> {
    let job = sqlx::query_as::<_, Job>(
        r#"INSERT INTO jobs (kind, payload, max_attempts)
           VALUES ($1, $2, $3)
           RETURNING id, kind, payload, status, attempts, max_attempts, last_error,
                     run_at, locked_at, locked_by, result, created_at, updated_at, completed_at"#,
    )
    .bind(kind)
    .bind(payload)
    .bind(max_attempts)
    .fetch_one(pool)
    .await?;
    Ok(job)
}

pub async fn get_job(pool: &PgPool, id: Uuid) -> AppResult<Option<Job>> {
    let job = sqlx::query_as::<_, Job>(
        r#"SELECT id, kind, payload, status, attempts, max_attempts, last_error,
                  run_at, locked_at, locked_by, result, created_at, updated_at, completed_at
           FROM jobs WHERE id = $1"#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(job)
}

/// Requeue jobs stuck in `running` after a worker crash / hard kill.
///
/// Jobs that have already exhausted `max_attempts` are marked `failed`
/// instead of looping forever.
pub async fn reclaim_stale(pool: &PgPool, stale_after_secs: i64) -> AppResult<u64> {
    let result = sqlx::query(
        r#"
        UPDATE jobs
        SET status = CASE
                WHEN attempts >= max_attempts THEN 'failed'
                ELSE 'pending'
              END,
            last_error = COALESCE(
                last_error,
                'reclaimed: worker lock expired'
            ),
            run_at = CASE
                WHEN attempts >= max_attempts THEN run_at
                ELSE NOW()
              END,
            locked_at = NULL,
            locked_by = NULL,
            completed_at = CASE
                WHEN attempts >= max_attempts THEN COALESCE(completed_at, NOW())
                ELSE NULL
              END,
            updated_at = NOW()
        WHERE status = 'running'
          AND locked_at IS NOT NULL
          AND locked_at < NOW() - ($1::text || ' seconds')::interval
        "#,
    )
    .bind(stale_after_secs)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

pub async fn claim_next(pool: &PgPool, worker_id: &str) -> AppResult<Option<Job>> {
    // Best-effort: free abandoned locks before selecting the next pending job.
    let _ = reclaim_stale(pool, STALE_RUNNING_SECS).await?;

    let job = sqlx::query_as::<_, Job>(
        r#"
        UPDATE jobs
        SET status = 'running',
            attempts = attempts + 1,
            locked_at = NOW(),
            locked_by = $1,
            updated_at = NOW()
        WHERE id = (
            SELECT id FROM jobs
            WHERE status = 'pending' AND run_at <= NOW()
            ORDER BY run_at
            FOR UPDATE SKIP LOCKED
            LIMIT 1
        )
        RETURNING id, kind, payload, status, attempts, max_attempts, last_error,
                  run_at, locked_at, locked_by, result, created_at, updated_at, completed_at
        "#,
    )
    .bind(worker_id)
    .fetch_optional(pool)
    .await?;
    Ok(job)
}

pub async fn complete(pool: &PgPool, id: Uuid, result: Value) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE jobs
           SET status = 'completed', result = $2, completed_at = NOW(),
               locked_at = NULL, locked_by = NULL, updated_at = NOW()
           WHERE id = $1"#,
    )
    .bind(id)
    .bind(result)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn fail(pool: &PgPool, id: Uuid, error: &str, retry: bool) -> AppResult<()> {
    if retry {
        sqlx::query(
            r#"UPDATE jobs
               SET status = CASE WHEN attempts >= max_attempts THEN 'failed' ELSE 'pending' END,
                   last_error = $2,
                   run_at = CASE WHEN attempts >= max_attempts THEN run_at ELSE NOW() + (attempts || ' seconds')::interval END,
                   locked_at = NULL,
                   locked_by = NULL,
                   completed_at = CASE WHEN attempts >= max_attempts THEN NOW() ELSE NULL END,
                   updated_at = NOW()
               WHERE id = $1"#,
        )
        .bind(id)
        .bind(error)
        .execute(pool)
        .await?;
    } else {
        sqlx::query(
            r#"UPDATE jobs
               SET status = 'failed', last_error = $2, completed_at = NOW(),
                   locked_at = NULL, locked_by = NULL, updated_at = NOW()
               WHERE id = $1"#,
        )
        .bind(id)
        .bind(error)
        .execute(pool)
        .await?;
    }
    Ok(())
}
