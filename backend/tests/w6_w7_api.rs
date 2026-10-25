//! W6/W7 API smoke tests — fixtures only, no live ALM credentials.

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
async fn requirements_link_import_preview_and_fixture_alm() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }

    let base = spawn_app().await;
    let client = reqwest::Client::new();

    let key = format!(
        "W{}",
        uuid::Uuid::new_v4().simple().to_string()[..6].to_uppercase()
    );
    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&serde_json::json!({
            "key": key,
            "name": "W6W7 Project",
        }))
        .send()
        .await
        .expect("project")
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let project_id = project["id"].as_str().unwrap();

    let req: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/requirements"))
        .json(&serde_json::json!({
            "title": "Must support SSO",
            "source": "manual"
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();

    let case: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/cases"))
        .json(&serde_json::json!({
            "key": format!("{key}-1"),
            "title": "SSO login",
            "steps": [{ "action": "Login via IdP", "expected_result": "Session created" }]
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();

    let link: Value = client
        .post(format!(
            "{base}/api/v1/projects/{project_id}/requirements:link"
        ))
        .json(&serde_json::json!({
            "case_id": case["case"]["id"],
            "requirement_id": req["id"]
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(link["requirement_id"], req["id"]);

    let importers: Value = client
        .get(format!("{base}/api/v1/importers"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(importers.as_array().unwrap().len() >= 9);

    let csv = include_str!("fixtures/importers/generic.csv");
    let preview: Value = client
        .post(format!(
            "{base}/api/v1/projects/{project_id}/imports:preview"
        ))
        .json(&serde_json::json!({
            "importer_id": "csv",
            "format": "csv",
            "content": csv,
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(preview["total"], 2);

    let report: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/imports"))
        .json(&serde_json::json!({
            "importer_id": "csv",
            "format": "csv",
            "content": csv,
            "dry_run": false
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(report["created_cases"], 2);

    let integration: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/integrations"))
        .json(&serde_json::json!({
            "kind": "openproject",
            "name": "OP fixture",
            "base_url": "https://openproject.example.test",
            "config": {
                "project_id": "1",
                "fixture_requirements": [{
                    "external_id": "42",
                    "external_key": "WP-42",
                    "title": "Users must authenticate",
                    "description": "",
                    "url": "https://openproject.example.test/work_packages/42",
                    "raw": {}
                }],
                "fixture_defect": {
                    "id": 77,
                    "subject": "Bug from failure"
                }
            },
            "secrets": { "api_token": "fixture-not-real" }
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(integration["has_secrets"], true);
    assert!(integration.get("secrets").is_none());

    let imported: Value = client
        .post(format!(
            "{base}/api/v1/projects/{project_id}/integrations/{}/requirements:import",
            integration["id"].as_str().unwrap()
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(imported.as_array().unwrap().len(), 1);
    assert_eq!(imported[0]["external_key"], "WP-42");

    let defect: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/defects"))
        .json(&serde_json::json!({
            "integration_id": integration["id"],
            "case_id": case["case"]["id"],
            "title": "SSO failure defect",
            "description": "Repro from failed run",
            "steps_to_reproduce": "1. Open IdP\n2. Observe error"
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(defect["external_key"], "WP-77");
}
