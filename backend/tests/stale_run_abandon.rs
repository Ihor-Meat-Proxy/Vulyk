//! Stale in_progress auto-cancel + admin abandon endpoint.

use std::time::Duration;

use serde_json::{json, Value};
use vulyk_api::auth::hash_password;

async fn spawn_app(auth_required: bool) -> (String, sqlx::PgPool) {
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
            required: auth_required,
            jwt_secret: "test-secret-stale-run".into(),
            jwt_ttl_hours: 12,
            session_ttl_hours: 24,
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

fn unique_key(prefix: &str) -> String {
    format!(
        "{prefix}{}",
        uuid::Uuid::new_v4().simple().to_string()[..6].to_uppercase()
    )
}

async fn insert_user(
    pool: &sqlx::PgPool,
    email: &str,
    password: &str,
    is_admin: bool,
) -> uuid::Uuid {
    let hash = hash_password(password).expect("hash");
    sqlx::query_scalar(
        r#"INSERT INTO users (email, display_name, password_hash, is_system_admin)
           VALUES ($1, $2, $3, $4)
           RETURNING id"#,
    )
    .bind(email)
    .bind("Stale Run Tester")
    .bind(hash)
    .bind(is_admin)
    .fetch_one(pool)
    .await
    .expect("insert user")
}

async fn login(client: &reqwest::Client, base: &str, email: &str, password: &str) -> String {
    let body: Value = client
        .post(format!("{base}/api/v1/auth/login"))
        .json(&json!({ "email": email, "password": password }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    body["token"].as_str().unwrap().to_string()
}

async fn create_project(client: &reqwest::Client, base: &str, token: &str) -> (String, uuid::Uuid) {
    let key = unique_key("SR");
    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({ "key": key, "name": format!("Stale {key}") }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = uuid::Uuid::parse_str(project["id"].as_str().unwrap()).unwrap();
    (key, id)
}

async fn open_in_progress_run(
    client: &reqwest::Client,
    base: &str,
    token: &str,
    project_id: uuid::Uuid,
) -> uuid::Uuid {
    let opened: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/ci/runs"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&json!({
            "provider": "other",
            "suite_name": "stale-suite",
            "name": "stale-suite",
            "external_ref": format!("stale-{}", uuid::Uuid::new_v4()),
            "start": true,
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(opened["run"]["status"], "in_progress");
    uuid::Uuid::parse_str(opened["run"]["id"].as_str().unwrap()).unwrap()
}

#[tokio::test]
async fn stale_in_progress_run_is_auto_cancelled() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }

    // Short TTL for the test; restore after.
    let prev = std::env::var("VULYK_STALE_RUN_TTL_HOURS").ok();
    std::env::set_var("VULYK_STALE_RUN_TTL_HOURS", "1");

    let (base, pool) = spawn_app(true).await;
    let client = reqwest::Client::new();
    let admin_email = format!("admin+{}@example.com", uuid::Uuid::new_v4().simple());
    let _ = insert_user(&pool, &admin_email, "password12345678", true).await;
    let admin_token = login(&client, &base, &admin_email, "password12345678").await;
    let (_key, project_id) = create_project(&client, &base, &admin_token).await;
    let run_id = open_in_progress_run(&client, &base, &admin_token, project_id).await;

    // Backdate updated_at past the 1-hour TTL.
    sqlx::query(
        r#"UPDATE test_runs
           SET updated_at = NOW() - INTERVAL '2 hours',
               started_at = NOW() - INTERVAL '2 hours'
           WHERE id = $1"#,
    )
    .bind(run_id)
    .execute(&pool)
    .await
    .expect("backdate run");

    let cancelled = vulyk_api::jobs::expire_stale_in_progress_runs(&pool)
        .await
        .expect("expire");
    assert!(cancelled >= 1, "expected at least one stale run cancelled");

    let status: String = sqlx::query_scalar("SELECT status FROM test_runs WHERE id = $1")
        .bind(run_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "cancelled");

    let meta: Value = sqlx::query_scalar("SELECT meta FROM test_runs WHERE id = $1")
        .bind(run_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(meta["cancel_reason"], "stale_ttl");
    assert_eq!(meta["cancelled"], true);

    match prev {
        Some(v) => std::env::set_var("VULYK_STALE_RUN_TTL_HOURS", v),
        None => std::env::remove_var("VULYK_STALE_RUN_TTL_HOURS"),
    }
}

#[tokio::test]
async fn admin_can_abandon_in_progress_run() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }

    let (base, pool) = spawn_app(true).await;
    let client = reqwest::Client::new();
    let admin_email = format!("admin+{}@example.com", uuid::Uuid::new_v4().simple());
    let _ = insert_user(&pool, &admin_email, "password12345678", true).await;
    let admin_token = login(&client, &base, &admin_email, "password12345678").await;
    let (_key, project_id) = create_project(&client, &base, &admin_token).await;
    let run_id = open_in_progress_run(&client, &base, &admin_token, project_id).await;

    let abandoned: Value = client
        .post(format!(
            "{base}/api/v1/projects/{project_id}/runs/{run_id}/abandon"
        ))
        .header("Authorization", format!("Bearer {admin_token}"))
        .json(&json!({ "message": "reporter died" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(abandoned["status"], "cancelled");
    assert_eq!(abandoned["meta"]["cancel_reason"], "admin_abandon");
    assert_eq!(abandoned["meta"]["cancel_message"], "reporter died");
    assert_eq!(abandoned["meta"]["cancelled"], true);
}

#[tokio::test]
async fn cannot_abandon_already_closed_run() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }

    let (base, pool) = spawn_app(true).await;
    let client = reqwest::Client::new();
    let admin_email = format!("admin+{}@example.com", uuid::Uuid::new_v4().simple());
    let _ = insert_user(&pool, &admin_email, "password12345678", true).await;
    let admin_token = login(&client, &base, &admin_email, "password12345678").await;
    let (_key, project_id) = create_project(&client, &base, &admin_token).await;
    let run_id = open_in_progress_run(&client, &base, &admin_token, project_id).await;

    // Close via CI close endpoint.
    let _ = client
        .post(format!(
            "{base}/api/v1/projects/{project_id}/ci/runs/{run_id}/close"
        ))
        .header("Authorization", format!("Bearer {admin_token}"))
        .json(&json!({ "message": "done" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let resp = client
        .post(format!(
            "{base}/api/v1/projects/{project_id}/runs/{run_id}/abandon"
        ))
        .header("Authorization", format!("Bearer {admin_token}"))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), reqwest::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn viewer_cannot_abandon_run() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }

    let (base, pool) = spawn_app(true).await;
    let client = reqwest::Client::new();
    let admin_email = format!("admin+{}@example.com", uuid::Uuid::new_v4().simple());
    let _ = insert_user(&pool, &admin_email, "password12345678", true).await;
    let admin_token = login(&client, &base, &admin_email, "password12345678").await;
    let (_key, project_id) = create_project(&client, &base, &admin_token).await;
    let run_id = open_in_progress_run(&client, &base, &admin_token, project_id).await;

    let viewer_email = format!("viewer+{}@example.com", uuid::Uuid::new_v4().simple());
    let viewer_id = insert_user(&pool, &viewer_email, "password12345678", false).await;
    sqlx::query(
        r#"INSERT INTO memberships (project_id, user_id, role)
           VALUES ($1, $2, 'viewer')"#,
    )
    .bind(project_id)
    .bind(viewer_id)
    .execute(&pool)
    .await
    .unwrap();
    let viewer_token = login(&client, &base, &viewer_email, "password12345678").await;

    let resp = client
        .post(format!(
            "{base}/api/v1/projects/{project_id}/runs/{run_id}/abandon"
        ))
        .header("Authorization", format!("Bearer {viewer_token}"))
        .json(&json!({ "message": "nope" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), reqwest::StatusCode::FORBIDDEN);

    // Unauthenticated denied when auth is required.
    let unauth = client
        .post(format!(
            "{base}/api/v1/projects/{project_id}/runs/{run_id}/abandon"
        ))
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert!(
        unauth.status() == reqwest::StatusCode::UNAUTHORIZED
            || unauth.status() == reqwest::StatusCode::FORBIDDEN,
        "expected 401/403, got {}",
        unauth.status()
    );
}
