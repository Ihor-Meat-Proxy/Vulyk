//! Woodpecker CI config: create, test connection, dispatch with status follow.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::State as AxumState,
    http::HeaderMap,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
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
struct WoodpeckerMock {
    trigger_hits: Arc<AtomicUsize>,
    last_body: Arc<tokio::sync::Mutex<Option<Value>>>,
    poll_hits: Arc<AtomicUsize>,
}

async fn user() -> impl IntoResponse {
    Json(json!({ "login": "vulyk-bot" }))
}

async fn lookup() -> impl IntoResponse {
    Json(json!({
        "id": 7,
        "full_name": "acme/demo",
        "name": "demo",
        "owner": "acme"
    }))
}

async fn trigger(
    AxumState(state): AxumState<WoodpeckerMock>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    state.trigger_hits.fetch_add(1, Ordering::SeqCst);
    *state.last_body.lock().await = Some(body);
    assert!(headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v == "Bearer wp-token"));
    Json(json!({
        "id": 100,
        "number": 42,
        "status": "pending",
        "branch": "main"
    }))
}

async fn poll(AxumState(state): AxumState<WoodpeckerMock>) -> impl IntoResponse {
    state.poll_hits.fetch_add(1, Ordering::SeqCst);
    Json(json!({
        "id": 100,
        "number": 42,
        "status": "running",
        "branch": "main"
    }))
}

async fn spawn_woodpecker_mock() -> (
    String,
    Arc<AtomicUsize>,
    Arc<tokio::sync::Mutex<Option<Value>>>,
) {
    let trigger_hits = Arc::new(AtomicUsize::new(0));
    let last_body = Arc::new(tokio::sync::Mutex::new(None));
    let poll_hits = Arc::new(AtomicUsize::new(0));
    let state = WoodpeckerMock {
        trigger_hits: trigger_hits.clone(),
        last_body: last_body.clone(),
        poll_hits: poll_hits.clone(),
    };
    let app = Router::new()
        .route("/api/user", get(user))
        .route("/api/repos/lookup/{repo}", get(lookup))
        .route("/api/repos/{id}/pipelines", post(trigger))
        .route("/api/repos/{id}/pipelines/{number}", get(poll))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind mock");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve mock");
    });
    tokio::time::sleep(Duration::from_millis(30)).await;
    (format!("http://{addr}"), trigger_hits, last_body)
}

#[tokio::test]
async fn woodpecker_test_and_dispatch_follow_status() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    std::env::set_var("VULYK_EGRESS_ALLOW_PRIVATE", "1");

    let base = spawn_api().await;
    let client = reqwest::Client::new();
    let (mock_base, hits, last_body) = spawn_woodpecker_mock().await;

    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&json!({ "name": "Woodpecker CI", "case_key_prefix": "WPC" }))
        .send()
        .await
        .expect("create project")
        .json()
        .await
        .expect("project json");
    let project_id = project["id"].as_str().expect("id");

    let cfg: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/ci-configs"))
        .json(&json!({
            "provider": "woodpecker",
            "name": "woodpecker-e2e",
            "config": {
                "base_url": mock_base,
                "token": "wp-token",
                "repo": "acme/demo",
                "branch": "main"
            }
        }))
        .send()
        .await
        .expect("create config")
        .json()
        .await
        .expect("config json");
    assert_eq!(cfg["provider"], "woodpecker");
    assert_eq!(cfg["has_token"], true);
    assert!(cfg["config"].get("token").is_none());

    let test: Value = client
        .post(format!(
            "{base}/api/v1/ci-configs/{}/test",
            cfg["id"].as_str().unwrap()
        ))
        .send()
        .await
        .expect("test")
        .json()
        .await
        .expect("test json");
    assert_eq!(test["ok"], true);
    assert!(test["message"].as_str().unwrap().contains("vulyk-bot"));
    assert!(test["message"].as_str().unwrap().contains("acme/demo"));

    let dispatch: Value = client
        .post(format!(
            "{base}/api/v1/ci-configs/{}/dispatch",
            cfg["id"].as_str().unwrap()
        ))
        .json(&json!({ "run_name": "woodpecker dispatch", "inputs": {} }))
        .send()
        .await
        .expect("dispatch")
        .json()
        .await
        .expect("dispatch json");

    assert_eq!(hits.load(Ordering::SeqCst), 1);
    assert_eq!(dispatch["status"], "running");
    assert_eq!(dispatch["external_id"], "42");
    assert_eq!(
        dispatch["external_url"],
        format!("{mock_base}/repos/acme/demo/pipeline/42")
    );

    let body = last_body.lock().await.clone().expect("trigger body");
    let vars = body["variables"].as_object().expect("variables");
    assert!(
        vars.contains_key("VULYK_RUN_ID") || vars.contains_key("vulyk_run_id"),
        "variables should carry run id: {body}"
    );
    assert!(vars.contains_key("VULYK_PROJECT_ID"), "variables={vars:?}");

    let run_id = dispatch["run_id"].as_str().expect("run_id");
    let detail: Value = client
        .get(format!("{base}/api/v1/runs/{run_id}"))
        .send()
        .await
        .expect("run")
        .json()
        .await
        .expect("run json");
    assert!(detail["pipeline_url"]
        .as_str()
        .unwrap_or("")
        .contains("/repos/acme/demo/pipeline/42"));
}
