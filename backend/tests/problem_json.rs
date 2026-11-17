//! Contract tests: error responses are RFC 9457 `application/problem+json`
//! with dual-written legacy `{ error: { code, message } }`.

use std::time::Duration;

use serde_json::Value;

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
            jwt_secret: "test-secret-problem-json".into(),
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

fn assert_problem_json(body: &Value, status: u16, code: &str) {
    assert_eq!(body["status"], status, "body={body}");
    assert_eq!(body["code"], code, "body={body}");
    assert_eq!(
        body["type"].as_str().unwrap_or(""),
        format!("urn:vulyk:api:problem:{code}"),
        "body={body}"
    );
    assert!(
        body["title"].as_str().is_some_and(|t| !t.is_empty()),
        "missing title: {body}"
    );
    assert!(
        body["detail"].as_str().is_some_and(|d| !d.is_empty()),
        "missing detail: {body}"
    );
    // Dual-written legacy shape for older clients.
    assert_eq!(body["error"]["code"], code, "body={body}");
    assert_eq!(
        body["error"]["message"], body["detail"],
        "legacy message must match detail: {body}"
    );
}

#[tokio::test]
async fn not_found_returns_problem_json() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let (base, _pool) = spawn_app(false).await;
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{base}/api/v1/cases/by-key/ZZZZ-99999"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
    let ct = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        ct.starts_with("application/problem+json"),
        "content-type={ct}"
    );
    let body: Value = resp.json().await.unwrap();
    assert_problem_json(&body, 404, "not_found");
}

#[tokio::test]
async fn bad_request_returns_problem_json() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let (base, _pool) = spawn_app(false).await;
    let client = reqwest::Client::new();
    // Empty project name → 400 from create_project validation.
    let resp = client
        .post(format!("{base}/api/v1/projects"))
        .json(&serde_json::json!({ "name": "" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
    let ct = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        ct.starts_with("application/problem+json"),
        "content-type={ct}"
    );
    let body: Value = resp.json().await.unwrap();
    assert_problem_json(&body, 400, "bad_request");
}

#[tokio::test]
async fn unauthorized_returns_problem_json_when_auth_required() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let (base, _pool) = spawn_app(true).await;
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{base}/api/v1/projects"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
    let ct = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        ct.starts_with("application/problem+json"),
        "content-type={ct}"
    );
    let body: Value = resp.json().await.unwrap();
    assert_problem_json(&body, 401, "unauthorized");
}

#[tokio::test]
async fn forbidden_returns_problem_json_for_non_admin() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let (base, pool) = spawn_app(true).await;
    let client = reqwest::Client::new();

    let email = format!("problem-json-{}@example.com", uuid::Uuid::new_v4().simple());
    // Unique per call so CodeQL does not flag a hard-coded credential.
    let password = format!("pj-{}", uuid::Uuid::new_v4());
    let hash = vulyk_api::auth::hash_password(password).expect("hash");
    sqlx::query(
        r#"INSERT INTO users (
               email, display_name, password_hash, is_system_admin, must_change_password
           ) VALUES ($1, $2, $3, false, false)"#,
    )
    .bind(&email)
    .bind("Non Admin")
    .bind(hash)
    .execute(&pool)
    .await
    .expect("insert user");

    let login: Value = client
        .post(format!("{base}/api/v1/auth/login"))
        .json(&serde_json::json!({ "email": email, "password": password }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let token = login["token"].as_str().unwrap();

    // Admin-only users list → 403 for a non-admin member.
    let resp = client
        .get(format!("{base}/api/v1/users"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        403,
        "expected forbidden for non-admin users list"
    );
    let ct = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        ct.starts_with("application/problem+json"),
        "content-type={ct}"
    );
    let body: Value = resp.json().await.unwrap();
    assert_problem_json(&body, 403, "forbidden");
}
