//! Content backup export→import round-trip and destination mocks.

#![allow(clippy::type_complexity)]

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::State as AxumState,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, put},
    Json, Router,
};
use serde_json::{json, Value};
use tokio::sync::Mutex;

async fn spawn_app() -> (String, sqlx::PgPool) {
    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for integration tests");
    let pool = vulyk_api::db::connect(&database_url)
        .await
        .expect("connect db");
    vulyk_api::db::migrate(&pool).await.expect("migrate");

    let state = vulyk_api::routes::AppState {
        pool: pool.clone(),
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
    (format!("http://{addr}"), pool)
}

async fn ensure_admin_token(client: &reqwest::Client, base: &str, pool: &sqlx::PgPool) -> String {
    let email = format!("backup-{}@vulyk.test", uuid::Uuid::new_v4().simple());
    let password = "BackupAdm1Pass12";
    let hash = vulyk_api::auth::hash_password(password).expect("hash");
    let _uid: uuid::Uuid = sqlx::query_scalar(
        r#"INSERT INTO users (email, display_name, password_hash, is_system_admin)
           VALUES ($1, 'Backup Admin', $2, TRUE)
           RETURNING id"#,
    )
    .bind(&email)
    .bind(&hash)
    .fetch_one(pool)
    .await
    .expect("insert admin");
    // Mark setup completed so other checks are happy
    let _ = sqlx::query(
        r#"INSERT INTO system_settings (key, value) VALUES ('setup_completed', 'true'::jsonb)
           ON CONFLICT (key) DO UPDATE SET value = 'true'::jsonb"#,
    )
    .execute(pool)
    .await;

    let res: Value = client
        .post(format!("{base}/api/v1/auth/login"))
        .json(&json!({ "email": email, "password": password }))
        .send()
        .await
        .expect("login")
        .error_for_status()
        .expect("login status")
        .json()
        .await
        .expect("login json");
    res["token"].as_str().expect("token").to_string()
}

#[tokio::test]
async fn export_import_roundtrip_local_destination() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let (base, pool) = spawn_app().await;
    let client = reqwest::Client::new();
    let token = ensure_admin_token(&client, &base, &pool).await;

    // Create project + case via API (auth optional)
    let key = format!(
        "B{}",
        uuid::Uuid::new_v4().simple().to_string()[..6].to_uppercase()
    );
    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&json!({ "key": key, "name": "Backup Proj", "description": "rt" }))
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
            "title": "Roundtrip case",
            "description": "d",
            "precondition": "p",
            "priority": "high",
            "status": "ready",
            "steps": [{ "action": "Do one thing", "expected_result": "It works", "data": "" }],
            "tag_names": ["backup-rt"]
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let case_key = case["case"]["key"].as_str().unwrap().to_string();

    // Custom field + value
    let field: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/custom-fields"))
        .json(&json!({
            "name": "Browser",
            "field_type": "text",
            "required": false,
            "sort_order": 0
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let field_id = field["id"].as_str().unwrap();
    let case_id = case["case"]["id"].as_str().unwrap();
    let _ = client
        .post(format!(
            "{base}/api/v1/projects/{project_id}/cases/{case_id}/custom-fields"
        ))
        .json(&json!([{ "field_id": field_id, "value": "Firefox" }]))
        .send()
        .await;

    let tmp = tempfile_dir();
    // Local destinations must stay under VULYK_CONTENT_BACKUP_DIR.
    std::env::set_var("VULYK_CONTENT_BACKUP_DIR", &tmp);
    let mut headers = reqwest::header::HeaderMap::new();
    if !token.is_empty() {
        headers.insert(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {token}").parse().unwrap(),
        );
    }

    let dest: Value = client
        .post(format!("{base}/api/v1/admin/backups/destinations"))
        .headers(headers.clone())
        .json(&json!({
            "name": "Local test",
            "kind": "local",
            "config": { "path": tmp },
            "enabled": true
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let dest_id = dest["id"].as_str().unwrap();

    let test: Value = client
        .post(format!(
            "{base}/api/v1/admin/backups/destinations/{dest_id}/test"
        ))
        .headers(headers.clone())
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(test["ok"], true);

    let run: Value = client
        .post(format!("{base}/api/v1/admin/backups/run"))
        .headers(headers.clone())
        .json(&json!({
            "destination_id": dest_id,
            "project_id": project_id,
            "pack_mode": "zip",
            "include_attachments": false
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(run["status"], "success", "{run}");
    assert!(run["size_bytes"].as_i64().unwrap() > 0);
    let run_id = run["id"].as_str().unwrap();

    // Dirty detection: after success, not dirty
    let pid = uuid::Uuid::parse_str(project_id).unwrap();
    let dirty = vulyk_api::services::backup::project_is_dirty(&pool, pid)
        .await
        .unwrap();
    assert!(!dirty);

    // Restore into new project
    let preview: Value = client
        .post(format!("{base}/api/v1/admin/backups/restore"))
        .headers(headers.clone())
        .json(&json!({
            "run_id": run_id,
            "mode": "new_project",
            "conflict": "skip",
            "new_project_key": format!("{key}2"),
            "dry_run": false
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(preview["applied"], true);
    assert!(preview["counts"]["cases_create"].as_i64().unwrap() >= 1);

    let projects: Value = client
        .get(format!("{base}/api/v1/projects"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let restored = projects
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["key"] == format!("{key}2"))
        .expect("restored project");
    let rid = restored["id"].as_str().unwrap();
    let cases: Value = client
        .get(format!("{base}/api/v1/projects/{rid}/cases"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let items = cases.get("items").unwrap_or(&cases);
    let found = items.as_array().unwrap().iter().any(|c| {
        c["key"] == case_key || c.get("case").and_then(|x| x.get("key")) == Some(&json!(case_key))
    });
    assert!(found, "restored case key {case_key} not found in {cases}");
}

#[tokio::test]
async fn github_destination_mock_test_connection() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let store: Arc<Mutex<Vec<(String, Vec<u8>)>>> = Arc::new(Mutex::new(Vec::new()));
    let store2 = store.clone();
    let mock = Router::new()
        .route(
            "/repos/{owner}/{repo}",
            get(|| async { Json(json!({ "full_name": "o/r" })) }),
        )
        .route(
            "/repos/{owner}/{repo}/contents/{*path}",
            put(
                |AxumState(st): AxumState<Arc<Mutex<Vec<(String, Vec<u8>)>>>>,
                 axum::extract::Path((_o, _r, path)): axum::extract::Path<(
                    String,
                    String,
                    String,
                )>,
                 body: axum::body::Bytes| async move {
                    st.lock().await.push((path, body.to_vec()));
                    (
                        StatusCode::CREATED,
                        Json(json!({ "content": { "sha": "abc" } })),
                    )
                        .into_response()
                },
            )
            .get(|| async {
                (
                    StatusCode::NOT_FOUND,
                    Json(json!({ "message": "Not Found" })),
                )
                    .into_response()
            }),
        )
        .with_state(store2);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, mock).await.ok();
    });
    tokio::time::sleep(Duration::from_millis(30)).await;
    let api_base = format!("http://{addr}");
    // Mock GitHub API listens on loopback HTTP — allow private egress for this test only.
    std::env::set_var("VULYK_EGRESS_ALLOW_PRIVATE", "1");

    let (base, pool) = spawn_app().await;
    let client = reqwest::Client::new();
    let token = ensure_admin_token(&client, &base, &pool).await;
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::AUTHORIZATION,
        format!("Bearer {token}").parse().unwrap(),
    );

    let dest: Value = client
        .post(format!("{base}/api/v1/admin/backups/destinations"))
        .headers(headers.clone())
        .json(&json!({
            "name": "GH mock",
            "kind": "github",
            "config": {
                "owner": "o",
                "repo": "r",
                "branch": "main",
                "path": "backups",
                "api_base": api_base
            },
            "secrets": { "token": "ghs_test" },
            "enabled": true
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(dest["has_secrets"], true);
    // Secrets must not appear in config
    assert!(dest["config"].get("token").is_none());

    let dest_id = dest["id"].as_str().expect("dest id");
    let test: Value = client
        .post(format!(
            "{base}/api/v1/admin/backups/destinations/{dest_id}/test"
        ))
        .headers(headers)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(test["ok"], true, "{test}");
}

#[tokio::test]
async fn local_destination_rejects_path_outside_backup_root() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let (base, pool) = spawn_app().await;
    let client = reqwest::Client::new();
    let token = ensure_admin_token(&client, &base, &pool).await;

    let allowed = tempfile_dir();
    std::env::set_var("VULYK_CONTENT_BACKUP_DIR", &allowed);
    let outside = tempfile_dir(); // sibling temp dir — must be rejected

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::AUTHORIZATION,
        format!("Bearer {token}").parse().unwrap(),
    );

    let res = client
        .post(format!("{base}/api/v1/admin/backups/destinations"))
        .headers(headers)
        .json(&json!({
            "name": "Escape hatch",
            "kind": "local",
            "config": { "path": outside },
            "enabled": true
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        reqwest::StatusCode::BAD_REQUEST,
        "outside path should be rejected: {}",
        res.text().await.unwrap_or_default()
    );
}

fn tempfile_dir() -> String {
    let dir = std::env::temp_dir().join(format!("vulyk-backup-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.to_string_lossy().into_owned()
}

#[allow(dead_code)]
fn _addr_type(_: SocketAddr) {}
