//! Generate cases from a PR/branch diff (#256).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::{extract::Path, http::StatusCode, routing::get, Json, Router};
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

async fn spawn_github_mock() -> (String, Arc<AtomicUsize>) {
    let hits = Arc::new(AtomicUsize::new(0));
    let hits_c = hits.clone();
    let app = Router::new()
        .route(
            "/api/v3/repos/{owner}/{repo}/pulls",
            get(|| async {
                Json(json!([{
                    "number": 12,
                    "title": "Add login",
                    "state": "open",
                    "html_url": "https://github.com/acme/demo/pull/12",
                    "updated_at": "2026-11-17T00:00:00Z",
                    "head": { "ref": "feature/login", "sha": "abc123deadbeef" },
                    "base": { "ref": "main", "sha": "base000" }
                }]))
            }),
        )
        .route(
            "/api/v3/repos/{owner}/{repo}/compare/{spec}",
            get({
                let hits = hits_c.clone();
                move |Path((_, _, spec)): Path<(String, String, String)>| {
                    let hits = hits.clone();
                    async move {
                        hits.fetch_add(1, Ordering::SeqCst);
                        assert!(spec.contains("..."), "spec={spec}");
                        Json(json!({
                            "sha": "abc123deadbeef",
                            "base_commit": { "sha": "base000" },
                            "files": [{
                                "filename": "src/login.ts",
                                "status": "modified",
                                "additions": 12,
                                "deletions": 2,
                                "patch": "@@ -1,3 +1,5 @@\n+export function login() { return true }\n function noop() {}"
                            }, {
                                "filename": "README.md",
                                "status": "modified",
                                "additions": 1,
                                "deletions": 0,
                                "patch": "@@ -1 +1,2 @@\n+# login\n docs"
                            }]
                        }))
                    }
                }
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind mock");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve mock");
    });
    tokio::time::sleep(Duration::from_millis(30)).await;
    (format!("http://{addr}"), hits)
}

#[tokio::test]
async fn preview_and_generate_from_pull_diff() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    std::env::set_var("VULYK_EGRESS_ALLOW_PRIVATE", "1");

    let base = spawn_api().await;
    let (mock, hits) = spawn_github_mock().await;
    let client = reqwest::Client::new();

    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&json!({ "name": "Diff Gen" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let project_id = project["id"].as_str().unwrap();

    let remote = format!("{mock}/acme/demo");
    let repo: Value = client
        .post(format!("{base}/api/v1/automation-repos"))
        .json(&json!({
            "name": "Demo",
            "provider": "github",
            "remote_url": remote,
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
    let repo_id = repo["id"].as_str().unwrap();

    let pulls: Value = client
        .get(format!("{base}/api/v1/automation-repos/{repo_id}/pulls"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(pulls.as_array().unwrap().len(), 1);
    assert_eq!(pulls[0]["number"], 12);

    let preview: Value = client
        .post(format!(
            "{base}/api/v1/projects/{project_id}/llm/diff-preview"
        ))
        .json(&json!({
            "repo_id": repo_id,
            "pull_number": 12,
            "exclude_paths": ["README.md"]
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(preview["files"].as_array().unwrap().len(), 2);
    assert_eq!(preview["selected_files"].as_array().unwrap().len(), 1);
    assert_eq!(preview["selected_files"][0]["path"], "src/login.ts");
    assert!(preview["selected_approx_tokens"].as_i64().unwrap() > 0);
    assert!(hits.load(Ordering::SeqCst) >= 1);

    let generated: Value = client
        .post(format!(
            "{base}/api/v1/projects/{project_id}/llm/generate-cases-from-diff"
        ))
        .json(&json!({
            "repo_id": repo_id,
            "pull_number": 12,
            "exclude_paths": ["README.md"],
            "persist": true,
            "count": 2
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(generated["drafts"].as_array().unwrap().len() >= 1);
    assert!(!generated["persisted"].as_array().unwrap().is_empty());
    let case = &generated["persisted"][0];
    let gen = &case["generation"];
    assert_eq!(gen["source"], "diff");
    assert_eq!(gen["pull_number"], 12);
    assert_eq!(gen["base_ref"], "main");
    assert_eq!(gen["head_ref"], "feature/login");
    assert!(gen["included_paths"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p == "src/login.ts"));

    let case_id = case["case"]["id"].as_str().unwrap();
    let detail: Value = client
        .get(format!(
            "{base}/api/v1/projects/{project_id}/cases/{case_id}"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(detail["generation"]["pull_number"], 12);
    assert_eq!(detail["generation"]["commit_sha"], "abc123deadbeef");

    let _ = StatusCode::OK;
}
