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
use hmac::Mac;
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

    // Local file:// fixture remotes require the private-egress test escape hatch.
    std::env::set_var("VULYK_EGRESS_ALLOW_PRIVATE", "1");

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

#[derive(Clone)]
struct GhMockState {
    zipball_hits: Arc<AtomicUsize>,
    commit_hits: Arc<AtomicUsize>,
    zip_bytes: Arc<Vec<u8>>,
}

async fn gh_commit(AxumState(state): AxumState<GhMockState>) -> impl IntoResponse {
    state.commit_hits.fetch_add(1, Ordering::SeqCst);
    (
        StatusCode::OK,
        [("etag", "\"commit-etag-1\"")],
        Json(json!({
            "sha": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "commit": { "tree": { "sha": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb" } }
        })),
    )
}

async fn gh_zipball(AxumState(state): AxumState<GhMockState>) -> impl IntoResponse {
    state.zipball_hits.fetch_add(1, Ordering::SeqCst);
    (
        StatusCode::OK,
        [("content-type", "application/zip")],
        state.zip_bytes.as_ref().clone(),
    )
}

async fn gh_branches() -> Json<Value> {
    Json(json!([
        { "name": "main", "commit": { "sha": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" } },
        { "name": "develop", "commit": { "sha": "dddddddddddddddddddddddddddddddddddddddd" } }
    ]))
}

fn fixture_zipball() -> Vec<u8> {
    use std::io::{Cursor, Write};
    let mut buf = Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut buf);
        let opts = zip::write::SimpleFileOptions::default();
        zip.start_file("acme-demo-aaaaaaaa/e2e/login.spec.ts", opts)
            .unwrap();
        zip.write_all(b"export const ok = true;\n").unwrap();
        zip.start_file("acme-demo-aaaaaaaa/src/other.ts", opts)
            .unwrap();
        zip.write_all(b"export const skip = true;\n").unwrap();
        zip.finish().unwrap();
    }
    buf.into_inner()
}

/// Minimal GitHub Enterprise-shaped API for archive indexing tests.
async fn spawn_github_mock() -> (String, Arc<AtomicUsize>, Arc<AtomicUsize>) {
    let zipball_hits = Arc::new(AtomicUsize::new(0));
    let commit_hits = Arc::new(AtomicUsize::new(0));
    let state = GhMockState {
        zipball_hits: zipball_hits.clone(),
        commit_hits: commit_hits.clone(),
        zip_bytes: Arc::new(fixture_zipball()),
    };

    let app = Router::new()
        .route(
            "/api/v3/repos/{owner}/{repo}/commits/{branch}",
            axum::routing::get(gh_commit),
        )
        .route(
            "/api/v3/repos/{owner}/{repo}/zipball/{branch}",
            axum::routing::get(gh_zipball),
        )
        .route(
            "/api/v3/repos/{owner}/{repo}/branches",
            axum::routing::get(gh_branches),
        )
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind github mock");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve github mock");
    });
    tokio::time::sleep(Duration::from_millis(30)).await;
    (format!("http://{addr}"), zipball_hits, commit_hits)
}

#[tokio::test]
async fn github_archive_indexes_without_git_and_is_incremental() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    std::env::set_var("VULYK_EGRESS_ALLOW_PRIVATE", "1");

    let base = spawn_api().await;
    let client = reqwest::Client::new();
    let (mock_base, zipball_hits, _commit_hits) = spawn_github_mock().await;
    let remote = format!("{mock_base}/acme/demo");

    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&json!({ "name": "GH Archive Demo" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let project_id = project["id"].as_str().unwrap();

    // Preview test on unsaved form values (no 404).
    let preview: Value = client
        .post(format!("{base}/api/v1/automation-repos/test"))
        .json(&json!({
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
    assert_eq!(preview["ok"], true, "{preview}");

    // Legacy project test before save must not be a raw 404.
    let unsaved = client
        .post(format!(
            "{base}/api/v1/projects/{project_id}/automation-repo/test"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(unsaved.status(), StatusCode::BAD_REQUEST);
    let body: Value = unsaved.json().await.unwrap();
    let msg = body["error"]["message"].as_str().unwrap_or("");
    assert!(
        msg.to_lowercase().contains("save") || msg.to_lowercase().contains("form"),
        "expected friendly message, got {body}"
    );

    let repo: Value = client
        .post(format!("{base}/api/v1/automation-repos"))
        .json(&json!({
            "name": "GH Demo",
            "provider": "github",
            "remote_url": remote,
            "branch": "main",
            "subpath": "e2e",
            "project_ids": [project_id],
            "poll_interval_minutes": 15
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
    assert!(!repo["webhook_secret"].as_str().unwrap_or("").is_empty());
    assert!(repo["webhook_url"].as_str().unwrap_or("").contains(repo_id));
    assert_eq!(repo["poll_interval_minutes"], 15);

    let branches: Value = client
        .get(format!("{base}/api/v1/automation-repos/{repo_id}/branches"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(branches["branches"].as_array().unwrap().len() >= 2);

    let indexed: Value = client
        .post(format!("{base}/api/v1/automation-repos/{repo_id}/reindex"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(indexed["status"], "ready", "{indexed}");
    assert_eq!(indexed["file_count"], 1);
    assert!(indexed["chunk_count"].as_i64().unwrap() >= 1);
    assert_eq!(indexed["last_index_trigger"], "manual");
    let first_zips = zipball_hits.load(Ordering::SeqCst);
    assert_eq!(first_zips, 1, "expected one zipball download");

    // Second reindex: still one archive download, but unchanged content is not re-persisted as new.
    let again: Value = client
        .post(format!("{base}/api/v1/automation-repos/{repo_id}/reindex"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(again["status"], "ready");
    assert_eq!(again["file_count"], 1);
    assert_eq!(zipball_hits.load(Ordering::SeqCst), 2);

    // Push webhook on configured branch queues reindex.
    let secret = repo["webhook_secret"].as_str().unwrap();
    let payload = br#"{"ref":"refs/heads/main"}"#;
    let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(secret.as_bytes()).expect("hmac key");
    mac.update(payload);
    let sig = format!("sha256={}", hex::encode(mac.finalize().into_bytes()));
    let wh = client
        .post(format!("{base}/api/v1/webhooks/automation-repos/{repo_id}"))
        .header("X-GitHub-Event", "push")
        .header("X-Hub-Signature-256", sig)
        .body(payload.to_vec())
        .send()
        .await
        .unwrap();
    assert_eq!(wh.status(), StatusCode::OK);
    let ack: Value = wh.json().await.unwrap();
    assert_eq!(ack["status"], "reindex_queued");
}

#[tokio::test]
async fn generic_test_reports_missing_git_clearly_when_unavailable() {
    // Unit-level: when git binary is absent the helper surfaces a clear message.
    // Integration environments usually have git; skip the live call if present.
    if vulyk_api::services::automation_repo::git_available() {
        eprintln!("skipping: git is available in this environment");
        return;
    }
    let (ok, message, _) = vulyk_api::services::automation_repo::test_connection(
        "https://example.com/acme/demo.git",
        "main",
        None,
    )
    .unwrap();
    assert!(!ok);
    assert!(
        message.contains("git is not installed") || message.contains("git not available"),
        "{message}"
    );
}
