//! Admin sample-data remove / re-seed: permissions and is_sample isolation.

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
            jwt_secret: "test-secret-sample".into(),
            jwt_ttl_hours: 12,
            session_ttl_hours: 24,
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
    (format!("http://{addr}"), pool)
}

fn unique_email(prefix: &str) -> String {
    format!("{prefix}+{}@example.com", uuid::Uuid::new_v4().simple())
}

async fn insert_admin(pool: &sqlx::PgPool, email: &str, password: &str) -> uuid::Uuid {
    let hash = hash_password(password).expect("hash");
    sqlx::query_scalar(
        r#"INSERT INTO users (email, display_name, password_hash, is_system_admin)
           VALUES ($1, $2, $3, TRUE)
           RETURNING id"#,
    )
    .bind(email)
    .bind("Sample Admin")
    .bind(hash)
    .fetch_one(pool)
    .await
    .expect("insert admin")
}

async fn insert_user(pool: &sqlx::PgPool, email: &str, password: &str) -> uuid::Uuid {
    let hash = hash_password(password).expect("hash");
    sqlx::query_scalar(
        r#"INSERT INTO users (email, display_name, password_hash, is_system_admin)
           VALUES ($1, $2, $3, FALSE)
           RETURNING id"#,
    )
    .bind(email)
    .bind("Regular User")
    .bind(hash)
    .fetch_one(pool)
    .await
    .expect("insert user")
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
async fn sample_data_remove_only_touches_sample_rows_and_requires_admin() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }

    let (base, pool) = spawn_app(true).await;
    let client = reqwest::Client::new();

    let admin_email = unique_email("sample-admin");
    let user_email = unique_email("sample-user");
    let password = "password123";
    insert_admin(&pool, &admin_email, password).await;
    insert_user(&pool, &user_email, password).await;
    let admin_token = login(&client, &base, &admin_email, password).await;
    let user_token = login(&client, &base, &user_email, password).await;

    // User-created (non-sample) project
    let user_project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .bearer_auth(&admin_token)
        .json(&serde_json::json!({
            "name": "Real User Project",
            "key": format!("REAL{}", &uuid::Uuid::new_v4().simple().to_string()[..6].to_uppercase())
        }))
        .send()
        .await
        .expect("create project")
        .error_for_status()
        .expect("create ok")
        .json()
        .await
        .expect("json");
    let user_project_id = user_project["id"].as_str().expect("id").to_string();
    assert_eq!(user_project["is_sample"], false);

    // Seed samples (admin forced path)
    let reseed = client
        .post(format!("{base}/api/v1/admin/sample-data/reseed"))
        .bearer_auth(&admin_token)
        .json(&serde_json::json!({ "locale": "en" }))
        .send()
        .await
        .expect("reseed");
    assert_eq!(reseed.status(), 200, "reseed should succeed for admin");
    let status: Value = reseed.json().await.expect("reseed json");
    assert_eq!(status["present"], true);
    let keys: Vec<&str> = status["projects"]
        .as_array()
        .expect("projects")
        .iter()
        .filter_map(|p| p["key"].as_str())
        .collect();
    assert!(keys.contains(&"KYIV_CONCERT_2026"));
    assert!(keys.contains(&"VULYK_QA"));

    // Non-admin cannot remove
    let forbidden = client
        .delete(format!("{base}/api/v1/admin/sample-data"))
        .bearer_auth(&user_token)
        .send()
        .await
        .expect("delete as user");
    assert_eq!(forbidden.status(), 403);

    // Admin removes — only sample rows
    let remove_resp = client
        .delete(format!("{base}/api/v1/admin/sample-data"))
        .bearer_auth(&admin_token)
        .send()
        .await
        .expect("delete");
    let remove_status = remove_resp.status();
    let remove_body = remove_resp.text().await.expect("body");
    assert_eq!(
        remove_status, 200,
        "remove sample data failed: {remove_body}"
    );
    let removed: Value = serde_json::from_str(&remove_body).expect("json");
    assert!(removed["projects"].as_i64().unwrap_or(0) >= 2);

    let sample_left: i64 =
        sqlx::query_scalar("SELECT COUNT(*)::bigint FROM projects WHERE is_sample = TRUE")
            .fetch_one(&pool)
            .await
            .expect("count sample");
    assert_eq!(sample_left, 0);

    let user_still: i64 = sqlx::query_scalar("SELECT COUNT(*)::bigint FROM projects WHERE id = $1")
        .bind(uuid::Uuid::parse_str(&user_project_id).unwrap())
        .fetch_one(&pool)
        .await
        .expect("user project");
    assert_eq!(
        user_still, 1,
        "user-created project must survive sample remove"
    );

    // Idempotent second remove
    let again: Value = client
        .delete(format!("{base}/api/v1/admin/sample-data"))
        .bearer_auth(&admin_token)
        .send()
        .await
        .expect("delete2")
        .error_for_status()
        .expect("ok")
        .json()
        .await
        .expect("json");
    assert_eq!(again["projects"], 0);

    // Reseed restores samples
    let restored: Value = client
        .post(format!("{base}/api/v1/admin/sample-data/reseed"))
        .bearer_auth(&admin_token)
        .json(&serde_json::json!({}))
        .send()
        .await
        .expect("reseed2")
        .error_for_status()
        .expect("ok")
        .json()
        .await
        .expect("json");
    assert_eq!(restored["present"], true);

    // Varied nightly naming
    let nightly: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)::bigint FROM test_runs
           WHERE is_sample = TRUE AND name LIKE 'Sample nightly #%'"#,
    )
    .fetch_one(&pool)
    .await
    .expect("nightly");
    assert!(
        nightly >= 10,
        "expected sample nightly suites, got {nightly}"
    );

    let sample_ci: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)::bigint FROM test_runs
           WHERE is_sample = TRUE AND created_by_name = 'Sample CI'"#,
    )
    .fetch_one(&pool)
    .await
    .expect("sample-ci");
    assert!(
        sample_ci >= 10,
        "expected Sample CI creator on sample nightlies, got {sample_ci}"
    );

    let wall_e_sample: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)::bigint FROM test_runs
           WHERE is_sample = TRUE AND created_by_name = 'WALL-E-rror'"#,
    )
    .fetch_one(&pool)
    .await
    .expect("wall-e");
    assert_eq!(
        wall_e_sample, 0,
        "legacy WALL-E-rror creator should not appear on is_sample runs"
    );
}
