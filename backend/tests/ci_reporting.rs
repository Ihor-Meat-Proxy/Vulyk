//! CI reporting API: lifecycle, tokens, JUnit mapping, unmatched keys.

use std::time::Duration;

use serde_json::{json, Value};

async fn spawn_app() -> String {
    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for integration tests");
    let pool = vulyk_api::db::connect(&database_url)
        .await
        .expect("connect db");
    vulyk_api::db::migrate(&pool).await.expect("migrate");
    vulyk_api::jobs::spawn_worker(pool.clone());

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

#[tokio::test]
async fn ci_lifecycle_open_result_close_and_unmatched_keys() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }

    let base = spawn_app().await;
    let client = reqwest::Client::new();

    let key = format!(
        "C{}",
        &uuid::Uuid::new_v4().simple().to_string()[..6].to_uppercase()
    );
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

    let case_key = format!("{key}-1");
    let _case: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/cases"))
        .json(&json!({
            "key": case_key,
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

    // CI token create
    let tok: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/ci-tokens"))
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

    let ext = format!("github:test:{}", uuid::Uuid::new_v4());
    let open: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/ci/runs"))
        .header("Authorization", format!("Bearer {secret}"))
        .json(&json!({
            "suite_name": "Playwright #1",
            "provider": "github",
            "repo": "acme/app",
            "branch": "main",
            "commit": "abc123",
            "pipeline_url": "https://github.com/acme/app/actions/runs/1",
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
    let run = open.get("run").unwrap_or(&open);
    assert_eq!(run["status"], "open");
    assert_eq!(run["lifecycle"], "ci");
    let run_id = run["id"].as_str().unwrap();

    // Idempotent reopen
    let open2: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/ci/runs"))
        .header("Authorization", format!("Bearer {secret}"))
        .json(&json!({
            "suite_name": "Playwright #1",
            "provider": "github",
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
    assert_eq!(open2["idempotent"], true);
    assert_eq!(open2["run"]["id"], run_id);

    let junit = format!(
        r#"<?xml version="1.0"?>
<testsuite name="s" tests="2">
  <testcase classname="t" name="ok" time="0.01">
    <properties><property name="test_key" value="{case_key}"/></properties>
  </testcase>
  <testcase classname="t" name="missing" time="0.01">
    <properties><property name="test_key" value="{key}-999"/></properties>
  </testcase>
</testsuite>"#
    );

    let ingest: Value = client
        .post(format!(
            "{base}/api/v1/projects/{project_id}/ci/runs/{run_id}/ingest"
        ))
        .header("Authorization", format!("Bearer {secret}"))
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
    let unmatched = ingest["unmatched_case_keys"].as_array().unwrap();
    assert!(unmatched.iter().any(|k| k.as_str() == Some(&format!("{key}-999"))));

    let mid: Value = client
        .get(format!(
            "{base}/api/v1/projects/{project_id}/ci/runs/{run_id}"
        ))
        .header("Authorization", format!("Bearer {secret}"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(mid["status"], "in_progress");

    let closed: Value = client
        .post(format!(
            "{base}/api/v1/projects/{project_id}/ci/runs/{run_id}/close"
        ))
        .header("Authorization", format!("Bearer {secret}"))
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
        .header("Authorization", format!("Bearer {secret}"))
        .json(&json!({
            "results": [{ "case_key": case_key, "status": "passed" }]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(after.status(), 409);
}

#[tokio::test]
async fn multi_repo_attach_to_projects() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }

    let base = spawn_app().await;
    let client = reqwest::Client::new();

    let key_a = format!(
        "A{}",
        &uuid::Uuid::new_v4().simple().to_string()[..5].to_uppercase()
    );
    let key_b = format!(
        "B{}",
        &uuid::Uuid::new_v4().simple().to_string()[..5].to_uppercase()
    );
    let pa: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&json!({ "key": key_a, "name": "Proj A" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let pb: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&json!({ "key": key_b, "name": "Proj B" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let id_a = pa["id"].as_str().unwrap();
    let id_b = pb["id"].as_str().unwrap();

    let repo: Value = client
        .post(format!("{base}/api/v1/automation-repos"))
        .json(&json!({
            "name": "Playwright suite",
            "provider": "github",
            "remote_url": "https://github.com/acme/e2e.git",
            "branch": "main",
            "project_ids": [id_a, id_b],
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(repo["name"], "Playwright suite");
    let links = repo["project_ids"].as_array().unwrap();
    assert_eq!(links.len(), 2);

    let for_a: Value = client
        .get(format!("{base}/api/v1/projects/{id_a}/automation-repos"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(!for_a.as_array().unwrap().is_empty());
}
