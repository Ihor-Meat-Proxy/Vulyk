//! Campaigns / iterations / pipeline ingest smoke tests.

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
async fn campaigns_iterations_pipeline_ingest() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }

    let base = spawn_app().await;
    let client = reqwest::Client::new();

    let key = format!(
        "E{}",
        uuid::Uuid::new_v4().simple().to_string()[..6].to_uppercase()
    );
    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&json!({ "key": key, "name": "Exec Proj" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let project_id = project["id"].as_str().unwrap();

    let case_detail: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/cases"))
        .json(&json!({
            "title": format!("{key}-1 health check"),
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
    let case = &case_detail["case"];
    let case_key = case["key"].as_str().unwrap();
    let case_id = case["id"].as_str().unwrap();

    let campaign: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/campaigns"))
        .json(&json!({ "name": "Integration_Automation", "description": "ci" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let campaign_id = campaign["id"].as_str().unwrap();

    let iteration: Value = client
        .post(format!(
            "{base}/api/v1/projects/{project_id}/campaigns/{campaign_id}/iterations"
        ))
        .json(&json!({
            "name": "Daily - dev",
            "reference": "Daily",
            "mode": "automated",
            "cadence": "daily",
            "case_ids": [case_id],
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let iteration_id = iteration["id"].as_str().unwrap();

    let tree: Value = client
        .get(format!(
            "{base}/api/v1/projects/{project_id}/executions/tree"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(tree
        .as_array()
        .unwrap()
        .iter()
        .any(|n| n["name"] == "Integration_Automation"));

    let junit = format!(
        r#"<?xml version="1.0"?>
<testsuite name="pytest" tests="2" failures="1" skipped="0">
  <testcase classname="tests.api" name="{case_key} health" time="0.4"/>
  <testcase classname="tests.api" name="unmapped boom" time="0.1">
    <failure message="assert false">stack</failure>
  </testcase>
</testsuite>"#
    );

    let ingest: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/ingest"))
        .json(&json!({
            "format": "junit",
            "content": junit,
            "campaign": "Integration_Automation",
            "iteration": "Daily - dev",
            "cadence": "daily",
            "run_name": "CI #1",
            "created_by_name": "WALL-E-rror",
            "environment": { "runner": "Playwright" },
            "artifacts": [{ "kind": "report", "url": "https://example.invalid/report" }],
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(ingest["inserted"].as_i64().unwrap() >= 1);
    assert_eq!(ingest["iteration_id"], iteration_id);
    let run_id = ingest["run_id"].as_str().unwrap();

    let dash: Value = client
        .get(format!(
            "{base}/api/v1/projects/{project_id}/iterations/{iteration_id}/dashboard"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(dash["status_counts"]["total"].as_i64().unwrap() >= 1);
    assert!(!dash["pass_rate_trend"].as_array().unwrap().is_empty());

    let suites: Value = client
        .get(format!(
            "{base}/api/v1/projects/{project_id}/iterations/{iteration_id}/suites?limit=10"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(suites["total"].as_i64().unwrap() >= 1);
    assert_eq!(suites["items"][0]["created_by_name"], "WALL-E-rror");

    let results: Value = client
        .get(format!(
            "{base}/api/v1/projects/{project_id}/suites-runs/{run_id}/results"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(results["total"].as_i64().unwrap() >= 1);

    let history: Value = client
        .get(format!(
            "{base}/api/v1/projects/{project_id}/cases/{case_id}/executions"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(history["total"].as_i64().unwrap() >= 1);
    assert!(!history["sparkline"].as_array().unwrap().is_empty());

    // Playwright JSON path
    let pw = format!(
        r#"{{"suites":[{{"title":"","file":"a.spec.ts","suites":[],"specs":[{{
          "title":"{case_key} landing",
          "ok":true,
          "tests":[{{"status":"expected","projectName":"chromium","results":[{{"status":"passed","duration":42}}]}}]
        }}]}}]}}"#
    );
    let pw_ingest: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/ingest"))
        .json(&json!({
            "format": "playwright_json",
            "content": pw,
            "campaign": "Integration_Automation",
            "iteration": "Daily - dev",
            "created_by_name": "ci-bot",
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(pw_ingest["parsed"].as_i64().unwrap() >= 1);

    // Update schedule
    let updated: Value = client
        .put(format!(
            "{base}/api/v1/projects/{project_id}/iterations/{iteration_id}"
        ))
        .json(&json!({
            "cadence": "on-push",
            "scheduled_start": "2026-10-26T08:00:00Z",
            "scheduled_end": "2026-10-26T18:00:00Z",
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(updated["cadence"], "on-push");
}
