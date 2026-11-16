//! Admin Updates endpoints (manual capability when updater env unset).

use std::time::Duration;

use chrono::Utc;
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

/// Unique per call so CodeQL does not flag a hard-coded credential.
fn test_password() -> String {
    format!("upd-{}", uuid::Uuid::new_v4())
}

/// Serialise tests that mutate shared `updates_*` system_settings keys.
async fn updates_settings_lock() -> tokio::sync::MutexGuard<'static, ()> {
    static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    LOCK.lock().await
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
    let password = test_password();
    insert_admin(&pool, &email, &password).await;
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

    let token = login(&client, &base, &email, &password).await;
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
async fn updates_status_exposes_persisted_check_error_and_keeps_last_good() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let _guard = updates_settings_lock().await;
    let (base, pool) = spawn_app().await;
    let email = format!("updates-err+{}@example.com", uuid::Uuid::new_v4().simple());
    let password = test_password();
    insert_admin(&pool, &email, &password).await;
    let client = reqwest::Client::new();
    let token = login(&client, &base, &email, &password).await;

    // Seed a last-good release cache + a durable check error.
    for (key, value) in [
        ("updates_latest_version", "1.2.3"),
        ("updates_latest_notes", "cached notes"),
        ("updates_latest_url", "https://example.com/v1.2.3"),
        ("updates_last_checked_at", "2024-01-01T00:00:00Z"),
        ("updates_last_check_error_code", "git_host_rate_limit"),
        ("updates_last_check_error_at", "2024-06-01T12:00:00Z"),
        ("updates_last_check_error_reset_at", "2024-06-01T13:00:00Z"),
    ] {
        vulyk_api::settings::set_value(&pool, key, serde_json::Value::String(value.into()))
            .await
            .expect("set setting");
    }

    let res = client
        .get(format!("{base}/api/v1/admin/updates"))
        .bearer_auth(&token)
        .send()
        .await
        .expect("get updates");
    assert!(res.status().is_success());
    let body: Value = res.json().await.expect("json");
    assert_eq!(body["latest_version"], "1.2.3");
    assert_eq!(body["last_checked_at"], "2024-01-01T00:00:00Z");
    assert_eq!(body["last_check_error_code"], "git_host_rate_limit");
    assert_eq!(body["last_check_error_at"], "2024-06-01T12:00:00Z");
    assert_eq!(body["last_check_error_reset_at"], "2024-06-01T13:00:00Z");
}

#[tokio::test]
async fn failed_check_records_attempt_time_keeps_last_good() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let _guard = updates_settings_lock().await;
    let (_base, pool) = spawn_app().await;
    for (key, value) in [
        ("updates_latest_version", "1.2.3"),
        ("updates_latest_notes", "cached notes"),
        ("updates_latest_url", "https://example.com/v1.2.3"),
        ("updates_last_checked_at", ""),
        ("updates_last_check_error_code", ""),
        ("updates_last_check_error_at", ""),
        ("updates_last_check_error_reset_at", ""),
    ] {
        vulyk_api::settings::set_value(&pool, key, serde_json::Value::String(value.into()))
            .await
            .expect("set");
    }

    let before = Utc::now() - chrono::Duration::seconds(2);
    let reset = Utc::now() + chrono::Duration::hours(1);
    vulyk_api::routes::updates::apply_check_outcome(
        &pool,
        Err((
            vulyk_api::routes::updates::CHECK_ERR_RATE_LIMIT,
            Some(reset),
        )),
    )
    .await
    .expect("apply failure outcome");

    let last = vulyk_api::settings::get_string(&pool, "updates_last_checked_at", "")
        .await
        .expect("last");
    assert!(!last.is_empty(), "failure must record attempt time");
    let parsed = chrono::DateTime::parse_from_rfc3339(&last).expect("rfc3339");
    assert!(parsed >= before);
    assert_eq!(
        vulyk_api::settings::get_string(&pool, "updates_latest_version", "")
            .await
            .unwrap(),
        "1.2.3"
    );
    assert_eq!(
        vulyk_api::settings::get_string(&pool, "updates_last_check_error_code", "")
            .await
            .unwrap(),
        "git_host_rate_limit"
    );
    let stored_reset =
        vulyk_api::settings::get_string(&pool, "updates_last_check_error_reset_at", "")
            .await
            .unwrap();
    assert_eq!(stored_reset, reset.to_rfc3339());
}

