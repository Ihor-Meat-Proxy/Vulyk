//! HTTP smoke tests against a real Postgres (CI service / local docker).

use std::time::Duration;

use serde_json::Value;

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
async fn health_and_project_case_crud() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }

    let base = spawn_app().await;
    let client = reqwest::Client::new();

    let health: Value = client
        .get(format!("{base}/health"))
        .send()
        .await
        .expect("health")
        .json()
        .await
        .expect("json");
    assert_eq!(health["status"], "ok");
    assert_eq!(
        health["version"],
        vulyk_api::db::crate_version(),
        "health should expose API semver"
    );

    let key = format!(
        "T{}",
        uuid::Uuid::new_v4().simple().to_string()[..6].to_uppercase()
    );
    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&serde_json::json!({
            "key": key,
            "name": "CI Project",
            "description": "from smoke test"
        }))
        .send()
        .await
        .expect("create project")
        .error_for_status()
        .expect("project status")
        .json()
        .await
        .expect("project json");
    let project_id = project["id"].as_str().expect("id");

    let suite: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/suites"))
        .json(&serde_json::json!({ "name": "Regression" }))
        .send()
        .await
        .expect("suite")
        .error_for_status()
        .expect("suite status")
        .json()
        .await
        .expect("suite json");
    let suite_id = suite["id"].as_str().expect("suite id");

    let detail: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/cases"))
        .json(&serde_json::json!({
            "key": format!("{key}-1"),
            "title": "Login works",
            "suite_id": suite_id,
            "steps": [{ "action": "Open app", "expected_result": "Dashboard" }],
            "tag_names": ["smoke"]
        }))
        .send()
        .await
        .expect("case")
        .error_for_status()
        .expect("case status")
        .json()
        .await
        .expect("case json");
    assert_eq!(detail["case"]["title"], "Login works");
    assert_eq!(detail["steps"].as_array().expect("steps").len(), 1);
    assert_eq!(detail["tags"].as_array().expect("tags").len(), 1);

    let cases: Value = client
        .get(format!("{base}/api/v1/projects/{project_id}/cases"))
        .send()
        .await
        .expect("list")
        .json()
        .await
        .expect("list json");
    assert_eq!(cases["items"].as_array().expect("arr").len(), 1);
    assert_eq!(cases["total"], 1);
}

#[tokio::test]
async fn case_depth_and_run_execution() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }

    let base = spawn_app().await;
    let client = reqwest::Client::new();
    let key = format!(
        "R{}",
        uuid::Uuid::new_v4().simple().to_string()[..6].to_uppercase()
    );

    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&serde_json::json!({ "key": key, "name": "Run Project" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let project_id = project["id"].as_str().unwrap();

    let suite: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/suites"))
        .json(&serde_json::json!({ "name": "Core" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let suite_id = suite["id"].as_str().unwrap();

    let field: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/custom-fields"))
        .json(&serde_json::json!({
            "name": "Component",
            "field_type": "select",
            "options": ["API", "UI"]
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

    let case: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/cases"))
        .json(&serde_json::json!({
            "key": format!("{key}-1"),
            "title": "Search indexes titles",
            "suite_id": suite_id,
            "status": "ready",
            "tag_names": ["fts", "w2"],
            "steps": [
                { "action": "Index cases", "expected_result": "GIN ready" },
                { "action": "Query FTS", "expected_result": "Hits returned" }
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
    let case_id = case["case"]["id"].as_str().unwrap();

    client
        .post(format!(
            "{base}/api/v1/projects/{project_id}/cases/{case_id}/custom-fields"
        ))
        .json(&serde_json::json!([{ "field_id": field_id, "value": "API" }]))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let updated: Value = client
        .put(format!(
            "{base}/api/v1/projects/{project_id}/cases/{case_id}"
        ))
        .json(&serde_json::json!({
            "title": "Search indexes titles and keys",
            "change_summary": "clarify FTS scope",
            "steps": [
                { "action": "Index cases", "expected_result": "GIN ready" },
                { "action": "Query FTS", "expected_result": "Hits ranked" }
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
    assert!(updated["case"]["version"].as_i64().unwrap() >= 2);

    let versions: Value = client
        .get(format!(
            "{base}/api/v1/projects/{project_id}/cases/{case_id}/versions"
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(!versions.as_array().unwrap().is_empty());

    let search: Value = client
        .get(format!(
            "{base}/api/v1/projects/{project_id}/cases/search?q=indexes&limit=10"
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(search["total"].as_i64().unwrap() >= 1);

    let bulk: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/cases/bulk"))
        .json(&serde_json::json!({
            "action": "set_status",
            "case_ids": [case_id],
            "status": "ready"
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(bulk["affected"], 1);

    let plan: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/plans"))
        .json(&serde_json::json!({
            "name": "Release checklist",
            "case_ids": [case_id]
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let plan_id = plan["plan"]["id"].as_str().unwrap();
    assert_eq!(plan["case_count"], 1);

    let run: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/runs"))
        .json(&serde_json::json!({
            "name": "Nightly",
            "plan_id": plan_id,
            "suite_ids": [suite_id],
            "start": true
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let run_id = run["run"]["id"].as_str().unwrap();
    assert_eq!(run["progress"]["total"], 1);
    assert_eq!(run["run"]["status"], "in_progress");
    let item_id = run["items"][0]["item"]["id"].as_str().unwrap();

    let execution: Value = client
        .get(format!(
            "{base}/api/v1/projects/{project_id}/runs/{run_id}/items/{item_id}"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(execution["steps"].as_array().unwrap().len(), 2);

    let result: Value = client
        .post(format!(
            "{base}/api/v1/projects/{project_id}/runs/{run_id}/results/execute"
        ))
        .json(&serde_json::json!({
            "run_item_id": item_id,
            "status": "passed",
            "message": "all good",
            "assignee": "qa@example.com",
            "steps": [
                {
                    "position": 0,
                    "action": "Index cases",
                    "expected_result": "GIN ready",
                    "status": "passed"
                },
                {
                    "position": 1,
                    "action": "Query FTS",
                    "expected_result": "Hits ranked",
                    "status": "passed",
                    "actual_result": "2 hits"
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
    assert_eq!(result["result"]["status"], "Passed");
    assert_eq!(result["steps"].as_array().unwrap().len(), 2);

    let attachment: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/attachments"))
        .json(&serde_json::json!({
            "entity_type": "result",
            "entity_id": result["result"]["id"],
            "filename": "note.txt",
            "content_type": "text/plain",
            "content_base64": "b2s="
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(attachment["filename"], "note.txt");
    assert_eq!(attachment["size_bytes"], 2);

    let run_after: Value = client
        .get(format!("{base}/api/v1/projects/{project_id}/runs/{run_id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(run_after["progress"]["passed"], 1);
    assert_eq!(run_after["items"][0]["item"]["status"], "Passed");
}
