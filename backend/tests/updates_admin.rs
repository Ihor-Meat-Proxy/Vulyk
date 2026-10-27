//! Admin Updates endpoints (manual capability when updater env unset).

use std::time::Duration;

use serde_json::Value;
use vulyk_api::auth::hash_password;

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
            required: true,
            jwt_secret: "test-secret-updates".into(),
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

async fn insert_admin(pool: &sqlx::PgPool, email: &str, password: &str) {
    let hash = hash_password(password).expect("hash");
    sqlx::query(
        r#"INSERT INTO users (email, display_name, password_hash, is_system_admin)
           VALUES ($1, $2, $3, TRUE)"#,
    )
    .bind(email)
    .bind("Updates Admin")
    .bind(hash)
    .execute(pool)
    .await
    .expect("insert admin");
}

async fn login(client: &reqwest::Client, base: &str, email: &str, password: &str) -> String {
    let login: Value = client
        .post(format!("{base}/api/v1/auth/login"))
        .json(&serde_json::json!({
            "email": email,
            "password": password
        }))
        .send()
        .await
        .expect("login")
        .json()
        .await
        .expect("login json");
    login["token"].as_str().expect("token").to_string()
}

#[tokio::test]
async fn updates_status_requires_admin_and_reports_manual() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let (base, pool) = spawn_app().await;
    let email = format!(
        "updates-admin+{}@example.com",
        uuid::Uuid::new_v4().simple()
    );
    insert_admin(&pool, &email, "admin-password-16").await;
    let client = reqwest::Client::new();

    let anon = client
        .get(format!("{base}/api/v1/admin/updates"))
        .send()
        .await
        .expect("anon");
    assert!(
        anon.status().is_client_error(),
        "anonymous must not read updates"
    );

    let token = login(&client, &base, &email, "admin-password-16").await;
    let res = client
        .get(format!("{base}/api/v1/admin/updates"))
        .bearer_auth(&token)
        .send()
        .await
        .expect("get updates");
    assert!(
        res.status().is_success(),
        "{}",
        res.text().await.unwrap_or_default()
    );
    let body: Value = res.json().await.expect("json");
    let current = body["current_version"].as_str().unwrap_or("");
    assert!(!current.is_empty(), "current_version present");
    assert_eq!(body["capability"], "manual_only");
    assert!(
        body["manual_command"]
            .as_str()
            .unwrap_or("")
            .contains("install.sh"),
        "manual command should mention install.sh"
    );
    assert!(
        body["rollback_command"]
            .as_str()
            .unwrap_or("")
            .contains("rollback"),
        "rollback_command present"
    );
    assert!(body["version_backups"].is_array());
    assert!(body["retention"]["rollback_days"].as_i64().unwrap_or(0) >= 1);

    let apply = client
        .post(format!("{base}/api/v1/admin/updates/apply"))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "version": "0.1.0" }))
        .send()
        .await
        .expect("apply");
    assert_eq!(
        apply.status(),
        reqwest::StatusCode::BAD_REQUEST,
        "apply without sidecar must fail"
    );

    let rollback = client
        .post(format!("{base}/api/v1/admin/updates/rollback"))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "confirm": "ROLLBACK" }))
        .send()
        .await
        .expect("rollback");
    assert_eq!(
        rollback.status(),
        reqwest::StatusCode::BAD_REQUEST,
        "rollback without sidecar must fail"
    );

    let bad_confirm = client
        .post(format!("{base}/api/v1/admin/updates/rollback"))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "confirm": "yes" }))
        .send()
        .await
        .expect("bad confirm");
    assert_eq!(bad_confirm.status(), reqwest::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn updates_preferences_roundtrip() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let (base, pool) = spawn_app().await;
    let email = format!(
        "updates-prefs+{}@example.com",
        uuid::Uuid::new_v4().simple()
    );
    insert_admin(&pool, &email, "admin-password-16").await;
    let client = reqwest::Client::new();
    let token = login(&client, &base, &email, "admin-password-16").await;

    let put = client
        .put(format!("{base}/api/v1/admin/updates/preferences"))
        .bearer_auth(&token)
        .json(&serde_json::json!({ "check_interval_hours": 12 }))
        .send()
        .await
        .expect("put");
    assert!(put.status().is_success());
    let body: Value = put.json().await.expect("json");
    assert_eq!(body["check_interval_hours"], 12);

    let get = client
        .get(format!("{base}/api/v1/admin/updates/preferences"))
        .bearer_auth(&token)
        .send()
        .await
        .expect("get");
    let body: Value = get.json().await.expect("json");
    assert_eq!(body["check_interval_hours"], 12);
}
