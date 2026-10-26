//! Human-readable case key lookup: GET /api/v1/cases/by-key/{key}

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

fn uniq(prefix: &str) -> String {
    format!(
        "{prefix}{}",
        uuid::Uuid::new_v4().simple().to_string()[..6].to_uppercase()
    )
}

#[tokio::test]
async fn get_case_by_key_is_case_insensitive() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let base = spawn_app().await;
    let client = reqwest::Client::new();
    let project_key = uniq("BK");

    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&serde_json::json!({
            "key": project_key,
            "name": "By Key Lookup"
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
    let prefix = project["case_key_prefix"].as_str().unwrap();

    let created: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/cases"))
        .json(&serde_json::json!({ "title": "Lookup me" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let key = created["case"]["key"].as_str().unwrap().to_string();
    assert_eq!(key, format!("{prefix}-1"));
    let case_id = created["case"]["id"].as_str().unwrap();

    let lower = key.to_ascii_lowercase();
    let detail: Value = client
        .get(format!("{base}/api/v1/cases/by-key/{lower}"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(detail["case"]["id"], case_id);
    assert_eq!(detail["case"]["key"], key);
    assert_eq!(detail["case"]["title"], "Lookup me");
    assert_eq!(detail["case"]["project_id"], project_id);

    let upper_detail: Value = client
        .get(format!("{base}/api/v1/cases/by-key/{key}"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(upper_detail["case"]["id"], case_id);
}

#[tokio::test]
async fn deleted_key_returns_gone() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let base = spawn_app().await;
    let client = reqwest::Client::new();
    let project_key = uniq("DG");

    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&serde_json::json!({ "key": project_key, "name": "Deleted Key Gone" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let project_id = project["id"].as_str().unwrap();

    let created: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/cases"))
        .json(&serde_json::json!({ "title": "Doomed" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let key = created["case"]["key"].as_str().unwrap().to_string();
    let case_id = created["case"]["id"].as_str().unwrap();

    let del = client
        .delete(format!(
            "{base}/api/v1/projects/{project_id}/cases/{case_id}"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(del.status(), 204);

    let resp = client
        .get(format!("{base}/api/v1/cases/by-key/{key}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 410);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["error"]["code"], "case_deleted");
}

#[tokio::test]
async fn unknown_key_returns_not_found() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let base = spawn_app().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{base}/api/v1/cases/by-key/ZZZZ-99999"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["error"]["code"], "not_found");
}
