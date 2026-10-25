//! Postgres-backed job queue helpers (`SKIP LOCKED`).

use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppResult;
use crate::models::Job;

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

pub async fn claim_next(pool: &PgPool, worker_id: &str) -> AppResult<Option<Job>> {
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
