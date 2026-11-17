//! Multi-repo project linking: default selection + CI open-run automation_repo_id (#255).

use std::time::Duration;

use axum::http::StatusCode;
use serde_json::{json, Value};

async fn spawn_api() -> String {
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
async fn project_can_link_several_repos_and_pick_default() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }

    let base = spawn_api().await;
    let client = reqwest::Client::new();

    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&json!({ "name": "Multi Repo Demo" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let project_id = project["id"].as_str().unwrap();

    let reject = client
        .post(format!("{base}/api/v1/automation-repos"))
        .json(&json!({
            "name": "bad",
            "provider": "svn",
            "remote_url": "https://github.com/acme/a",
            "project_ids": [project_id]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(reject.status(), StatusCode::BAD_REQUEST);

    let repo_a: Value = client
        .post(format!("{base}/api/v1/automation-repos"))
        .json(&json!({
            "name": "Alpha",
            "provider": "generic",
            "remote_url": "https://github.com/acme/alpha.git",
            "branch": "main",
            "project_ids": [project_id]
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let repo_b: Value = client
        .post(format!("{base}/api/v1/automation-repos"))
        .json(&json!({
            "name": "Beta",
            "provider": "forgejo",
            "remote_url": "https://github.com/acme/beta.git",
            "branch": "main",
            "project_ids": [project_id]
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let id_a = repo_a["id"].as_str().unwrap();
    let id_b = repo_b["id"].as_str().unwrap();
    let _ = id_a;

    let listed: Value = client
        .get(format!("{base}/api/v1/projects/{project_id}/automation-repos"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let items = listed.as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert!(items.iter().any(|r| r["is_default"] == true));

    let set_default: Value = client
        .put(format!(
            "{base}/api/v1/projects/{project_id}/automation-repos/default"
        ))
        .json(&json!({ "repo_id": id_b }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(set_default["id"], id_b);
    assert_eq!(set_default["is_default"], true);

    let listed2: Value = client
        .get(format!("{base}/api/v1/projects/{project_id}/automation-repos"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let default_ids: Vec<_> = listed2
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["is_default"] == true)
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    assert_eq!(default_ids, vec![id_b]);

    let open: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/ci/runs"))
        .json(&json!({
            "name": "CI from Beta",
            "provider": "forgejo",
            "automation_repo_id": id_b,
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
    assert_eq!(open["run"]["meta"]["automation_repo_id"], id_b);
    assert!(open["run"]["ci_repo"]
        .as_str()
        .unwrap_or("")
        .contains("beta"));

    let orphan = uuid::Uuid::new_v4();
    let missing = client
        .post(format!("{base}/api/v1/projects/{project_id}/ci/runs"))
        .json(&json!({
            "name": "missing link",
            "automation_repo_id": orphan,
            "start": false
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::BAD_REQUEST);
}
