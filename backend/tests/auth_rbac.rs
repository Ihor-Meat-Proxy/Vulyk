//! Auth, RBAC, API tokens, and admin settings smoke tests.

use std::time::Duration;

use serde_json::Value;
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
            jwt_secret: "test-secret-w1".into(),
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

fn unique_email(prefix: &str) -> String {
    format!("{prefix}+{}@example.com", uuid::Uuid::new_v4().simple())
}

async fn insert_user(
    pool: &sqlx::PgPool,
    email: &str,
    password: &str,
    is_admin: bool,
    must_change: bool,
) -> uuid::Uuid {
    let hash = hash_password(password).expect("hash");
    sqlx::query_scalar(
        r#"INSERT INTO users (
               email, display_name, password_hash, is_system_admin, must_change_password
           ) VALUES ($1, $2, $3, $4, $5)
           RETURNING id"#,
    )
    .bind(email)
    .bind("Test User")
    .bind(hash)
    .bind(is_admin)
    .bind(must_change)
    .fetch_one(pool)
    .await
    .expect("insert user")
}

async fn insert_admin(pool: &sqlx::PgPool, email: &str, password: &str) -> uuid::Uuid {
    insert_user(pool, email, password, true, false).await
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
        .error_for_status()
        .expect("login ok")
        .json()
        .await
        .expect("login json");
    login["token"].as_str().expect("token").to_string()
}

#[tokio::test]
async fn bootstrap_login_token_and_rbac() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }

    let (base, pool) = spawn_app(true).await;
    let client = reqwest::Client::new();

    let status: Value = client
        .get(format!("{base}/api/v1/auth/status"))
        .send()
        .await
        .expect("status")
        .json()
        .await
        .expect("status json");
    assert_eq!(status["auth_required"], true);

    let email = unique_email("admin");
    let password = "password12345678";

    let user_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&pool)
        .await
        .expect("count");

    let (token, admin_id) = if user_count == 0 {
        let body: Value = client
            .post(format!("{base}/api/v1/auth/bootstrap"))
            .json(&serde_json::json!({
                "email": email,
                "password": password,
                "display_name": "Bootstrap Admin"
            }))
            .send()
            .await
            .expect("bootstrap")
            .error_for_status()
            .expect("bootstrap status")
            .json()
            .await
            .expect("bootstrap json");
        (
            body["token"].as_str().expect("token").to_string(),
            body["user"]["id"].as_str().expect("id").to_string(),
        )
    } else {
        let id = insert_admin(&pool, &email, password).await;
        (
            login(&client, &base, &email, password).await,
            id.to_string(),
        )
    };

    let me: Value = client
        .get(format!("{base}/api/v1/auth/me"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("me")
        .error_for_status()
        .expect("me status")
        .json()
        .await
        .expect("me json");
    assert_eq!(me["email"], email);
    assert_eq!(me["is_system_admin"], true);

    let relogin = login(&client, &base, &email, password).await;
    assert!(relogin.len() > 20);

    let token_resp: Value = client
        .post(format!("{base}/api/v1/tokens"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&serde_json::json!({
            "name": "ci-mcp",
            "scopes": ["api"]
        }))
        .send()
        .await
        .expect("token")
        .error_for_status()
        .expect("token status")
        .json()
        .await
        .expect("token json");
    let api_token = token_resp["secret"].as_str().expect("api token");
    assert!(api_token.starts_with("vulyk_at_"));

    let me_via_api: Value = client
        .get(format!("{base}/api/v1/auth/me"))
        .header("Authorization", format!("Bearer {api_token}"))
        .send()
        .await
        .expect("me api")
        .error_for_status()
        .expect("me api status")
        .json()
        .await
        .expect("me api json");
    assert_eq!(me_via_api["id"], admin_id);

    let key = format!(
        "A{}",
        uuid::Uuid::new_v4().simple().to_string()[..6].to_uppercase()
    );
    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&serde_json::json!({
            "key": key,
            "name": "Auth Project",
            "description": "rbac"
        }))
        .send()
        .await
        .expect("project")
        .error_for_status()
        .expect("project status")
        .json()
        .await
        .expect("project json");
    let project_id = project["id"].as_str().expect("project id");

    let memberships: Value = client
        .get(format!("{base}/api/v1/projects/{project_id}/memberships"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("memberships")
        .error_for_status()
        .expect("memberships status")
        .json()
        .await
        .expect("memberships json");
    let arr = memberships.as_array().expect("arr");
    assert_eq!(arr.len(), 1);
    assert_eq!(arr[0]["role"], "admin");

    let engineer_email = unique_email("engineer");
    let engineer: Value = client
        .post(format!("{base}/api/v1/users"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&serde_json::json!({
            "email": engineer_email,
            "password": password,
            "display_name": "Test Automation Engineer",
            "is_system_admin": false
        }))
        .send()
        .await
        .expect("create user")
        .error_for_status()
        .expect("user status")
        .json()
        .await
        .expect("user json");
    let engineer_id = engineer["id"].as_str().expect("engineer id");

    let membership: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/memberships"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&serde_json::json!({
            "user_id": engineer_id,
            "role": "test_automation_engineer"
        }))
        .send()
        .await
        .expect("add member")
        .error_for_status()
        .expect("member status")
        .json()
        .await
        .expect("member json");
    assert_eq!(membership["role"], "test_automation_engineer");

    let integration: Value = client
        .post(format!("{base}/api/v1/admin/integrations"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&serde_json::json!({
            "kind": "openproject",
            "name": "Default OpenProject",
            "config": { "base_url": "https://op.example" },
            "enabled": false
        }))
        .send()
        .await
        .expect("integration")
        .error_for_status()
        .expect("integration status")
        .json()
        .await
        .expect("integration json");
    assert_eq!(integration["kind"], "openproject");

    let engineer_token = login(&client, &base, &engineer_email, password).await;
    let forbidden = client
        .get(format!("{base}/api/v1/users"))
        .header("Authorization", format!("Bearer {engineer_token}"))
        .send()
        .await
        .expect("engineer users");
    assert_eq!(forbidden.status().as_u16(), 403);
}

#[tokio::test]
async fn open_mode_still_lists_projects() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }

    let (base, _pool) = spawn_app(false).await;
    let client = reqwest::Client::new();
    let status: Value = client
        .get(format!("{base}/api/v1/auth/status"))
        .send()
        .await
        .expect("status")
        .json()
        .await
        .expect("json");
    assert_eq!(status["auth_required"], false);
    assert!(
        status["password_policy"]["min_length"].as_u64().unwrap_or(0) >= 12,
        "password_policy should be exposed on auth status"
    );

    let projects = client
        .get(format!("{base}/api/v1/projects"))
        .send()
        .await
        .expect("projects");
    assert!(projects.status().is_success());
}

