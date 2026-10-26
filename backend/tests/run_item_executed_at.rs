//! Regression: assignee/comment-only run-item updates must not clobber executed_at.

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
async fn assignee_only_update_preserves_executed_at() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }

    let base = spawn_app().await;
    let client = reqwest::Client::new();

    let key = format!(
        "X{}",
        uuid::Uuid::new_v4().simple().to_string()[..6].to_uppercase()
    );
    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&serde_json::json!({ "key": key, "name": "Executed At Proj" }))
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
        .json(&serde_json::json!({ "title": "Preserve timestamp", "status": "ready" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let case_id = case["case"]["id"].as_str().unwrap();

    let run: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/runs"))
        .json(&serde_json::json!({
            "name": "Manual",
            "case_ids": [case_id],
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
    let run_id = run["run"]["id"].as_str().unwrap();
    let item_id = run["items"][0]["item"]["id"].as_str().unwrap();

    let marked: Value = client
        .put(format!(
            "{base}/api/v1/projects/{project_id}/runs/{run_id}/items/{item_id}"
        ))
        .json(&serde_json::json!({ "status": "passed" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(marked["item"]["status"], "Passed");
    let executed_at = marked["item"]["executed_at"]
        .as_str()
        .expect("executed_at set on status change")
        .to_string();

    // Give the clock a chance to move so a buggy NOW() rewrite would differ.
    tokio::time::sleep(Duration::from_millis(50)).await;

    let updated: Value = client
        .put(format!(
            "{base}/api/v1/projects/{project_id}/runs/{run_id}/items/{item_id}"
        ))
        .json(&serde_json::json!({
            "assignee": "qa@example.com",
            "comment": "note only"
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(updated["item"]["status"], "Passed");
    assert_eq!(updated["item"]["assignee"], "qa@example.com");
    assert_eq!(updated["item"]["comment"], "note only");
    assert_eq!(
        updated["item"]["executed_at"].as_str().unwrap(),
        executed_at,
        "assignee/comment-only update must preserve executed_at"
    );

    // Explicit status change still refreshes executed_at.
    let retest: Value = client
        .put(format!(
            "{base}/api/v1/projects/{project_id}/runs/{run_id}/items/{item_id}"
        ))
        .json(&serde_json::json!({ "status": "failed" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(retest["item"]["status"], "Failed");
    assert_ne!(
        retest["item"]["executed_at"].as_str().unwrap(),
        executed_at,
        "explicit status change should refresh executed_at"
    );
}
