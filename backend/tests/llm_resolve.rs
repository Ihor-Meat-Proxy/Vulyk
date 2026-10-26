//! LLM provider resolution + stubbed HTTP provider (no real OpenRouter).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use tokio::sync::Mutex;

use axum::{
    extract::State as AxumState, http::StatusCode, response::IntoResponse, routing::post, Json,
    Router,
};
use serde_json::{json, Value};

fn test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

async fn lock_tests() -> tokio::sync::MutexGuard<'static, ()> {
    test_lock().lock().await
}

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
struct StubState {
    hits: Arc<AtomicUsize>,
    fail_with: Option<u16>,
    reply: String,
}

async fn stub_chat(
    AxumState(state): AxumState<StubState>,
    Json(_body): Json<Value>,
) -> impl IntoResponse {
    state.hits.fetch_add(1, Ordering::SeqCst);
    if let Some(code) = state.fail_with {
        return (
            StatusCode::from_u16(code).unwrap_or(StatusCode::TOO_MANY_REQUESTS),
            Json(json!({ "error": { "message": "rate limited by stub" } })),
        )
            .into_response();
    }
    (
        StatusCode::OK,
        Json(json!({
            "choices": [{
                "message": { "content": state.reply }
            }]
        })),
    )
        .into_response()
}

async fn spawn_stub(fail_with: Option<u16>, reply: &str) -> (String, Arc<AtomicUsize>) {
    let hits = Arc::new(AtomicUsize::new(0));
    let state = StubState {
        hits: hits.clone(),
        fail_with,
        reply: reply.to_string(),
    };
    let app = Router::new()
        .route("/chat/completions", post(stub_chat))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind stub");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve stub");
    });
    tokio::time::sleep(Duration::from_millis(30)).await;
    (format!("http://{addr}"), hits)
}

