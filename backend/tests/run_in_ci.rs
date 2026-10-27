//! Run in CI: Queued run + GitHub workflow_dispatch inputs + attach via start.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::{extract::State as AxumState, http::StatusCode, routing::post, Json, Router};
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

#[derive(Clone)]
struct DispatchMock {
    hits: Arc<AtomicUsize>,
    last_body: Arc<tokio::sync::Mutex<Option<Value>>>,
}

async fn mock_dispatch(
    AxumState(state): AxumState<DispatchMock>,
    Json(body): Json<Value>,
) -> StatusCode {
    state.hits.fetch_add(1, Ordering::SeqCst);
    *state.last_body.lock().await = Some(body);
    StatusCode::NO_CONTENT
}

async fn spawn_github_dispatch_mock() -> (
    String,
    Arc<AtomicUsize>,
    Arc<tokio::sync::Mutex<Option<Value>>>,
) {
    let hits = Arc::new(AtomicUsize::new(0));
    let last_body = Arc::new(tokio::sync::Mutex::new(None));
    let state = DispatchMock {
        hits: hits.clone(),
        last_body: last_body.clone(),
    };
    let app = Router::new()
        .route(
            "/api/v3/repos/{owner}/{repo}/actions/workflows/{workflow}/dispatches",
            post(mock_dispatch),
        )
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind mock");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve mock");
    });
    tokio::time::sleep(Duration::from_millis(30)).await;
    (format!("http://{addr}"), hits, last_body)
}

#[tokio::test]
async fn run_in_ci_creates_queued_run_and_dispatches_with_vulyk_run_id() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    std::env::set_var("VULYK_EGRESS_ALLOW_PRIVATE", "1");

    let base = spawn_api().await;
    let client = reqwest::Client::new();
    let (mock_base, hits, last_body) = spawn_github_dispatch_mock().await;
    let remote = format!("{mock_base}/acme/demo");

    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&json!({ "name": "Run In CI Demo", "case_key_prefix": "RIC" }))
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
        .json(&json!({
            "title": "Sign in",
            "steps": [{ "action": "Open", "expected_result": "Form" }]
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let case_key = case
        .get("key")
        .and_then(|v| v.as_str())
        .or_else(|| case.pointer("/case/key").and_then(|v| v.as_str()))
        .unwrap()
        .to_string();
    let case_id = case
        .get("id")
        .and_then(|v| v.as_str())
        .or_else(|| case.pointer("/case/id").and_then(|v| v.as_str()))
        .unwrap()
        .to_string();

    let repo: Value = client
        .post(format!("{base}/api/v1/automation-repos"))
        .json(&json!({
            "name": "Demo",
            "provider": "github",
            "remote_url": remote,
            "branch": "main",
            "actions_token": "ghp_test_actions",
            "ci_workflow_file": "vulyk-dispatch-example.yml",
            "ci_workflow_ref": "main",
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
    assert_eq!(repo["has_actions_token"], true);
    assert_eq!(repo["ci_workflow_file"], "vulyk-dispatch-example.yml");

    // Point the upserted CI config at the mock API (owner/repo from remote URL).
    // run_in_ci builds api from github.com by default — inject api_base via a
    // pre-created config name matching the repo id prefix after first call fails?
    // Instead: patch the auto-created config after a dry path by calling with
    // GHE-style remote so api_base is the mock host.
    // Re-create repo with same mock remote (parse_remote uses host/api/v3).

    let res: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/run-in-ci"))
        .json(&json!({
            "case_ids": [case_id],
            "run_name": "CI demo"
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(res["run"]["status"], "queued", "{res}");
    assert_eq!(res["run"]["lifecycle"], "ci");
    assert!(res["case_keys"]
        .as_array()
        .unwrap()
        .iter()
        .any(|k| k.as_str() == Some(case_key.as_str())));
    assert_eq!(res["dispatch"]["status"], "dispatched");
    assert_eq!(hits.load(Ordering::SeqCst), 1);

    let body = last_body.lock().await.clone().expect("dispatch body");
    let run_id = res["run"]["id"].as_str().unwrap();
    assert_eq!(body["inputs"]["vulyk_run_id"], run_id);
    assert!(
        body["inputs"]["case_keys"]
            .as_str()
            .unwrap()
            .to_uppercase()
            .contains(&case_key.to_uppercase()),
        "{body}"
    );

    // Attach: start with pipeline URL (simulates workflow first step).
    let started: Value = client
        .post(format!(
            "{base}/api/v1/projects/{project_id}/ci/runs/{run_id}/start"
        ))
        .json(&json!({
            "pipeline_url": "https://github.com/acme/demo/actions/runs/99",
            "branch": "main",
            "commit": "abc"
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(started["status"], "in_progress");
    assert_eq!(
        started["pipeline_url"],
        "https://github.com/acme/demo/actions/runs/99"
    );
}
