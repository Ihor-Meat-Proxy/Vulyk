//! Jenkins CI config: create, test connection, dispatch with crumb + queue follow.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::State as AxumState,
    http::{HeaderMap, HeaderValue, StatusCode},
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
struct JenkinsMock {
    trigger_hits: Arc<AtomicUsize>,
    last_form: Arc<tokio::sync::Mutex<Option<String>>>,
}

async fn crumb() -> impl IntoResponse {
    Json(json!({
        "crumb": "test-crumb-value",
        "crumbRequestField": "Jenkins-Crumb"
    }))
}

async fn whoami() -> impl IntoResponse {
    Json(json!({ "name": "vulyk-bot" }))
}

async fn job_api() -> impl IntoResponse {
    Json(json!({
        "name": "e2e",
        "url": "http://jenkins.local/job/folder/job/e2e/",
        "buildable": true
    }))
}

async fn trigger(
    AxumState(state): AxumState<JenkinsMock>,
    headers: HeaderMap,
    body: String,
) -> impl IntoResponse {
    state.trigger_hits.fetch_add(1, Ordering::SeqCst);
    *state.last_form.lock().await = Some(body);
    assert_eq!(
        headers.get("Jenkins-Crumb").and_then(|v| v.to_str().ok()),
        Some("test-crumb-value")
    );
    let mut res_headers = HeaderMap::new();
    res_headers.insert(
        axum::http::header::LOCATION,
        HeaderValue::from_static("/queue/item/99/"),
    );
    (StatusCode::CREATED, res_headers)
}

async fn queue_item() -> impl IntoResponse {
    Json(json!({
        "executable": {
            "number": 42,
            "url": "http://jenkins.local/job/folder/job/e2e/42/"
        }
    }))
}

async fn spawn_jenkins_mock() -> (
    String,
    Arc<AtomicUsize>,
    Arc<tokio::sync::Mutex<Option<String>>>,
) {
    let trigger_hits = Arc::new(AtomicUsize::new(0));
    let last_form = Arc::new(tokio::sync::Mutex::new(None));
    let state = JenkinsMock {
        trigger_hits: trigger_hits.clone(),
        last_form: last_form.clone(),
    };
    let app = Router::new()
        .route("/crumbIssuer/api/json", get(crumb))
        .route("/whoAmI/api/json", get(whoami))
        .route("/job/folder/job/e2e/api/json", get(job_api))
        .route("/job/folder/job/e2e/buildWithParameters", post(trigger))
        .route("/queue/item/99/api/json", get(queue_item))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind mock");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve mock");
    });
    tokio::time::sleep(Duration::from_millis(30)).await;
    (format!("http://{addr}"), trigger_hits, last_form)
}

#[tokio::test]
async fn jenkins_test_and_dispatch_follow_queue() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    std::env::set_var("VULYK_EGRESS_ALLOW_PRIVATE", "1");

    let base = spawn_api().await;
    let client = reqwest::Client::new();
    let (mock_base, hits, last_form) = spawn_jenkins_mock().await;

    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&json!({ "name": "Jenkins CI", "case_key_prefix": "JNK" }))
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
            "provider": "jenkins",
            "name": "jenkins-e2e",
            "config": {
                "base_url": mock_base,
                "job_path": "folder/e2e",
                "username": "bot",
                "token": "api-token",
                "use_crumb": true
            }
        }))
        .send()
        .await
        .expect("create config")
        .json()
        .await
        .expect("config json");
    assert_eq!(cfg["provider"], "jenkins");
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

    let dispatch: Value = client
        .post(format!(
            "{base}/api/v1/ci-configs/{}/dispatch",
            cfg["id"].as_str().unwrap()
        ))
        .json(&json!({ "run_name": "jenkins dispatch", "inputs": {} }))
        .send()
        .await
        .expect("dispatch")
        .json()
        .await
        .expect("dispatch json");

    assert_eq!(hits.load(Ordering::SeqCst), 1);
    assert_eq!(dispatch["status"], "running");
    assert_eq!(dispatch["external_id"], "42");
    assert!(dispatch["external_url"]
        .as_str()
        .unwrap()
        .contains("/job/folder/job/e2e/42"));

    let form = last_form.lock().await.clone().unwrap_or_default();
    assert!(
        form.contains("VULYK_RUN_ID=") || form.contains("vulyk_run_id="),
        "form should carry run id: {form}"
    );
    assert!(form.contains("VULYK_PROJECT_ID="), "form={form}");

    let run_id = dispatch["run_id"].as_str().expect("run_id");
    let detail: Value = client
        .get(format!("{base}/api/v1/runs/{run_id}"))
        .send()
        .await
        .expect("get run")
        .json()
        .await
        .expect("run json");
    let pipeline = detail["run"]["pipeline_url"]
        .as_str()
        .or_else(|| detail["pipeline_url"].as_str())
        .unwrap_or("");
    assert!(
        pipeline.contains("/42"),
        "pipeline_url={pipeline:?} detail={detail}"
    );
}