async fn create_project(client: &reqwest::Client, base: &str, name: &str) -> Value {
    client
        .post(format!("{base}/api/v1/projects"))
        .json(&json!({ "name": name }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap()
}

async fn clear_global_llm_configs(client: &reqwest::Client, base: &str, project_id: &str) {
    let list: Value = client
        .get(format!("{base}/api/v1/projects/{project_id}/llm/configs"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    for cfg in list.as_array().cloned().unwrap_or_default() {
        if cfg["project_id"].is_null() {
            let id = cfg["id"].as_str().unwrap();
            client
                .delete(format!(
                    "{base}/api/v1/projects/{project_id}/llm/configs/{id}"
                ))
                .send()
                .await
                .unwrap()
                .error_for_status()
                .unwrap();
        }
    }
}

fn uniq(prefix: &str) -> String {
    format!(
        "{prefix}{}",
        uuid::Uuid::new_v4().simple().to_string()[..8].to_uppercase()
    )
}

#[tokio::test]
async fn resolves_project_provider_over_global_and_calls_stub() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let _guard = lock_tests().await;

    let base = spawn_api().await;
    let client = reqwest::Client::new();
    let (stub_url, hits) = spawn_stub(None, "stub-project-answer").await;

    let project = create_project(&client, &base, &format!("LLM Resolve {}", uniq("P"))).await;
    let project_id = project["id"].as_str().unwrap();
    clear_global_llm_configs(&client, &base, project_id).await;

    // Global default (openai_compatible stub) — should lose to project config.
    let global: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/llm/configs"))
        .json(&json!({
            "provider": "openai_compatible",
            "name": "Global stub",
            "model": "global-model",
            "base_url": stub_url,
            "api_key": "sk-global",
            "is_default": true,
            "global": true
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(global["project_id"].is_null());
    assert_eq!(global["is_default"], true);

    let (project_stub, project_hits) = spawn_stub(None, "stub-from-project-provider").await;
    let project_cfg: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/llm/configs"))
        .json(&json!({
            "provider": "openai_compatible",
            "name": "Project stub",
            "model": "project-model",
            "base_url": project_stub,
            "api_key": "sk-project",
            "is_default": true,
            "global": false
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(project_cfg["project_id"], project_id);
    assert_eq!(project_cfg["model"], "project-model");

    let ask: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/llm/ask"))
        .json(&json!({ "question": "What coverage exists?" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(ask["provider"], "openai_compatible");
    assert_eq!(ask["model"], "project-model");
    assert!(ask["answer"]
        .as_str()
        .unwrap()
        .contains("stub-from-project-provider"));
    assert_eq!(project_hits.load(Ordering::SeqCst), 1);
    assert_eq!(
        hits.load(Ordering::SeqCst),
        0,
        "global stub must not be called"
    );

    clear_global_llm_configs(&client, &base, project_id).await;
}

#[tokio::test]
async fn uses_global_default_when_project_has_no_config() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let _guard = lock_tests().await;

    let base = spawn_api().await;
    let client = reqwest::Client::new();
    let (stub_url, hits) = spawn_stub(None, "from-global-default").await;

    let project_a = create_project(&client, &base, &format!("Has Global {}", uniq("G"))).await;
    let a_id = project_a["id"].as_str().unwrap();
    clear_global_llm_configs(&client, &base, a_id).await;

    let _global: Value = client
        .post(format!("{base}/api/v1/projects/{a_id}/llm/configs"))
        .json(&json!({
            "provider": "openrouter",
            "name": "OR",
            "model": "liquid/lfm-2.5-2.6b:free",
            "base_url": stub_url,
            "api_key": "sk-or-test",
            "is_default": true,
            "global": true
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();

    // Second project with no project-scoped config should use the global default.
    let project_b =
        create_project(&client, &base, &format!("Kyiv Concert Chat {}", uniq("K"))).await;
    let b_id = project_b["id"].as_str().unwrap();

    let ask: Value = client
        .post(format!("{base}/api/v1/projects/{b_id}/llm/ask"))
        .json(&json!({ "question": "Summarise coverage" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(ask["provider"], "openrouter");
    assert_eq!(ask["model"], "liquid/lfm-2.5-2.6b:free");
    assert!(ask["answer"]
        .as_str()
        .unwrap()
        .contains("from-global-default"));
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    assert_ne!(ask["provider"], "mock");

    clear_global_llm_configs(&client, &base, a_id).await;
}

#[tokio::test]
async fn provider_http_error_is_surfaced_not_mock() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let _guard = lock_tests().await;

    let base = spawn_api().await;
    let client = reqwest::Client::new();
    let (stub_url, hits) = spawn_stub(Some(429), "should-not-appear").await;

    let project = create_project(&client, &base, &format!("Rate Limit {}", uniq("R"))).await;
    let project_id = project["id"].as_str().unwrap();
    clear_global_llm_configs(&client, &base, project_id).await;

    client
        .post(format!("{base}/api/v1/projects/{project_id}/llm/configs"))
        .json(&json!({
            "provider": "openrouter",
            "name": "OR",
            "model": "liquid/lfm-2.5-2.6b:free",
            "base_url": stub_url,
            "api_key": "sk-or-test",
            "is_default": true
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let resp = client
        .post(format!("{base}/api/v1/projects/{project_id}/llm/ask"))
        .json(&json!({ "question": "Anything?" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
    let body: Value = resp.json().await.unwrap();
    let message = body["error"]["message"].as_str().unwrap();
    assert!(
        message.contains("openrouter") && message.contains("liquid/lfm-2.5-2.6b:free"),
        "expected provider/model in error, got: {message}"
    );
    assert!(
        message.contains("429") || message.contains("Rate limited"),
        "expected rate-limit signal, got: {message}"
    );
    assert!(!message.contains("mock-1"));
    assert_eq!(hits.load(Ordering::SeqCst), 1);

    // generate-cases shares the same resolver / error path (no silent mock).
    let resp = client
        .post(format!(
            "{base}/api/v1/projects/{project_id}/llm/generate-cases"
        ))
        .json(&json!({
            "requirements_text": "Login works",
            "count": 1
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        400,
        "generate-cases should not fall back to mock"
    );
    let body: Value = resp.json().await.unwrap();
    let message = body["error"]["message"].as_str().unwrap();
    assert!(message.contains("openrouter"), "generate-cases: {message}");
}

#[tokio::test]
async fn mock_only_when_no_configs_exist() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let _guard = lock_tests().await;

    let base = spawn_api().await;
    let client = reqwest::Client::new();
    let project = create_project(&client, &base, &format!("No Provider {}", uniq("N"))).await;
    let project_id = project["id"].as_str().unwrap();
    clear_global_llm_configs(&client, &base, project_id).await;

    let ask: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/llm/ask"))
        .json(&json!({ "question": "Anything?" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(ask["provider"], "mock");
    assert_eq!(ask["model"], "mock-1");
}

#[tokio::test]
async fn create_project_derives_key_from_name() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let _guard = lock_tests().await;

    let base = spawn_api().await;
    let client = reqwest::Client::new();
    let token = uniq("Z");
    let name = format!("Zeta Unique Concert {token}");
    let project = create_project(&client, &base, &name).await;
    let key = project["key"].as_str().unwrap();
    assert!(
        key.starts_with("ZUC"),
        "expected initials ZUC…, got {key} for name {name}"
    );
    assert!(project["case_key_prefix"]
        .as_str()
        .unwrap()
        .starts_with("ZUC"));

    // Collision: second project with same name gets a distinct key.
    let again = create_project(&client, &base, &name).await;
    assert_ne!(again["key"], project["key"]);
    assert!(again["key"].as_str().unwrap().starts_with("ZUC"));
}
