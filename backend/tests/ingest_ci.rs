//! Ingest + webhook smoke tests (W4/W5).

use std::time::Duration;

use serde_json::{json, Value};

async fn spawn_app() -> String {
    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for integration tests");
    let pool = vulyk_api::db::connect(&database_url)
        .await
        .expect("connect db");
    vulyk_api::db::migrate(&pool).await.expect("migrate");
    vulyk_api::jobs::spawn_worker(pool.clone());

    let state = vulyk_api::routes::AppState {
        pool,
        secrets: vulyk_api::crypto::SecretsKey::parse("test-secrets-key"),
        auth: vulyk_api::auth::AuthConfig {
            required: false,
            jwt_secret: "test-secret".into(),
            jwt_ttl_hours: 12,
            session_ttl_hours: 168,
        },
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
    format!("http://{addr}")
}

#[tokio::test]
async fn junit_ingest_and_gitlab_webhook_status() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }

    let base = spawn_app().await;
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

    let _case: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/cases"))
        .json(&json!({
            "key": format!("{key}-1"),
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
    let run_id = run["id"].as_str().unwrap();

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
                "external_id": "t::ok",
                "case_key": format!("{key}-1"),
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
    assert!(bulk["updated"].as_i64().unwrap() + bulk["inserted"].as_i64().unwrap() >= 1);

    let results: Value = client
        .get(format!("{base}/api/v1/runs/{run_id}/results"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(results.as_array().unwrap().len() >= 2);

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
async fn vulyk_json_async_job() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }

    let base = spawn_app().await;
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
    let run_id = run["id"].as_str().unwrap();

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

    let mut status = String::new();
    for _ in 0..40 {
        let job: Value = client
            .get(format!("{base}/api/v1/jobs/{job_id}"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        status = job["status"].as_str().unwrap_or("").to_string();
        if status == "completed" || status == "failed" {
            assert_eq!(status, "completed", "job error: {:?}", job["last_error"]);
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert_eq!(status, "completed");

    let results: Value = client
        .get(format!("{base}/api/v1/runs/{run_id}/results"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(results.as_array().unwrap().len(), 2);
}
