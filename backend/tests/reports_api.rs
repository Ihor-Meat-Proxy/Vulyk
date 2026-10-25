//! W8 reports API smoke tests.

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
async fn dashboard_summary_trends_coverage_flaky_and_export() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }

    let base = spawn_app().await;
    let client = reqwest::Client::new();

    let key = format!(
        "R{}",
        uuid::Uuid::new_v4().simple().to_string()[..6].to_uppercase()
    );
    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&serde_json::json!({
            "key": key,
            "name": "Reports Project",
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

    let case_a: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/cases"))
        .json(&serde_json::json!({
            "key": format!("{key}-1"),
            "title": "Flaky candidate",
            "steps": [{ "action": "Click", "expected_result": "Ok" }]
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
            "title": "Stable case",
            "steps": [{ "action": "Click", "expected_result": "Ok" }]
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
        .json(&serde_json::json!({ "title": "Login works", "source": "manual" }))
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

    // Two runs with alternating outcomes for case A → flaky.
    for (name, a_status, b_status) in [
        ("run-1", "passed", "passed"),
        ("run-2", "failed", "passed"),
        ("run-3", "passed", "passed"),
    ] {
        let run: Value = client
            .post(format!("{base}/api/v1/projects/{project_id}/runs"))
            .json(&serde_json::json!({
                "name": name,
                "source": "api",
                "status": "completed"
            }))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        // W3 create_run returns TestRunDetail { run, ... }
        let run_id = run
            .get("run")
            .and_then(|r| r.get("id"))
            .or_else(|| run.get("id"))
            .and_then(|v| v.as_str())
            .expect("run id");

        client
            .post(format!(
                "{base}/api/v1/projects/{project_id}/runs/{run_id}/results"
            ))
            .json(&serde_json::json!({
                "results": [
                    { "case_id": case_a_id, "status": a_status, "message": "" },
                    { "case_id": case_b_id, "status": b_status, "message": "" }
                ]
            }))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
    }

    let dash: Value = client
        .get(format!(
            "{base}/api/v1/projects/{project_id}/reports/dashboard"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(dash["total_cases"], 2);
    assert_eq!(dash["total_runs"], 3);
    assert!(!dash["recent_run_summaries"].as_array().unwrap().is_empty());

    let trends: Value = client
        .get(format!(
            "{base}/api/v1/projects/{project_id}/reports/trends?granularity=run"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(trends["points"].as_array().unwrap().len(), 3);

    let coverage: Value = client
        .get(format!(
            "{base}/api/v1/projects/{project_id}/reports/coverage"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(coverage["total_requirements"], 1);
    assert_eq!(coverage["covered_requirements"], 1);

    let flaky: Value = client
        .get(format!("{base}/api/v1/projects/{project_id}/reports/flaky"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let flaky_cases = flaky["cases"].as_array().unwrap();
    assert!(
        flaky_cases.iter().any(|c| c["case_id"] == case_a_id),
        "expected flaky case A, got {flaky:?}"
    );

    let csv = client
        .get(format!(
            "{base}/api/v1/projects/{project_id}/reports/export?format=csv&report=dashboard"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(csv.contains("total_cases"));

    let pdf = client
        .get(format!(
            "{base}/api/v1/projects/{project_id}/reports/export?format=pdf"
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
