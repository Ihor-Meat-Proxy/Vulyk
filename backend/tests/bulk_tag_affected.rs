//! Bulk tag ops must not count foreign/missing case_ids as affected (review #39).

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

async fn create_project(client: &reqwest::Client, base: &str, name: &str) -> String {
    let key = uniq("T");
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
async fn bulk_add_tags_ignores_foreign_case_ids_in_affected() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }

    let base = spawn_app().await;
    let client = reqwest::Client::new();

    let proj_a = create_project(&client, &base, "Tag A").await;
    let proj_b = create_project(&client, &base, "Tag B").await;
    let case_a = create_case(&client, &base, &proj_a).await;
    let case_b = create_case(&client, &base, &proj_b).await;
    let missing = uuid::Uuid::new_v4().to_string();

    let body: Value = client
        .post(format!("{base}/api/v1/projects/{proj_a}/cases/bulk"))
        .json(&json!({
            "action": "add_tags",
            "case_ids": [case_a, case_b, missing],
            "tag_names": ["smoke"]
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
        body["affected"].as_i64().unwrap(),
        1,
        "only in-project case should count: {body}"
    );
    assert_eq!(body["action"], "add_tags");

    // Foreign case must not have received project A's tag.
    let detail_b: Value = client
        .get(format!("{base}/api/v1/projects/{proj_b}/cases/{case_b}"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let tags_b = detail_b["tags"].as_array().cloned().unwrap_or_default();
    assert!(
        tags_b.iter().all(|t| t["name"] != "smoke"),
        "foreign case must not gain project A tags: {detail_b}"
    );

    let detail_a: Value = client
        .get(format!("{base}/api/v1/projects/{proj_a}/cases/{case_a}"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let tags_a = detail_a["tags"].as_array().cloned().unwrap_or_default();
    assert!(
        tags_a.iter().any(|t| t["name"] == "smoke"),
        "in-project case should be tagged: {detail_a}"
    );
}