#[tokio::test]
async fn weak_password_login_forces_change_and_blocks_api() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }

    let (base, pool) = spawn_app(true).await;
    let client = reqwest::Client::new();
    let email = unique_email("legacy");
    let weak = "legacy1"; // below policy; login must still verify the hash
    let _id = insert_user(&pool, &email, weak, true, false).await;

    let login: Value = client
        .post(format!("{base}/api/v1/auth/login"))
        .json(&serde_json::json!({ "email": email, "password": weak }))
        .send()
        .await
        .expect("login")
        .error_for_status()
        .expect("login ok")
        .json()
        .await
        .expect("login json");
    assert_eq!(login["user"]["must_change_password"], true);
    let token = login["token"].as_str().expect("token");

    // Direct API bypass of UI must still be blocked.
    let blocked = client
        .get(format!("{base}/api/v1/users"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("users");
    assert_eq!(blocked.status().as_u16(), 403);

    let me_ok = client
        .get(format!("{base}/api/v1/auth/me"))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .expect("me");
    assert!(me_ok.status().is_success());

    // Reject new password that is still too short.
    let bad_change = client
        .post(format!("{base}/api/v1/auth/change-password"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&serde_json::json!({ "new_password": "still-too-short1" }))
        .send()
        .await
        .expect("bad change");
    assert_eq!(bad_change.status().as_u16(), 400);

    let strong = "brand-new-password1";
    let changed = client
        .post(format!("{base}/api/v1/auth/change-password"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&serde_json::json!({ "new_password": strong }))
        .send()
        .await
        .expect("change");
    assert_eq!(changed.status().as_u16(), 204);

    let relogin = login(&client, &base, &email, strong).await;
    let allowed = client
        .get(format!("{base}/api/v1/users"))
        .header("Authorization", format!("Bearer {relogin}"))
        .send()
        .await
        .expect("users after change");
    assert!(allowed.status().is_success());
}

#[tokio::test]
async fn password_policy_rejects_weak_on_create() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }

    let (base, pool) = spawn_app(true).await;
    let client = reqwest::Client::new();
    let admin_email = unique_email("poladmin");
    let admin_password = "password12345678";
    let _ = insert_admin(&pool, &admin_email, admin_password).await;
    let token = login(&client, &base, &admin_email, admin_password).await;

    let weak = client
        .post(format!("{base}/api/v1/users"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&serde_json::json!({
            "email": unique_email("weak"),
            "password": "short1",
            "display_name": "Weak",
            "is_system_admin": false
        }))
        .send()
        .await
        .expect("create");
    assert_eq!(weak.status().as_u16(), 400);
}
