//! Automation repo index + generate-code context (local fixture, stubbed LLM).

use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::State as AxumState, http::StatusCode, response::IntoResponse, routing::post, Json,
    Router,
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
    reply: String,
}

async fn stub_chat(
    AxumState(state): AxumState<StubState>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    state.hits.fetch_add(1, Ordering::SeqCst);
    let user = body["messages"]
        .as_array()
        .and_then(|a| a.iter().find(|m| m["role"] == "user"))
        .and_then(|m| m["content"].as_str())
        .unwrap_or("");
    let reply = if user.contains("login.page") || user.contains("Repository context") {
        format!(
            "// reused LoginPage\nimport {{ LoginPage }} from '../pages/login.page';\n{}",
            state.reply
        )
    } else {
        state.reply.clone()
    };
    (
        StatusCode::OK,
        Json(json!({
            "choices": [{ "message": { "content": reply } }]
        })),
    )
}

async fn spawn_stub(reply: &str) -> (String, Arc<AtomicUsize>) {
    let hits = Arc::new(AtomicUsize::new(0));
    let state = StubState {
        hits: hits.clone(),
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

fn fixture_git_url() -> String {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/automation_repo");
    let tmp = std::env::temp_dir().join(format!("vulyk-fixture-{}", uuid::Uuid::new_v4().simple()));
    let _ = std::fs::remove_dir_all(&tmp);
    copy_dir(&src, &tmp).expect("copy fixture");
    Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(&tmp)
        .status()
        .expect("git init");
    Command::new("git")
        .args(["config", "user.email", "test@vulyk.local"])
        .current_dir(&tmp)
        .status()
        .ok();
    Command::new("git")
        .args(["config", "user.name", "test"])
        .current_dir(&tmp)
        .status()
        .ok();
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(&tmp)
        .status()
        .expect("git add");
    Command::new("git")
        .args(["commit", "-m", "fixture"])
        .current_dir(&tmp)
        .status()
        .expect("git commit");
    // file:// URL for local clone
    format!("file://{}", tmp.canonicalize().unwrap().display())
}

fn copy_dir(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let to = dst.join(entry.file_name());
        if ty.is_dir() {
            if entry.file_name() == ".git" {
                continue;
            }
            copy_dir(&entry.path(), &to)?;
        } else {
            std::fs::copy(entry.path(), to)?;
        }
    }
    Ok(())
}

#[tokio::test]
async fn indexes_fixture_repo_and_injects_context_into_generate_code() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }

    let base = spawn_api().await;
    let client = reqwest::Client::new();
    let (stub_url, hits) = spawn_stub("test('generated', async ({ page }) => {});").await;
    let remote = fixture_git_url();

    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&json!({ "name": "Automation Context Demo" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let project_id = project["id"].as_str().unwrap();

    let repo: Value = client
        .put(format!(
            "{base}/api/v1/projects/{project_id}/automation-repo"
        ))
        .json(&json!({
            "provider": "generic",
            "remote_url": remote,
            "branch": "main",
            "subpath": ""
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(repo["provider"], "generic");
    assert_eq!(repo["has_token"], false);

    let test: Value = client
        .post(format!(
            "{base}/api/v1/projects/{project_id}/automation-repo/test"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(test["ok"], true);

    let indexed: Value = client
        .post(format!(
            "{base}/api/v1/projects/{project_id}/automation-repo/reindex"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(indexed["status"], "ready");
    assert!(indexed["file_count"].as_i64().unwrap() >= 3);
    assert!(indexed["chunk_count"].as_i64().unwrap() >= 3);

    client
        .post(format!("{base}/api/v1/projects/{project_id}/llm/configs"))
        .json(&json!({
            "provider": "openai_compatible",
            "name": "Stub",
            "model": "stub-1",
            "base_url": stub_url,
            "api_key": "sk-test",
            "is_default": true
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let case: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/cases"))
        .json(&json!({
            "title": "User can sign in with email",
            "description": "Login form accepts credentials",
            "steps": [{ "action": "Open login", "expected_result": "Form visible" }]
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let case_id = case["id"]
        .as_str()
        .or_else(|| case["case"]["id"].as_str())
        .unwrap();

    let gen: Value = client
        .post(format!(
            "{base}/api/v1/projects/{project_id}/llm/generate-code"
        ))
        .json(&json!({
            "case_ids": [case_id],
            "framework": "playwright"
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(gen["provider"], "openai_compatible");
    assert_eq!(gen["model"], "stub-1");
    assert!(gen["code"].as_str().unwrap().contains("LoginPage"));
    let ctx = gen["context_files"].as_array().unwrap();
    assert!(
        !ctx.is_empty(),
        "expected context files from indexed repo, got {ctx:?}"
    );
    assert!(
        ctx.iter()
            .any(|f| f["path"].as_str().unwrap().contains("login")
                || f["path"].as_str().unwrap().contains("playwright.config")
                || f["path"].as_str().unwrap().contains("package.json")),
        "expected login/config context, got {ctx:?}"
    );
    assert_eq!(hits.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn developer_has_llm_assist_in_role_matrix() {
    // Unit-level: ensure RBAC matrix includes the documented Generate permissions.
    use vulyk_api::auth::{Permission, ProjectRole};
    assert!(ProjectRole::Developer.allows(Permission::LlmAssist));
    assert!(!ProjectRole::Developer.allows(Permission::ManageAutomationRepo));
    assert!(ProjectRole::TestAutomationEngineer.allows(Permission::ManageAutomationRepo));
    assert!(ProjectRole::TestArchitect.allows(Permission::ManageAutomationRepo));
    assert!(!ProjectRole::Viewer.allows(Permission::LlmAssist));
}
