//! Review #8: every case version bump must record a case_versions row (no silent skip).

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
async fn create_and_updates_each_leave_a_version_snapshot() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let base = spawn_app().await;
    let client = reqwest::Client::new();
    let project_key = uniq("SV");

    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&serde_json::json!({
            "key": project_key,
            "name": "Snapshot Versions"
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

    let created: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/cases"))
        .json(&serde_json::json!({
            "title": "Snapshot case",
            "steps": [{ "action": "A1", "expected_result": "E1" }]
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let case_id = created["case"]["id"].as_str().unwrap();
    assert_eq!(created["case"]["version"].as_i64().unwrap(), 1);

    let versions1: Value = client
        .get(format!(
            "{base}/api/v1/projects/{project_id}/cases/{case_id}/versions"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let arr1 = versions1.as_array().unwrap();
    assert_eq!(arr1.len(), 1);
    assert_eq!(arr1[0]["version"].as_i64().unwrap(), 1);

    let updated: Value = client
        .put(format!(
            "{base}/api/v1/projects/{project_id}/cases/{case_id}"
        ))
        .json(&serde_json::json!({
            "title": "Snapshot case edited",
            "change_summary": "title tweak",
            "expected_version": 1
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(updated["case"]["version"].as_i64().unwrap(), 2);

    let versions2: Value = client
        .get(format!(
            "{base}/api/v1/projects/{project_id}/cases/{case_id}/versions"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let arr2 = versions2.as_array().unwrap();
    assert_eq!(arr2.len(), 2, "update must add a snapshot for version 2");
    let versions: Vec<i64> = arr2
        .iter()
        .map(|v| v["version"].as_i64().unwrap())
        .collect();
    assert!(versions.contains(&1));
    assert!(versions.contains(&2));
    let v2 = arr2
        .iter()
        .find(|v| v["version"].as_i64() == Some(2))
        .unwrap();
    assert_eq!(v2["change_summary"].as_str().unwrap(), "title tweak");
}
