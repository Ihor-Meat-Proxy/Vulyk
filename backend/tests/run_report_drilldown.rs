//! Reports run drill-down API tests.

use std::time::Duration;

use serde_json::Value;

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
async fn run_detail_results_filter_group_coverage_and_export() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }

    let base = spawn_app().await;
    let client = reqwest::Client::new();

    let key = format!(
        "D{}",
        uuid::Uuid::new_v4().simple().to_string()[..6].to_uppercase()
    );
    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&serde_json::json!({
            "key": key,
            "name": "Drilldown Project",
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

    let suite: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/suites"))
        .json(&serde_json::json!({ "name": "Auth", "sort_order": 0 }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let suite_id = suite["id"].as_str().unwrap();

    let case_a: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/cases"))
        .json(&serde_json::json!({
            "key": format!("{key}-1"),
            "title": "Login happy path",
            "suite_id": suite_id,
            "priority": "high",
            "steps": [{ "action": "Open /login", "expected_result": "Form visible" }]
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let case_b: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/cases"))
        .json(&serde_json::json!({
            "key": format!("{key}-2"),
            "title": "Logout",
            "suite_id": suite_id,
            "priority": "medium",
            "steps": [{ "action": "Click logout", "expected_result": "Redirected" }]
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let case_a_id = case_a["case"]["id"].as_str().unwrap();
    let case_b_id = case_b["case"]["id"].as_str().unwrap();

    let req: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/requirements"))
        .json(&serde_json::json!({ "title": "Auth works", "source": "manual" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    client
        .post(format!(
            "{base}/api/v1/projects/{project_id}/requirements:link"
        ))
        .json(&serde_json::json!({
            "case_id": case_a_id,
            "requirement_id": req["id"]
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    // Uncovered requirement
    client
        .post(format!("{base}/api/v1/projects/{project_id}/requirements"))
        .json(&serde_json::json!({ "title": "Orphan req", "source": "manual" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    // Previous run: A passed, B failed
    let prev: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/runs"))
        .json(&serde_json::json!({
            "name": "prev-run",
            "source": "ci",
            "status": "completed",
            "meta": { "branch": "main", "commit": "abc123", "pipeline_url": "https://ci.example/1" },
            "include_all_cases": true,
            "start": true
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let prev_id = prev["run"]["id"].as_str().unwrap();
    client
        .post(format!(
            "{base}/api/v1/projects/{project_id}/runs/{prev_id}/results"
        ))
        .json(&serde_json::json!({
            "results": [
                { "case_id": case_a_id, "status": "passed", "message": "ok" },
                { "case_id": case_b_id, "status": "failed", "message": "boom" }
            ]
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    // Current run: A failed (newly failing), B passed (fixed)
    let curr: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/runs"))
        .json(&serde_json::json!({
            "name": "curr-run",
            "source": "ci",
            "status": "completed",
            "meta": {
                "branch": "feature/x",
                "commit": "def456",
                "pipeline_url": "https://ci.example/2"
            },
            "include_all_cases": true,
            "start": true
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let curr_id = curr["run"]["id"].as_str().unwrap();
    client
        .post(format!(
            "{base}/api/v1/projects/{project_id}/runs/{curr_id}/results"
        ))
        .json(&serde_json::json!({
            "results": [
                {
                    "case_id": case_a_id,
                    "status": "failed",
                    "message": "timeout",
                    "steps": [{
                        "position": 0,
                        "action": "Open /login",
                        "expected_result": "Form visible",
                        "actual_result": "Spinner forever",
                        "status": "failed"
                    }]
                },
                { "case_id": case_b_id, "status": "passed", "message": "fixed" }
            ]
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let detail: Value = client
        .get(format!(
            "{base}/api/v1/projects/{project_id}/reports/runs/{curr_id}"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(detail["name"], "curr-run");
    assert_eq!(detail["source_kind"], "ci");
    assert_eq!(detail["branch"], "feature/x");
    assert_eq!(detail["commit"], "def456");
    assert_eq!(detail["pipeline_url"], "https://ci.example/2");
    assert!(detail["counts"]["total"].as_i64().unwrap() >= 2);

    let results: Value = client
        .get(format!(
            "{base}/api/v1/projects/{project_id}/reports/runs/{curr_id}/results?limit=50&sort=key_asc"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(results["total"], 2);
    let items = results["items"].as_array().unwrap();
    let a = items
        .iter()
        .find(|i| i["case_id"] == case_a_id)
        .expect("case A");
    let b = items
        .iter()
        .find(|i| i["case_id"] == case_b_id)
        .expect("case B");
    assert_eq!(a["comparison"], "newly_failing");
    assert_eq!(b["comparison"], "fixed");
    assert!(a["folder_path"].as_str().unwrap().contains("Auth"));

    let filtered: Value = client
        .get(format!(
            "{base}/api/v1/projects/{project_id}/reports/runs/{curr_id}/results?status=failed"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(filtered["total"], 1);
    assert_eq!(filtered["items"][0]["case_id"], case_a_id);

    let expand: Value = client
        .get(format!(
            "{base}/api/v1/projects/{project_id}/reports/runs/{curr_id}/cases/{case_a_id}"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(!expand["steps"].as_array().unwrap().is_empty());
    assert!(!expand["comment_history"].as_array().unwrap().is_empty());

    let groups: Value = client
        .get(format!(
            "{base}/api/v1/projects/{project_id}/reports/runs/{curr_id}/groups?group_by=status"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(groups["group_by"], "status");
    assert!(groups["groups"].as_array().unwrap().len() >= 2);

    let cov: Value = client
        .get(format!(
            "{base}/api/v1/projects/{project_id}/reports/coverage/detail"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(cov["covered_requirements"], 1);
    assert_eq!(cov["uncovered_requirements"], 1);
    assert!(!cov["covered"][0]["linked_cases"]
        .as_array()
        .unwrap()
        .is_empty());

    let csv = client
        .get(format!(
            "{base}/api/v1/projects/{project_id}/reports/export?format=csv&report=run_results&run_id={curr_id}&status=failed"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .text()
        .await
        .unwrap();
    let a_key = a["case_key"].as_str().unwrap();
    let b_key = b["case_key"].as_str().unwrap();
    assert!(csv.contains("case_key"));
    assert!(csv.contains(a_key), "csv={csv}");
    assert!(
        !csv.contains(b_key),
        "csv should be filtered to failed only: {csv}"
    );

    let pdf = client
        .get(format!(
            "{base}/api/v1/projects/{project_id}/reports/export?format=pdf&report=run_results&run_id={curr_id}"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .bytes()
        .await
        .unwrap();
    assert!(pdf.starts_with(b"%PDF"));
}
