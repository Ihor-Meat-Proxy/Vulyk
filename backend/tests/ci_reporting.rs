//! CI reporting API: open → result → close, tokens, permissions, idempotency, JUnit keys.

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
            jwt_secret: "test-secret-ci-report".into(),
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
    .bind("CI Tester")
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

#[tokio::test]
async fn ci_open_result_close_and_junit_key() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }

    let (base, _pool) = spawn_app(false).await;
    let client = reqwest::Client::new();
    let key = unique_key("C");

    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&json!({ "key": key, "name": "CI Report Proj" }))
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
            "title": "Login",
            "status": "ready",
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let case_key = case
        .get("key")
        .or_else(|| case.get("case").and_then(|c| c.get("key")))
        .and_then(|v| v.as_str())
        .expect("case key")
        .to_string();

    let ext = format!("github:run:{}", uuid::Uuid::new_v4());
    let opened: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/ci/runs"))
        .json(&json!({
            "provider": "github",
            "repo": "acme/tests",
            "branch": "main",
            "commit": "abc123",
            "pipeline_url": "https://github.com/acme/tests/actions/runs/1",
            "suite_name": "Nightly",
            "external_ref": ext,
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(opened["idempotent"], false);
    assert_eq!(opened["run"]["status"], "open");
    assert_eq!(opened["run"]["lifecycle"], "ci");
    let run_id = opened["run"]["id"].as_str().unwrap();

    // Idempotent open with same external_ref
    let again: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/ci/runs"))
        .json(&json!({ "external_ref": ext, "suite_name": "Nightly" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(again["idempotent"], true);
    assert_eq!(again["run"]["id"], run_id);

    let results: Value = client
        .post(format!(
            "{base}/api/v1/projects/{project_id}/ci/runs/{run_id}/results"
        ))
        .json(&json!({
            "results": [
                {
                    "case_key": case_key,
                    "status": "passed",
                    "external_id": "spec::login",
                    "duration_ms": 12
                },
                {
                    "case_key": format!("{key}-999"),
                    "status": "failed",
                    "external_id": "spec::missing"
                }
            ]
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(results["inserted"], 1);
    assert_eq!(results["status"], "in_progress");
    let unmatched = results["unmatched_case_keys"].as_array().unwrap();
    assert!(unmatched
        .iter()
        .any(|k| k.as_str() == Some(&format!("{key}-999"))));

    // Idempotent replay of same external_id + status
    let replay: Value = client
        .post(format!(
            "{base}/api/v1/projects/{project_id}/ci/runs/{run_id}/results"
        ))
        .json(&json!({
            "results": [{
                "case_key": case_key,
                "status": "passed",
                "external_id": "spec::login"
            }]
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(replay["inserted"], 0);
    assert!(replay["idempotent_replays"].as_i64().unwrap() >= 1);

    // JUnit with test_key property
    let junit = format!(
        r#"<?xml version="1.0"?>
<testsuite name="s" tests="1">
  <testcase classname="t" name="login" time="0.05">
    <properties>
      <property name="test_key" value="{case_key}"/>
    </properties>
  </testcase>
</testsuite>"#
    );
    let ingest: Value = client
        .post(format!(
            "{base}/api/v1/projects/{project_id}/ci/runs/{run_id}/ingest"
        ))
        .json(&json!({
            "format": "junit",
            "content": junit,
            "source": "junit"
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(ingest["inserted"].as_i64().unwrap() >= 1);
    assert!(ingest["unmatched_case_keys"].as_array().unwrap().is_empty());

    let closed: Value = client
        .post(format!(
            "{base}/api/v1/projects/{project_id}/ci/runs/{run_id}/close"
        ))
        .json(&json!({}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(closed["status"], "closed");

    // Post after close → 409
    let after = client
        .post(format!(
            "{base}/api/v1/projects/{project_id}/ci/runs/{run_id}/results"
        ))
        .json(&json!({
            "results": [{ "case_key": case_key, "status": "failed" }]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(after.status(), reqwest::StatusCode::CONFLICT);
}

#[tokio::test]
async fn ci_token_auth_and_permissions() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }

    let (base, pool) = spawn_app(true).await;
    let client = reqwest::Client::new();

    let admin_email = format!("admin+{}@example.com", uuid::Uuid::new_v4().simple());
    let _admin_id = insert_user(&pool, &admin_email, "password123", true).await;
    let admin_token = login(&client, &base, &admin_email, "password123").await;

    let key = unique_key("T");
    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .header("Authorization", format!("Bearer {admin_token}"))
        .json(&json!({ "key": key, "name": "Token Proj" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let project_id = project["id"].as_str().unwrap();

    let viewer_email = format!("viewer+{}@example.com", uuid::Uuid::new_v4().simple());
    let viewer_id = insert_user(&pool, &viewer_email, "password123", false).await;
    sqlx::query(
        r#"INSERT INTO memberships (project_id, user_id, role)
           VALUES ($1, $2, 'viewer')"#,
    )
    .bind(uuid::Uuid::parse_str(project_id).unwrap())
    .bind(viewer_id)
    .execute(&pool)
    .await
    .unwrap();
    let viewer_token = login(&client, &base, &viewer_email, "password123").await;

    // Viewer cannot create CI tokens
    let forbidden = client
        .post(format!("{base}/api/v1/projects/{project_id}/ci-tokens"))
        .header("Authorization", format!("Bearer {viewer_token}"))
        .json(&json!({ "name": "nope" }))
        .send()
        .await
        .unwrap();
    assert_eq!(forbidden.status(), reqwest::StatusCode::FORBIDDEN);

    // Admin creates CI token
    let tok: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/ci-tokens"))
        .header("Authorization", format!("Bearer {admin_token}"))
        .json(&json!({ "name": "github-main" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let secret = tok["secret"].as_str().unwrap();
    assert!(secret.starts_with("vulyk_ci_"));

    // CI token can open a run
    let opened: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/ci/runs"))
        .header("Authorization", format!("Bearer {secret}"))
        .json(&json!({
            "suite_name": "token-run",
            "provider": "github",
            "external_ref": format!("tok:{}", uuid::Uuid::new_v4()),
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(opened["run"]["status"], "open");

    // CI token cannot manage tokens (ManageApiTokens)
    let manage = client
        .post(format!("{base}/api/v1/projects/{project_id}/ci-tokens"))
        .header("Authorization", format!("Bearer {secret}"))
        .json(&json!({ "name": "another" }))
        .send()
        .await
        .unwrap();
    assert_eq!(manage.status(), reqwest::StatusCode::FORBIDDEN);

    // Other project rejected
    let other: Value = client
        .post(format!("{base}/api/v1/projects"))
        .header("Authorization", format!("Bearer {admin_token}"))
        .json(&json!({ "key": unique_key("X"), "name": "Other" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let other_id = other["id"].as_str().unwrap();
    let cross = client
        .post(format!("{base}/api/v1/projects/{other_id}/ci/runs"))
        .header("Authorization", format!("Bearer {secret}"))
        .json(&json!({ "suite_name": "nope" }))
        .send()
        .await
        .unwrap();
    assert_eq!(cross.status(), reqwest::StatusCode::FORBIDDEN);
}
