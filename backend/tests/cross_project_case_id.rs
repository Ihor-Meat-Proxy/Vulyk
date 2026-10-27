//! Cross-project case_id must not be accepted into runs / results (review #37).

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

async fn create_project(client: &reqwest::Client, base: &str, name: &str) -> (String, String) {
    let key = uniq("X");
    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&json!({ "key": key, "name": name }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    (
        project["id"].as_str().unwrap().to_string(),
        project["key"].as_str().unwrap().to_string(),
    )
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
async fn create_run_ignores_foreign_case_ids() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }

    let base = spawn_app().await;
    let client = reqwest::Client::new();

    let (proj_a, _) = create_project(&client, &base, "Proj A").await;
    let (proj_b, _) = create_project(&client, &base, "Proj B").await;
    let case_a = create_case(&client, &base, &proj_a).await;
    let case_b = create_case(&client, &base, &proj_b).await;

    let run: Value = client
        .post(format!("{base}/api/v1/projects/{proj_a}/runs"))
        .json(&json!({
            "name": "cross-project",
            "case_ids": [case_a, case_b],
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
    let run_id = run
        .get("id")
        .or_else(|| run.get("run").and_then(|r| r.get("id")))
        .and_then(|v| v.as_str())
        .unwrap();

    let items: Value = client
        .get(format!(
            "{base}/api/v1/projects/{proj_a}/runs/{run_id}/items"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let items = items.as_array().expect("items array");
    assert_eq!(items.len(), 1, "only in-project case should be on the run: {items:?}");
    let got = items[0]
        .pointer("/item/case_id")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("missing item.case_id in {items:?}"));
    assert_eq!(got, case_a);
}

#[tokio::test]
async fn ingest_results_rejects_foreign_case_id() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }

    let base = spawn_app().await;
    let client = reqwest::Client::new();

    let (proj_a, _) = create_project(&client, &base, "Ingest A").await;
    let (proj_b, _) = create_project(&client, &base, "Ingest B").await;
    let case_b = create_case(&client, &base, &proj_b).await;

    let run: Value = client
        .post(format!("{base}/api/v1/projects/{proj_a}/runs"))
        .json(&json!({ "name": "ingest-foreign", "source": "manual" }))
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

    let resp = client
        .post(format!(
            "{base}/api/v1/projects/{proj_a}/runs/{run_id}/results"
        ))
        .json(&json!({
            "results": [{
                "case_id": case_b,
                "status": "passed",
                "source": "manual"
            }]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        400,
        "foreign case_id must be rejected: {}",
        resp.text().await.unwrap_or_default()
    );
}
