//! Per-project CI unmatched policy: off / create_passed / create_all / fail_unmatched + dry_run.

use std::time::Duration;

use serde_json::{json, Value};

async fn spawn_app() -> String {
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

async fn create_project(client: &reqwest::Client, base: &str, policy: &str) -> (String, String) {
    let key = format!(
        "P{}",
        uuid::Uuid::new_v4().simple().to_string()[..6].to_uppercase()
    );
    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&json!({
            "key": key,
            "name": format!("Policy {policy}"),
            "ci_unmatched_policy": policy
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(project["ci_unmatched_policy"].as_str(), Some(policy));
    (project["id"].as_str().unwrap().to_string(), key)
}

async fn create_run(client: &reqwest::Client, base: &str, project_id: &str) -> String {
    let run: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/runs"))
        .json(&json!({ "name": "policy-run", "source": "ci" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    run.get("id")
        .or_else(|| run.get("run").and_then(|r| r.get("id")))
        .and_then(|v| v.as_str())
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn policy_off_lists_unmatched_without_creating() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let base = spawn_app().await;
    let client = reqwest::Client::new();
    let (project_id, _) = create_project(&client, &base, "off").await;
    let run_id = create_run(&client, &base, &project_id).await;

    let bulk: Value = client
        .post(format!("{base}/api/v1/runs/{run_id}/results:bulk"))
        .json(&json!({
            "results": [{
                "automation_id": "tests.orphan::passed",
                "title": "orphan passed",
                "status": "passed",
                "external_id": "tests.orphan::passed"
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

    assert_eq!(bulk["inserted"], 0);
    assert!(bulk["auto_created_cases"]
        .as_array()
        .map(|a| a.is_empty())
        .unwrap_or(true));
    let unmatched = bulk["unmatched_case_keys"].as_array().unwrap();
    assert_eq!(unmatched.len(), 1);
    assert_eq!(unmatched[0], "TESTS.ORPHAN::PASSED");

    let cases: Value = client
        .get(format!("{base}/api/v1/projects/{project_id}/cases"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let items = cases["items"].as_array().unwrap();
    assert!(items.is_empty(), "off must not create cases");
}

#[tokio::test]
async fn policy_create_passed_creates_draft_for_passed_only() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let base = spawn_app().await;
    let client = reqwest::Client::new();
    let (project_id, _) = create_project(&client, &base, "create_passed").await;
    let run_id = create_run(&client, &base, &project_id).await;

    let bulk: Value = client
        .post(format!("{base}/api/v1/runs/{run_id}/results:bulk"))
        .json(&json!({
            "results": [
                {
                    "automation_id": "tests.auto::ok",
                    "title": "auto ok",
                    "status": "passed",
                    "external_id": "tests.auto::ok"
                },
                {
                    "automation_id": "tests.auto::bad",
                    "title": "auto bad",
                    "status": "failed",
                    "external_id": "tests.auto::bad"
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

    assert_eq!(bulk["inserted"], 1);
    let created = bulk["auto_created_cases"].as_array().unwrap();
    assert_eq!(created.len(), 1);
    assert_eq!(created[0]["matched_by"], "TESTS.AUTO::OK");
    assert_eq!(created[0]["title"], "auto ok");
    assert!(created[0]["case_id"].as_str().is_some());
    assert!(created[0]["key"].as_str().is_some());

    let unmatched = bulk["unmatched_case_keys"].as_array().unwrap();
    assert_eq!(unmatched.len(), 1);
    assert_eq!(unmatched[0], "TESTS.AUTO::BAD");

    let case_id = created[0]["case_id"].as_str().unwrap();
    let detail: Value = client
        .get(format!("{base}/api/v1/projects/{project_id}/cases/{case_id}"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(detail["case"]["status"], "draft");
    assert_eq!(
        detail["case"]["automation_id"].as_str(),
        Some("TESTS.AUTO::OK")
    );
}

#[tokio::test]
async fn policy_create_all_creates_for_failed() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let base = spawn_app().await;
    let client = reqwest::Client::new();
    let (project_id, _) = create_project(&client, &base, "create_all").await;
    let run_id = create_run(&client, &base, &project_id).await;

    let bulk: Value = client
        .post(format!("{base}/api/v1/runs/{run_id}/results:bulk"))
        .json(&json!({
            "results": [{
                "automation_id": "tests.auto::fail",
                "title": "auto fail",
                "status": "failed",
                "external_id": "tests.auto::fail"
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

    assert_eq!(bulk["inserted"], 1);
    assert_eq!(bulk["auto_created_cases"].as_array().unwrap().len(), 1);
    assert!(bulk["unmatched_case_keys"]
        .as_array()
        .map(|a| a.is_empty())
        .unwrap_or(false));
}

#[tokio::test]
async fn policy_fail_unmatched_returns_400() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let base = spawn_app().await;
    let client = reqwest::Client::new();
    let (project_id, _) = create_project(&client, &base, "fail_unmatched").await;
    let run_id = create_run(&client, &base, &project_id).await;

    let resp = client
        .post(format!("{base}/api/v1/runs/{run_id}/results:bulk"))
        .json(&json!({
            "results": [{
                "automation_id": "tests.missing::x",
                "title": "missing",
                "status": "passed",
                "external_id": "tests.missing::x"
            }]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
    let body: Value = resp.json().await.unwrap();
    let msg = body["error"]["message"].as_str().unwrap_or("");
    assert!(
        msg.contains("fail_unmatched") || msg.contains("unmatched"),
        "unexpected error body: {body}"
    );
}

#[tokio::test]
async fn dry_run_reports_without_persisting_creates() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let base = spawn_app().await;
    let client = reqwest::Client::new();
    let (project_id, _) = create_project(&client, &base, "create_passed").await;
    let run_id = create_run(&client, &base, &project_id).await;

    let bulk: Value = client
        .post(format!("{base}/api/v1/runs/{run_id}/results:bulk"))
        .json(&json!({
            "dry_run": true,
            "results": [{
                "automation_id": "tests.dry::run",
                "title": "dry preview",
                "status": "passed",
                "external_id": "tests.dry::run"
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

    assert_eq!(bulk["dry_run"], true);
    assert_eq!(bulk["inserted"], 0);
    let created = bulk["auto_created_cases"].as_array().unwrap();
    assert_eq!(created.len(), 1);
    assert!(created[0]["case_id"].is_null());
    assert!(created[0]["key"].is_null());
    assert_eq!(created[0]["title"], "dry preview");
    assert_eq!(created[0]["matched_by"], "TESTS.DRY::RUN");

    let cases: Value = client
        .get(format!("{base}/api/v1/projects/{project_id}/cases"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        cases["items"].as_array().unwrap().is_empty(),
        "dry_run must not persist cases"
    );
}

#[tokio::test]
async fn update_project_ci_unmatched_policy() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let base = spawn_app().await;
    let client = reqwest::Client::new();
    let (project_id, _) = create_project(&client, &base, "off").await;

    let updated: Value = client
        .put(format!("{base}/api/v1/projects/{project_id}"))
        .json(&json!({ "ci_unmatched_policy": "create_all" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(updated["ci_unmatched_policy"], "create_all");

    let bad = client
        .put(format!("{base}/api/v1/projects/{project_id}"))
        .json(&json!({ "ci_unmatched_policy": "nope" }))
        .send()
        .await
        .unwrap();
    assert_eq!(bad.status(), 400);
}
