//! Case key allocation + folder (suite) operations.

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

fn uniq(prefix: &str) -> String {
    format!(
        "{prefix}{}",
        &uuid::Uuid::new_v4().simple().to_string()[..6].to_uppercase()
    )
}

#[tokio::test]
async fn allocates_prefix_from_name_and_monotonic_keys() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let base = spawn_app().await;
    let client = reqwest::Client::new();
    let key = uniq("K");

    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&serde_json::json!({
            "key": key,
            "name": "Kyiv Open-Air Concert 2026"
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let prefix = project["case_key_prefix"].as_str().unwrap();
    assert!(
        prefix.starts_with("KOC"),
        "expected KOC initials, got {prefix}"
    );
    assert_eq!(project["next_case_number"], 0);

    let project_id = project["id"].as_str().unwrap();
    let c1: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/cases"))
        .json(&serde_json::json!({ "title": "First" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(c1["case"]["key"], format!("{prefix}-1"));

    let c2: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/cases"))
        .json(&serde_json::json!({ "title": "Second" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(c2["case"]["key"], format!("{prefix}-2"));

    let refreshed: Value = client
        .get(format!("{base}/api/v1/projects/{project_id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(refreshed["next_case_number"], 2);
}

#[tokio::test]
async fn deleted_keys_are_never_reused() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let base = spawn_app().await;
    let client = reqwest::Client::new();
    let key = uniq("D");

    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&serde_json::json!({ "key": key, "name": "Delete Reuse Check" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let project_id = project["id"].as_str().unwrap();
    let prefix = project["case_key_prefix"].as_str().unwrap();

    let mut ids = Vec::new();
    for i in 1..=3 {
        let detail: Value = client
            .post(format!("{base}/api/v1/projects/{project_id}/cases"))
            .json(&serde_json::json!({ "title": format!("Case {i}") }))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(detail["case"]["key"], format!("{prefix}-{i}"));
        ids.push(detail["case"]["id"].as_str().unwrap().to_string());
    }

    // Delete the highest number — counter must not decrease.
    client
        .delete(format!(
            "{base}/api/v1/projects/{project_id}/cases/{}",
            ids[2]
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let next: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/cases"))
        .json(&serde_json::json!({ "title": "After delete" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(next["case"]["key"], format!("{prefix}-4"));
}

#[tokio::test]
async fn concurrent_creates_never_collide() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let base = spawn_app().await;
    let client = reqwest::Client::new();
    let key = uniq("C");

    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&serde_json::json!({ "key": key, "name": "Concurrent Keys" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let project_id = project["id"].as_str().unwrap().to_string();
    let url = format!("{base}/api/v1/projects/{project_id}/cases");

    let mut handles = Vec::new();
    for i in 0..12 {
        let client = client.clone();
        let url = url.clone();
        handles.push(tokio::spawn(async move {
            client
                .post(&url)
                .json(&serde_json::json!({ "title": format!("Parallel {i}") }))
                .send()
                .await
                .unwrap()
                .error_for_status()
                .unwrap()
                .json::<Value>()
                .await
                .unwrap()
        }));
    }

    let mut keys = Vec::new();
    for h in handles {
        let detail = h.await.unwrap();
        keys.push(detail["case"]["key"].as_str().unwrap().to_string());
    }
    keys.sort();
    keys.dedup();
    assert_eq!(keys.len(), 12, "expected unique keys, got {keys:?}");
}

#[tokio::test]
async fn folder_nesting_move_and_delete_options() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let base = spawn_app().await;
    let client = reqwest::Client::new();
    let key = uniq("F");

    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&serde_json::json!({ "key": key, "name": "Folder Ops" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let project_id = project["id"].as_str().unwrap();

    let root: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/suites"))
        .json(&serde_json::json!({ "name": "Root folder", "sort_order": 0 }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let root_id = root["id"].as_str().unwrap();

    let child: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/suites"))
        .json(&serde_json::json!({
            "name": "Child folder",
            "parent_id": root_id,
            "sort_order": 0
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let child_id = child["id"].as_str().unwrap();

    let case: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/cases"))
        .json(&serde_json::json!({
            "title": "Nested case",
            "suite_id": child_id
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

    // Selecting parent folder includes descendant cases.
    let listed: Value = client
        .get(format!(
            "{base}/api/v1/projects/{project_id}/cases?suite_id={root_id}"
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(listed["total"], 1);

    // Move case to root via bulk.
    client
        .post(format!("{base}/api/v1/projects/{project_id}/cases/bulk"))
        .json(&serde_json::json!({
            "action": "move_suite",
            "case_ids": [case_id],
            "suite_id": root_id
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let suites: Value = client
        .get(format!("{base}/api/v1/projects/{project_id}/suites"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let root_suite = suites
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == root_id)
        .unwrap();
    assert_eq!(root_suite["case_count"], 1);

    // Delete child (empty) then delete root with move_to_parent (case → unfiled).
    client
        .delete(format!(
            "{base}/api/v1/projects/{project_id}/suites/{child_id}"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    client
        .delete(format!(
            "{base}/api/v1/projects/{project_id}/suites/{root_id}?on_nonempty=move_to_parent"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let after: Value = client
        .get(format!("{base}/api/v1/projects/{project_id}/cases/{case_id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(after["case"]["suite_id"].is_null());
}

#[tokio::test]
async fn prefix_editable_only_before_cases_exist() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let base = spawn_app().await;
    let client = reqwest::Client::new();
    let key = uniq("P");

    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&serde_json::json!({ "key": key, "name": "Prefix Edit" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let project_id = project["id"].as_str().unwrap();

    let wanted = format!("Z{}", &uuid::Uuid::new_v4().simple().to_string()[..3].to_uppercase());
    let updated: Value = client
        .put(format!("{base}/api/v1/projects/{project_id}"))
        .json(&serde_json::json!({ "case_key_prefix": wanted }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(updated["case_key_prefix"], wanted);

    client
        .post(format!("{base}/api/v1/projects/{project_id}/cases"))
        .json(&serde_json::json!({ "title": "Locks prefix" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let denied = client
        .put(format!("{base}/api/v1/projects/{project_id}"))
        .json(&serde_json::json!({ "case_key_prefix": "YY" }))
        .send()
        .await
        .unwrap();
    assert_eq!(denied.status(), 400);
}
