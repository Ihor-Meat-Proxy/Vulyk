//! Stale `running` jobs must be reclaimed so ingest does not stall after crashes.

use serde_json::json;

#[tokio::test]
async fn reclaim_stale_running_job_then_claim() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    };

    let pool = vulyk_api::db::connect(&database_url)
        .await
        .expect("connect db");
    vulyk_api::db::migrate(&pool).await.expect("migrate");

    let job = vulyk_api::services::jobs::enqueue(&pool, "ingest", json!({}), 5)
        .await
        .expect("enqueue");

    // Simulate a worker that claimed the job and then died.
    sqlx::query(
        r#"UPDATE jobs
           SET status = 'running',
               attempts = 1,
               locked_at = NOW() - INTERVAL '20 minutes',
               locked_by = 'dead-worker',
               updated_at = NOW() - INTERVAL '20 minutes'
           WHERE id = $1"#,
    )
    .bind(job.id)
    .execute(&pool)
    .await
    .expect("mark stale");

    // Parallel cargo tests share DATABASE_URL; another claim_next may have
    // already reclaimed this row. Assert the planted job ends pending, not
    // that this particular call's rows_affected is non-zero.
    let _ = vulyk_api::services::jobs::reclaim_stale(&pool, 15 * 60)
        .await
        .expect("reclaim");

    let after = vulyk_api::services::jobs::get_job(&pool, job.id)
        .await
        .expect("get")
        .expect("job exists");
    assert_eq!(
        after.status, "pending",
        "stale running job must be requeued (got status={})",
        after.status
    );
    assert!(after.locked_at.is_none());
    assert!(after.locked_by.is_none());

    // claim_next runs reclaim first; with a fresh lock age of 0 it should not
    // re-pending our already-pending job. Bring it to the front and claim it.
    sqlx::query("UPDATE jobs SET run_at = NOW() - INTERVAL '365 days' WHERE id = $1")
        .bind(job.id)
        .execute(&pool)
        .await
        .expect("prioritize");

    let mut claimed_ours = false;
    for _ in 0..32 {
        let Some(claimed) = vulyk_api::services::jobs::claim_next(&pool, "test-worker")
            .await
            .expect("claim")
        else {
            break;
        };
        if claimed.id == job.id {
            claimed_ours = true;
            break;
        }
        // Finish unrelated jobs so a shared CI database cannot starve this assertion.
        vulyk_api::services::jobs::complete(&pool, claimed.id, json!({"ok": true}))
            .await
            .expect("complete unrelated");
    }
    assert!(
        claimed_ours,
        "reclaimed job must be claimable via claim_next"
    );

    let _ = sqlx::query("DELETE FROM jobs WHERE id = $1")
        .bind(job.id)
        .execute(&pool)
        .await;
}

#[tokio::test]
async fn reclaim_exhausted_attempts_marks_failed() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    };

    let pool = vulyk_api::db::connect(&database_url)
        .await
        .expect("connect db");
    vulyk_api::db::migrate(&pool).await.expect("migrate");

    let job = vulyk_api::services::jobs::enqueue(&pool, "ingest", json!({}), 1)
        .await
        .expect("enqueue");

    sqlx::query(
        r#"UPDATE jobs
           SET status = 'running',
               attempts = 1,
               max_attempts = 1,
               locked_at = NOW() - INTERVAL '30 minutes',
               locked_by = 'dead-worker'
           WHERE id = $1"#,
    )
    .bind(job.id)
    .execute(&pool)
    .await
    .expect("mark stale exhausted");

    let _ = vulyk_api::services::jobs::reclaim_stale(&pool, 15 * 60)
        .await
        .expect("reclaim");

    let after = vulyk_api::services::jobs::get_job(&pool, job.id)
        .await
        .expect("get")
        .expect("job exists");
    assert_eq!(after.status, "failed");
    assert!(after.completed_at.is_some());

    let _ = sqlx::query("DELETE FROM jobs WHERE id = $1")
        .bind(job.id)
        .execute(&pool)
        .await;
}
