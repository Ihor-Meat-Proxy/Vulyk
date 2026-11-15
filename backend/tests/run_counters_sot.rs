//! Run counters SoT: denormalized test_runs.*_count and Reports summary
//! must match run_items aggregates (not every test_results row).

use std::time::Duration;

use serde_json::Value;

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
    (format!("http://{addr}"), pool)
}

fn unique_key(prefix: &str) -> String {
    // case_key_prefix max 8 chars: ^[A-Z][A-Z0-9]{1,7}$
    format!(
        "{prefix}{}",
        &uuid::Uuid::new_v4().simple().to_string().to_uppercase()[..7]
    )
}

async fn create_project_case_run(
    client: &reqwest::Client,
    base: &str,
) -> (String, String, String, String, String) {
    let key = unique_key("C");
    let project_resp = client
        .post(format!("{base}/api/v1/projects"))
        .json(&serde_json::json!({
            "key": key,
            "name": format!("Counters SoT {key}"),
            "case_key_prefix": key
        }))
        .send()
        .await
        .unwrap();
    if !project_resp.status().is_success() {
        let status = project_resp.status();
        let body = project_resp.text().await.unwrap_or_default();
        panic!("create project failed: {status} {body}");
    }
    let project: Value = project_resp.json().await.unwrap();
    let project_id = project["id"].as_str().unwrap().to_string();

    let case: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/cases"))
        .json(&serde_json::json!({ "title": "SoT case", "status": "ready" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let case_id = case["case"]["id"].as_str().unwrap().to_string();
    let case_key = case["case"]["key"].as_str().unwrap().to_string();

    let run: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/runs"))
        .json(&serde_json::json!({
            "name": "Manual SoT",
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
    let run_id = run["run"]["id"].as_str().unwrap().to_string();
    let item_id = run["items"][0]["item"]["id"].as_str().unwrap().to_string();
    (project_id, case_id, case_key, run_id, item_id)
}

#[tokio::test]
async fn update_run_item_refreshes_denormalized_counters() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }

    let (base, pool) = spawn_app().await;
    let client = reqwest::Client::new();
    let (project_id, _case_id, _case_key, run_id, item_id) =
        create_project_case_run(&client, &base).await;
    let run_uuid = uuid::Uuid::parse_str(&run_id).unwrap();

    client
        .put(format!(
            "{base}/api/v1/projects/{project_id}/runs/{run_id}/items/{item_id}"
        ))
        .json(&serde_json::json!({ "status": "passed" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let row = sqlx::query_as::<_, (i32, i32, i32, i32)>(
        r#"SELECT pass_count, fail_count, skip_count, blocked_count
           FROM test_runs WHERE id = $1"#,
    )
    .bind(run_uuid)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row, (1, 0, 0, 0));

    let item_passed: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)::bigint FROM run_items
           WHERE run_id = $1 AND status = 'Passed'"#,
    )
    .bind(run_uuid)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(item_passed, 1);

    let detail: Value = client
        .get(format!("{base}/api/v1/projects/{project_id}/runs/{run_id}"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(detail["progress"]["passed"], 1);
    assert_eq!(detail["run"]["pass_count"], 1);
}

#[tokio::test]
async fn create_result_refreshes_counters_and_upserts_run_item() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }

    let (base, pool) = spawn_app().await;
    let client = reqwest::Client::new();
    let (project_id, case_id, _case_key, run_id, _item_id) =
        create_project_case_run(&client, &base).await;
    let run_uuid = uuid::Uuid::parse_str(&run_id).unwrap();

    // Second create_result for same case → still one run_item; counters stay
    // 1 pass / 0 fail (not inflated by multiple test_results rows).
    for status in ["failed", "passed"] {
        client
            .post(format!(
                "{base}/api/v1/projects/{project_id}/runs/{run_id}/results/execute"
            ))
            .json(&serde_json::json!({
                "case_id": case_id,
                "status": status,
                "message": format!("attempt {status}"),
                "sync_run_item": true
            }))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
    }

    let result_rows: i64 =
        sqlx::query_scalar(r#"SELECT COUNT(*)::bigint FROM test_results WHERE run_id = $1"#)
            .bind(run_uuid)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(result_rows, 2, "history keeps both attempts");

    let item_count: i64 =
        sqlx::query_scalar(r#"SELECT COUNT(*)::bigint FROM run_items WHERE run_id = $1"#)
            .bind(run_uuid)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(item_count, 1, "one status per case");

    let row = sqlx::query_as::<_, (i32, i32)>(
        r#"SELECT pass_count, fail_count FROM test_runs WHERE id = $1"#,
    )
    .bind(run_uuid)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        row,
        (1, 0),
        "counters follow latest run_item, not both result rows"
    );
}

#[tokio::test]
async fn bulk_results_ingest_updates_run_items_and_counters() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }

    let (base, pool) = spawn_app().await;
    let client = reqwest::Client::new();
    let key = unique_key("B");

    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&serde_json::json!({
            "key": key,
            "name": format!("Bulk SoT {key}"),
            "case_key_prefix": key
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

    let mut case_ids = Vec::new();
    for title in ["Pass", "Fail", "Skip"] {
        let case: Value = client
            .post(format!("{base}/api/v1/projects/{project_id}/cases"))
            .json(&serde_json::json!({ "title": title, "status": "ready" }))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        case_ids.push(case["case"]["id"].as_str().unwrap().to_string());
    }

    let run: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/runs"))
        .json(&serde_json::json!({
            "name": "Bulk mixed",
            "include_all_cases": true,
            "start": true,
            "source": "ci"
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
    let run_uuid = uuid::Uuid::parse_str(run_id).unwrap();

    client
        .post(format!(
            "{base}/api/v1/projects/{project_id}/runs/{run_id}/results"
        ))
        .json(&serde_json::json!({
            "results": [
                { "case_id": case_ids[0], "status": "passed" },
                { "case_id": case_ids[1], "status": "failed" },
                { "case_id": case_ids[2], "status": "skipped" }
            ]
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let row = sqlx::query_as::<_, (i32, i32, i32)>(
        r#"SELECT pass_count, fail_count, skip_count FROM test_runs WHERE id = $1"#,
    )
    .bind(run_uuid)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(row, (1, 1, 1));

    let summary: Value = client
        .get(format!(
            "{base}/api/v1/projects/{project_id}/reports/runs/{run_id}/summary"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(summary["counts"]["passed"], 1);
    assert_eq!(summary["counts"]["failed"], 1);
    assert_eq!(summary["counts"]["skipped"], 1);
    assert_eq!(summary["pass_rate"], 50.0);
}

#[tokio::test]
async fn ci_ingest_reports_summary_matches_run_detail_progress() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }

    let (base, _pool) = spawn_app().await;
    let client = reqwest::Client::new();
    let key = unique_key("I");

    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&serde_json::json!({
            "key": key,
            "name": format!("CI SoT {key}"),
            "case_key_prefix": key
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

    let case_a: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/cases"))
        .json(&serde_json::json!({ "title": "CI A", "status": "ready" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let case_b: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/cases"))
        .json(&serde_json::json!({ "title": "CI B", "status": "ready" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let key_a = case_a["case"]["key"].as_str().unwrap();
    let key_b = case_b["case"]["key"].as_str().unwrap();

    let opened: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/ci/runs"))
        .json(&serde_json::json!({
            "provider": "github",
            "repo": format!("acme/{key}"),
            "branch": "main",
            "commit": "sotcafe",
            "external_ref": format!("sot-{key}"),
            "suite_name": "SoT suite"
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let run_id = opened["run"]["id"].as_str().unwrap();

    // Two results for case A (retry) + one for B → run_items = 2 cases, not 3 rows.
    client
        .post(format!(
            "{base}/api/v1/projects/{project_id}/ci/runs/{run_id}/results"
        ))
        .json(&serde_json::json!({
            "results": [
                { "case_key": key_a, "status": "failed", "external_id": "a1", "duration_ms": 1 },
                { "case_key": key_a, "status": "passed", "external_id": "a2", "duration_ms": 2 },
                { "case_key": key_b, "status": "skipped", "external_id": "b1", "duration_ms": 3 }
            ]
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    client
        .post(format!(
            "{base}/api/v1/projects/{project_id}/ci/runs/{run_id}/close"
        ))
        .json(&serde_json::json!({ "message": "done" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let detail: Value = client
        .get(format!("{base}/api/v1/projects/{project_id}/runs/{run_id}"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();

    let summary: Value = client
        .get(format!(
            "{base}/api/v1/projects/{project_id}/reports/runs/{run_id}/summary"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(detail["progress"]["total"], 2);
    assert_eq!(detail["progress"]["passed"], 1);
    assert_eq!(detail["progress"]["skipped"], 1);
    assert_eq!(detail["progress"]["failed"], 0);

    assert_eq!(summary["counts"]["total"], 2);
    assert_eq!(summary["counts"]["passed"], 1);
    assert_eq!(summary["counts"]["skipped"], 1);
    assert_eq!(summary["counts"]["failed"], 0);

    assert_eq!(detail["run"]["pass_count"], 1);
    assert_eq!(detail["run"]["skip_count"], 1);
    assert_eq!(detail["run"]["fail_count"], 0);
}