#[tokio::test]
async fn auto_check_backoff_skips_until_rate_limit_reset() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let _guard = updates_settings_lock().await;
    let (base, pool) = spawn_app().await;
    let email = format!("updates-skip+{}@example.com", uuid::Uuid::new_v4().simple());
    let password = test_password();
    insert_admin(&pool, &email, &password).await;
    let client = reqwest::Client::new();
    let token = login(&client, &base, &email, &password).await;

    let attempt = Utc::now().to_rfc3339();
    let reset = (Utc::now() + chrono::Duration::hours(2)).to_rfc3339();
    for (key, value) in [
        ("updates_latest_version", "2.0.0"),
        ("updates_last_checked_at", attempt.as_str()),
        ("updates_last_check_error_code", "git_host_rate_limit"),
        ("updates_last_check_error_at", attempt.as_str()),
        ("updates_last_check_error_reset_at", reset.as_str()),
    ] {
        vulyk_api::settings::set_value(&pool, key, serde_json::Value::String(value.into()))
            .await
            .expect("set");
    }

    // Auto-check (no force) must return cached status without advancing attempt time.
    let res = client
        .post(format!("{base}/api/v1/admin/updates/check"))
        .bearer_auth(&token)
        .send()
        .await
        .expect("check");
    assert!(
        res.status().is_success(),
        "{}",
        res.text().await.unwrap_or_default()
    );
    let body: Value = res.json().await.expect("json");
    assert_eq!(body["last_checked_at"], attempt);
    assert_eq!(body["last_check_error_code"], "git_host_rate_limit");
    assert_eq!(body["last_check_error_reset_at"], reset);
    assert_eq!(body["latest_version"], "2.0.0");
}

#[tokio::test]
async fn successful_check_clears_last_check_error() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let _guard = updates_settings_lock().await;
    let (_base, pool) = spawn_app().await;
    for (key, value) in [
        ("updates_latest_version", "0.1.0"),
        ("updates_last_check_error_code", "git_host_rate_limit"),
        ("updates_last_check_error_at", "2024-06-01T12:00:00Z"),
        ("updates_last_check_error_reset_at", "2024-06-01T13:00:00Z"),
        ("updates_last_checked_at", "2024-06-01T12:00:00Z"),
    ] {
        vulyk_api::settings::set_value(&pool, key, serde_json::Value::String(value.into()))
            .await
            .expect("set");
    }

    vulyk_api::routes::updates::apply_check_outcome(
        &pool,
        Ok(Some(vulyk_api::routes::updates::GithubReleaseInfo {
            tag: "3.4.5".into(),
            notes: "fresh".into(),
            html_url: "https://example.com/v3.4.5".into(),
        })),
    )
    .await
    .expect("apply success");

    assert_eq!(
        vulyk_api::settings::get_string(&pool, "updates_last_check_error_code", "")
            .await
            .unwrap(),
        ""
    );
    assert_eq!(
        vulyk_api::settings::get_string(&pool, "updates_last_check_error_at", "")
            .await
            .unwrap(),
        ""
    );
    assert_eq!(
        vulyk_api::settings::get_string(&pool, "updates_last_check_error_reset_at", "")
            .await
            .unwrap(),
        ""
    );
    assert_eq!(
        vulyk_api::settings::get_string(&pool, "updates_latest_version", "")
            .await
            .unwrap(),
        "3.4.5"
    );
    let last = vulyk_api::settings::get_string(&pool, "updates_last_checked_at", "")
        .await
        .unwrap();
    assert_ne!(last, "2024-06-01T12:00:00Z");
    assert!(!last.is_empty());
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
    let password = test_password();
    insert_admin(&pool, &email, &password).await;
    let client = reqwest::Client::new();
    let token = login(&client, &base, &email, &password).await;

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
