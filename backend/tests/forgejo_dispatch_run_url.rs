//! Forgejo dispatch resolves a concrete actions/runs/{id} URL after 204.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::State as AxumState,
    http::StatusCode,
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
struct ForgejoMock {
    list_hits: Arc<AtomicUsize>,
    public_base: String,
}

async fn dispatch() -> impl IntoResponse {
    StatusCode::NO_CONTENT
}

async fn list_runs(AxumState(state): AxumState<ForgejoMock>) -> impl IntoResponse {
    state.list_hits.fetch_add(1, Ordering::SeqCst);
    Json(json!({
        "workflow_runs": [{
            "id": 99,
            "name": "vulyk.yml",
            "path": ".forgejo/workflows/vulyk.yml",
            "event": "workflow_dispatch",
            "head_branch": "main",
            "html_url": format!("{}/acme/demo/actions/runs/99", state.public_base)
        }]
    }))
}

async fn spawn_forgejo_mock() -> (String, Arc<AtomicUsize>) {
    let list_hits = Arc::new(AtomicUsize::new(0));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind mock");
    let addr = listener.local_addr().expect("addr");
    let public_base = format!("http://{addr}");
    let state = ForgejoMock {
        list_hits: list_hits.clone(),
        public_base: public_base.clone(),
    };
    let app = Router::new()
        .route(
            "/api/v1/repos/acme/demo/actions/workflows/vulyk.yml/dispatches",
            post(dispatch),
        )
        .route("/api/v1/repos/acme/demo/actions/runs", get(list_runs))
        .with_state(state);
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve mock");
    });
    tokio::time::sleep(Duration::from_millis(30)).await;
    (public_base, list_hits)
}

#[tokio::test]
async fn forgejo_dispatch_resolves_run_url() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    std::env::set_var("VULYK_EGRESS_ALLOW_PRIVATE", "1");

    let base = spawn_api().await;
    let client = reqwest::Client::new();
    let (mock_base, list_hits) = spawn_forgejo_mock().await;

    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&json!({ "name": "Forgejo CI", "case_key_prefix": "FGJ" }))
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
            "provider": "forgejo",
            "name": "forgejo-e2e",
            "config": {
                "base_url": mock_base,
                "token": "fg-token",
                "owner": "acme",
                "repo": "demo",
                "workflow_id": "vulyk.yml",
                "ref": "main"
            }
        }))
        .send()
        .await
        .expect("create config")
        .json()
        .await
        .expect("config json");

    let dispatch: Value = client
        .post(format!(
            "{base}/api/v1/ci-configs/{}/dispatch",
            cfg["id"].as_str().unwrap()
        ))
        .json(&json!({ "run_name": "forgejo dispatch", "inputs": {} }))
        .send()
        .await
        .expect("dispatch")
        .json()
        .await
        .expect("dispatch json");

    assert!(
        list_hits.load(Ordering::SeqCst) >= 1,
        "expected runs list poll"
    );
    assert_eq!(dispatch["external_id"], "99");
    let url = dispatch["external_url"].as_str().unwrap_or("");
    assert!(
        url.contains("/actions/runs/99"),
        "expected concrete run URL, got {url}"
    );
    assert!(
        !url.contains("actions?workflow="),
        "must not keep workflow list URL: {url}"
    );
}
