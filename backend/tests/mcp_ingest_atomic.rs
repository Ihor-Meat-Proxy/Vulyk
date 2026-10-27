//! MCP-friendly bulk `POST .../results` must not commit a partial batch (review #40).

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

fn uniq(prefix: &str) -> String {
    format!(
        "{prefix}{}",
        uuid::Uuid::new_v4().simple().to_string()[..6].to_uppercase()
    )
}

async fn create_project(client: &reqwest::Client, base: &str) -> String {
    let key = uniq("M");
    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&json!({ "key": key, "name": "MCP ingest" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    project["id"].as_str().unwrap().to_string()
}

async fn create_case(client: &reqwest::Client, base: &str, project_id: &str) -> String {
    let case: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/cases"))
        .json(&json!({ "title": "Case", "status": "ready" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    case["case"]["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn bulk_results_rolls_back_when_later_row_invalid() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }

    let base = spawn_app().await;
    let client = reqwest::Client::new();
    let project_id = create_project(&client, &base).await;
    let case_id = create_case(&client, &base, &project_id).await;

    let run: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/runs"))
        .json(&json!({ "name": "partial-batch", "source": "mcp" }))
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
        .unwrap()
        .to_string();

    let resp = client
        .post(format!(
            "{base}/api/v1/projects/{project_id}/runs/{run_id}/results"
        ))
        .json(&json!({
            "results": [
                {
                    "case_id": case_id,
                    "status": "passed",
                    "source": "mcp",
                    "external_id": "ok-1"
                },
                {
                    "case_id": case_id,
                    "status": "not-a-real-status",
                    "source": "mcp",
                    "external_id": "bad-2"
                }
            ]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        400,
        "invalid status must 400: {}",
        resp.text().await.unwrap_or_default()
    );

    let listed: Value = client
        .get(format!(
            "{base}/api/v1/projects/{project_id}/runs/{run_id}/results"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let rows = listed.as_array().expect("results array");
    assert!(
        rows.is_empty(),
        "partial batch must roll back; got {listed}"
    );
}
