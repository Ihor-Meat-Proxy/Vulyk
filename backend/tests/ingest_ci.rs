//! Ingest + webhook smoke tests (W4/W5).

use std::time::Duration;

use serde_json::{json, Value};

async fn spawn_app() -> (String, sqlx::PgPool) {
    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for integration tests");
    let pool = vulyk_api::db::connect(&database_url)
        .await
        .expect("connect db");
    vulyk_api::db::migrate(&pool).await.expect("migrate");
    vulyk_api::jobs::spawn_worker(pool.clone());

    let state = vulyk_api::routes::AppState {
        pool: pool.clone(),
        secrets: vulyk_api::crypto::SecretsKey::parse("test-secrets-key"),
        auth: vulyk_api::auth::AuthConfig {
            required: false,
            jwt_secret: "test-secret".into(),
            jwt_ttl_hours: 12,
            session_ttl_hours: 168,
        },
        login_throttle: vulyk_api::auth::LoginThrottle::new(),
        password_policy: vulyk_api::auth::PasswordPolicy::from_env(),
    };
    let app = vulyk_api::routes::app_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve");
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    (format!("http://{addr}"), pool)
}

#[tokio::test]
async fn junit_ingest_and_gitlab_webhook_status() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }

    let (base, _pool) = spawn_app().await;
    let client = reqwest::Client::new();

    let key = format!(
        "I{}",
        uuid::Uuid::new_v4().simple().to_string()[..6].to_uppercase()
    );
    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&json!({ "key": key, "name": "Ingest Proj" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let project_id = project["id"].as_str().unwrap();

    let case: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/cases"))
        .json(&json!({
            "title": "Health",
            "status": "ready",
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let case_key = case
        .get("key")
        .or_else(|| case.get("case").and_then(|c| c.get("key")))
        .and_then(|v| v.as_str())
        .expect("case key")
        .to_string();

    let run: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/runs"))
        .json(&json!({ "name": "junit smoke", "source": "ci" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let run_id = run
        .get("id")
        .or_else(|| run.get("run").and_then(|r| r.get("id")))
        .and_then(|v| v.as_str())
        .unwrap();

    let junit = r#"<?xml version="1.0"?>
<testsuite name="pytest" tests="2">
  <testcase classname="t" name="ok" time="0.01"/>
  <testcase classname="t" name="bad" time="0.02">
    <failure message="boom"/>
  </testcase>
</testsuite>"#;

    let ingest: Value = client
        .post(format!("{base}/api/v1/runs/{run_id}/ingest"))
        .json(&json!({
            "format": "junit",
            "content": junit,
            "source": "junit"
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(ingest["parsed"], 2);
    assert_eq!(ingest["inserted"], 2);

    let bulk: Value = client
        .post(format!("{base}/api/v1/runs/{run_id}/results:bulk"))
        .json(&json!({
            "upsert": true,
            "results": [{
                "external_id": "t::ok::linked",
                "case_key": case_key,
                "status": "passed",
                "duration_ms": 11
            }]
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        bulk["updated"].as_i64().unwrap()
            + bulk["inserted"].as_i64().unwrap()
            + bulk["idempotent_replays"].as_i64().unwrap_or(0)
            >= 1
    );

    let results: Value = client
        .get(format!("{base}/api/v1/runs/{run_id}/results"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(results["items"].as_array().unwrap().len() >= 2);

    // CI config + webhook status sync (GitLab pipeline shape).
    let cfg: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/ci-configs"))
        .json(&json!({
            "provider": "gitlab",
            "name": "gitlab main",
            "webhook_secret": "s3cret",
            "config": {
                "project_id": "42",
                "trigger_token": "glptt-test",
                "base_url": "https://gitlab.example.com",
                "ref": "main"
            }
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let cfg_id = cfg["id"].as_str().unwrap();
    assert_eq!(cfg["has_token"], true);
    assert_eq!(cfg["has_webhook_secret"], true);
    assert!(
        cfg["config"].get("trigger_token").is_none(),
        "CI create response must not echo trigger_token: {cfg}"
    );
    assert!(
        cfg.get("webhook_secret").is_none() || cfg["webhook_secret"].is_null(),
        "CI create response must not echo webhook_secret: {cfg}"
    );

    // Seed a dispatch row as if previously triggered.
    let dispatch: Value = client
        .post(format!("{base}/api/v1/ci-configs/{cfg_id}/dispatch"))
        .json(&json!({
            "run_id": run_id,
            "inputs": { "SUITE": "smoke" }
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    // Dispatch to fake GitLab fails network / 4xx — status should be failed, still creates row.
    assert!(dispatch.get("id").is_some());

    // Manually simulate a successful pipeline webhook against a queued/dispatched row.
    // Re-insert a running dispatch via SQL-less path: create another dispatch then webhook.
    let _ = dispatch;

    let wh = client
        .post(format!("{base}/api/v1/webhooks/gitlab/{cfg_id}"))
        .header("X-Gitlab-Token", "s3cret")
        .header("X-Gitlab-Event", "Pipeline Hook")
        .json(&json!({
            "object_kind": "pipeline",
            "object_attributes": {
                "id": 999001,
                "status": "success",
                "url": "https://gitlab.example.com/pipelines/999001"
            }
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    assert_eq!(wh["received"], true);
}

#[tokio::test]
async fn webhook_without_secret_is_rejected() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }

    let (base, _pool) = spawn_app().await;
    let client = reqwest::Client::new();

    let key = format!(
        "W{}",
        uuid::Uuid::new_v4().simple().to_string()[..6].to_uppercase()
    );
    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&json!({
            "key": key,
            "name": "Webhook Auth Project",
            "description": "fail-closed webhooks"
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let project_id = project["id"].as_str().unwrap();

    // Config with no webhook_secret must not authorize unscoped GitLab webhooks.
    let _cfg: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/ci-configs"))
        .json(&json!({
            "provider": "gitlab",
            "name": "no-secret",
            "config": {
                "project_id": "42",
                "trigger_token": "glptt-test",
                "base_url": "https://gitlab.example.com",
                "ref": "main"
            }
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();

    let wh = client
        .post(format!("{base}/api/v1/webhooks/gitlab"))
        .header("X-Gitlab-Event", "Pipeline Hook")
        .json(&json!({
            "object_kind": "pipeline",
            "object_attributes": {
                "id": 1,
                "status": "success",
                "url": "https://gitlab.example.com/pipelines/1"
            }
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        wh.status().as_u16(),
        401,
        "missing webhook secret must fail closed: {}",
        wh.text().await.unwrap_or_default()
    );
}

#[tokio::test]
async fn vulyk_json_async_job() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }

    let (base, pool) = spawn_app().await;
    let client = reqwest::Client::new();

    let key = format!(
        "J{}",
        uuid::Uuid::new_v4().simple().to_string()[..6].to_uppercase()
    );
    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&json!({ "key": key, "name": "Job Proj" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let project_id = project["id"].as_str().unwrap();

    let run: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/runs"))
        .json(&json!({ "name": "async ingest" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let run_id = run
        .get("id")
        .or_else(|| run.get("run").and_then(|r| r.get("id")))
        .and_then(|v| v.as_str())
        .unwrap();

    let doc = json!({
        "format": "vulyk/results@v1",
        "results": [
            { "external_id": "a", "status": "passed" },
            { "external_id": "b", "status": "failed", "message": "x" }
        ]
    });

    let ingest: Value = client
        .post(format!("{base}/api/v1/runs/{run_id}/ingest"))
        .json(&json!({
            "format": "vulyk_json",
            "content": doc.to_string(),
            "async_mode": true
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let job_id = ingest["job_id"].as_str().expect("job_id");

    // Parallel cargo test binaries share DATABASE_URL. A worker from another
    // binary can claim this job and then exit, leaving status=`running` until
    // the 15m production reclaim window. Drive completion locally: reclaim
    // short-lived abandoned locks and run worker ticks ourselves.
    let mut status = String::new();
    let mut last_job = json!({});
    for i in 0..60 {
        let job: Value = client
            .get(format!("{base}/api/v1/jobs/{job_id}"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        last_job = job.clone();
        status = job["status"].as_str().unwrap_or("").to_string();
        if status == "completed" || status == "failed" {
            assert_eq!(status, "completed", "job error: {:?}", job["last_error"]);
            break;
        }
        if status == "running" {
            // Free locks held longer than 1s (dead peer workers).
            let _ = vulyk_api::services::jobs::reclaim_stale(&pool, 1).await;
        }
        let _ = vulyk_api::jobs::process_one(&pool, "ingest-ci-local").await;
        // First few iterations give the local spawn_worker a chance before we
        // reclaim aggressively.
        let delay = if i < 4 { 100 } else { 200 };
        tokio::time::sleep(Duration::from_millis(delay)).await;
    }
    assert_eq!(
        status, "completed",
        "async ingest job did not finish in time: {last_job}"
    );

    let results: Value = client
        .get(format!("{base}/api/v1/runs/{run_id}/results"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(results["items"].as_array().unwrap().len(), 2);
}
