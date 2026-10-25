//! HTTP smoke tests against a real Postgres (CI service / local docker).

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
async fn health_and_project_case_crud() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }

    let base = spawn_app().await;
    let client = reqwest::Client::new();

    let health: Value = client
        .get(format!("{base}/health"))
        .send()
        .await
        .expect("health")
        .json()
        .await
        .expect("json");
    assert_eq!(health["status"], "ok");

    let key = format!(
        "T{}",
        uuid::Uuid::new_v4().simple().to_string()[..6].to_uppercase()
    );
    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&serde_json::json!({
            "key": key,
            "name": "CI Project",
            "description": "from smoke test"
        }))
        .send()
        .await
        .expect("create project")
        .error_for_status()
        .expect("project status")
        .json()
        .await
        .expect("project json");
    let project_id = project["id"].as_str().expect("id");

    let suite: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/suites"))
        .json(&serde_json::json!({ "name": "Regression" }))
        .send()
        .await
        .expect("suite")
        .error_for_status()
        .expect("suite status")
        .json()
        .await
        .expect("suite json");
    let suite_id = suite["id"].as_str().expect("suite id");

    let detail: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/cases"))
        .json(&serde_json::json!({
            "key": format!("{key}-1"),
            "title": "Login works",
            "suite_id": suite_id,
            "steps": [{ "action": "Open app", "expected_result": "Dashboard" }]
        }))
        .send()
        .await
        .expect("case")
        .error_for_status()
        .expect("case status")
        .json()
        .await
        .expect("case json");
    assert_eq!(detail["case"]["title"], "Login works");
    assert_eq!(detail["steps"].as_array().expect("steps").len(), 1);

    let cases: Value = client
        .get(format!("{base}/api/v1/projects/{project_id}/cases"))
        .send()
        .await
        .expect("list")
        .json()
        .await
        .expect("list json");
    assert_eq!(cases.as_array().expect("arr").len(), 1);
}
