//! Integration tests: append-only result history immutability and queries.

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

#[tokio::test]
async fn result_history_is_append_only_and_queryable() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }

    let base = spawn_app().await;
    let client = reqwest::Client::new();

    let key = format!(
        "H{}",
        uuid::Uuid::new_v4().simple().to_string()[..6].to_uppercase()
    );
    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&serde_json::json!({
            "key": key,
            "name": "History Project",
            "description": "result history"
        }))
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
        .json(&serde_json::json!({
            "key": format!("{key}-1"),
            "title": "History case",
            "steps": [
                { "action": "Do thing", "expected_result": "ok" }
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

    let run: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/runs"))
        .json(&serde_json::json!({
            "name": "History run",
            "case_ids": [case_id],
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
    let item_id = run["items"][0]["item"]["id"].as_str().unwrap();
    assert_eq!(run["items"][0]["item"]["status"], "Untested");

    let statuses = ["Passed", "Failed", "Blocked", "Skipped"];
    let mut result_ids = Vec::new();
    for status in statuses {
        let result: Value = client
            .post(format!(
                "{base}/api/v1/projects/{project_id}/runs/{run_id}/results/execute"
            ))
            .json(&serde_json::json!({
                "run_item_id": item_id,
                "status": status,
                "message": format!("marked {status}"),
            }))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(result["result"]["status"], status);
        assert!(result["result"]["case_version"].as_i64().unwrap() >= 1);
        result_ids.push(result["result"]["id"].as_str().unwrap().to_string());
    }

    // Run-item timeline (chronological).
    let timeline: Value = client
        .get(format!(
            "{base}/api/v1/projects/{project_id}/runs/{run_id}/items/{item_id}/results"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let items = timeline.as_array().unwrap();
    assert_eq!(items.len(), 4);
    for (i, status) in statuses.iter().enumerate() {
        assert_eq!(items[i]["result"]["status"], *status);
        assert_eq!(items[i]["result"]["id"], result_ids[i]);
    }

    // Execution view embeds history.
    let exec: Value = client
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
    assert_eq!(exec["history"].as_array().unwrap().len(), 4);
    assert_eq!(exec["latest_result"]["result"]["status"], "Skipped");
    assert_eq!(exec["item"]["item"]["status"], "Skipped");

    // Case-level history with filters + summary.
    let case_hist: Value = client
        .get(format!("{base}/api/v1/cases/{case_id}/results?limit=10"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(case_hist["total"], 4);
    assert_eq!(case_hist["summary"]["total_results"], 4);
    assert_eq!(case_hist["summary"]["latest_status"], "Skipped");
    assert_eq!(case_hist["summary"]["passed"], 1);
    assert_eq!(case_hist["summary"]["failed"], 1);
    assert_eq!(case_hist["summary"]["blocked"], 1);
    assert_eq!(case_hist["summary"]["skipped"], 1);
    assert!((case_hist["summary"]["pass_rate"].as_f64().unwrap() - 50.0).abs() < 0.01);

    let filtered: Value = client
        .get(format!(
            "{base}/api/v1/projects/{project_id}/cases/{case_id}/results?status=Failed"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(filtered["total"], 1);
    assert_eq!(filtered["items"][0]["result"]["status"], "Failed");

    // Immutability: UPDATE is rejected by DB trigger.
    let pool = vulyk_api::db::connect(&std::env::var("DATABASE_URL").unwrap())
        .await
        .unwrap();
    let rid = uuid::Uuid::parse_str(&result_ids[0]).unwrap();
    let err = sqlx::query("UPDATE test_results SET message = 'tamper' WHERE id = $1")
        .bind(rid)
        .execute(&pool)
        .await;
    assert!(err.is_err(), "expected update to fail");
    let msg = err.unwrap_err().to_string();
    assert!(
        msg.contains("immutable") || msg.contains("append-only"),
        "unexpected error: {msg}"
    );

    // Re-ingest with same external_id still appends (never overwrites).
    let ingest1: Value = client
        .post(format!("{base}/api/v1/runs/{run_id}/results:bulk"))
        .json(&serde_json::json!({
            "upsert": true,
            "results": [{
                "case_id": case_id,
                "external_id": "hist::same",
                "status": "passed"
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
    assert_eq!(ingest1["inserted"], 1);
    assert_eq!(ingest1["updated"], 0);

    let ingest2: Value = client
        .post(format!("{base}/api/v1/runs/{run_id}/results:bulk"))
        .json(&serde_json::json!({
            "upsert": true,
            "results": [{
                "case_id": case_id,
                "external_id": "hist::same",
                "status": "failed"
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
    assert_eq!(ingest2["inserted"], 1);
    assert_eq!(ingest2["updated"], 0);

    let by_ext: (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM test_results WHERE run_id = $1 AND external_id = $2",
    )
    .bind(uuid::Uuid::parse_str(run_id).unwrap())
    .bind("hist::same")
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(by_ext.0, 2);

    // Accept lowercase input → PascalCase stored.
    let lower: Value = client
        .post(format!(
            "{base}/api/v1/projects/{project_id}/runs/{run_id}/results/execute"
        ))
        .json(&serde_json::json!({
            "run_item_id": item_id,
            "status": "retest",
            "message": "lowercase ok",
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(lower["result"]["status"], "Retest");
}
