//! Ingest can match cases by automation_id without the case key in the title.

use std::time::Duration;

use serde_json::{json, Value};

async fn spawn_app() -> String {
    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for integration tests");
    let pool = vulyk_api::db::connect(&database_url)
        .await
        .expect("connect db");
    vulyk_api::db::migrate(&pool).await.expect("migrate");

    let state = vulyk_api::routes::AppState {
        pool,
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
    format!("http://{addr}")
}

#[tokio::test]
async fn bulk_results_match_automation_id_without_key_in_title() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }

    let base = spawn_app().await;
    let client = reqwest::Client::new();

    let key = format!(
        "A{}",
        uuid::Uuid::new_v4().simple().to_string()[..6].to_uppercase()
    );
    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&json!({ "key": key, "name": "Automation Id Proj" }))
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
            "title": "Login without key in title",
            "status": "ready",
            "automation_id": "tests.login::valid_password"
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let case_id = case["case"]["id"].as_str().unwrap().to_string();
    assert_eq!(
        case["case"]["automation_id"].as_str(),
        Some("tests.login::valid_password")
    );

    let run: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/runs"))
        .json(&json!({ "name": "auto-id", "source": "ci" }))
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

    let bulk: Value = client
        .post(format!("{base}/api/v1/runs/{run_id}/results:bulk"))
        .json(&json!({
            "results": [{
                "external_id": "tests.login::valid_password",
                "title": "valid_password",
                "automation_id": "tests.login::valid_password",
                "status": "passed",
                "duration_ms": 12
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

    assert_eq!(
        bulk["inserted"], 1,
        "automation_id must match without case key in title"
    );
    assert!(
        bulk["unmatched_case_keys"]
            .as_array()
            .map(|a| a.is_empty())
            .unwrap_or(false),
        "expected no unmatched keys, got {:?}",
        bulk["unmatched_case_keys"]
    );

    let results: Value = client
        .get(format!("{base}/api/v1/runs/{run_id}/results"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let rows = results["items"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["case_id"].as_str().unwrap(), case_id);
}

#[tokio::test]
async fn junit_ingest_matches_automation_id_property() {
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
        .json(&json!({ "key": key, "name": "JUnit Auto Id" }))
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
            "title": "Anonymous junit",
            "status": "ready",
            "automation_id": "pkg::anon_test"
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let case_id = case["case"]["id"].as_str().unwrap().to_string();

    let run: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/runs"))
        .json(&json!({ "name": "junit-auto", "source": "ci" }))
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

    let xml = r#"<?xml version="1.0"?>
<testsuite name="s" tests="1">
  <testcase classname="pkg" name="anon_test" time="0.2">
    <properties>
      <property name="vulyk_automation_id" value="pkg::anon_test"/>
    </properties>
  </testcase>
</testsuite>"#;

    let ingest: Value = client
        .post(format!("{base}/api/v1/runs/{run_id}/ingest"))
        .json(&json!({
            "format": "junit",
            "content": xml
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(
        ingest["inserted"], 1,
        "junit automation_id property must match"
    );
    assert!(
        ingest["unmatched_case_keys"]
            .as_array()
            .map(|a| a.is_empty())
            .unwrap_or(true),
        "unexpected unmatched: {:?}",
        ingest["unmatched_case_keys"]
    );

    let results: Value = client
        .get(format!("{base}/api/v1/runs/{run_id}/results"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let rows = results["items"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["case_id"].as_str().unwrap(), case_id);
}
