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
        uuid::Uuid::new_v4().simple().to_string()[..6].to_uppercase()
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
        .get(format!(
            "{base}/api/v1/projects/{project_id}/cases/{case_id}"
        ))
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

    let wanted = format!(
        "Z{}",
        uuid::Uuid::new_v4().simple().to_string()[..3].to_uppercase()
    );
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

#[tokio::test]
async fn recursive_counts_direct_only_and_numeric_key_sort() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let base = spawn_app().await;
    let client = reqwest::Client::new();
    let key = uniq("RC");

    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&serde_json::json!({ "key": key, "name": "Recursive Counts" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let project_id = project["id"].as_str().unwrap();
    let prefix = project["case_key_prefix"].as_str().unwrap().to_string();

    let root: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/suites"))
        .json(&serde_json::json!({ "name": "Parent", "sort_order": 0 }))
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
            "name": "Child",
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

    // One direct case on parent + twelve in child (covers string-vs-numeric order).
    client
        .post(format!("{base}/api/v1/projects/{project_id}/cases"))
        .json(&serde_json::json!({ "title": "Parent direct", "suite_id": root_id }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    for i in 1..=12 {
        client
            .post(format!("{base}/api/v1/projects/{project_id}/cases"))
            .json(&serde_json::json!({
                "title": format!("Child case {i}"),
                "suite_id": child_id
            }))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
    }

    // Unfiled case for unfiled_only.
    client
        .post(format!("{base}/api/v1/projects/{project_id}/cases"))
        .json(&serde_json::json!({ "title": "Unfiled lone" }))
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
    let child_suite = suites
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == child_id)
        .unwrap();
    assert_eq!(root_suite["case_count"], 13, "parent recursive = 1 + 12");
    assert_eq!(root_suite["direct_case_count"], 1);
    assert_eq!(child_suite["case_count"], 12);
    assert_eq!(child_suite["direct_case_count"], 12);

    let with_desc: Value = client
        .get(format!(
            "{base}/api/v1/projects/{project_id}/cases?suite_id={root_id}&limit=50"
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(with_desc["total"], 13);

    let direct: Value = client
        .get(format!(
            "{base}/api/v1/projects/{project_id}/cases?suite_id={root_id}&direct_only=true"
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(direct["total"], 1);

    let unfiled: Value = client
        .get(format!(
            "{base}/api/v1/projects/{project_id}/cases?unfiled_only=true"
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(unfiled["total"], 1);
    assert!(unfiled["items"][0]["suite_id"].is_null());

    // Numeric sort inside child: PREFIX-2 before PREFIX-10 (not localeCompare).
    let ordered: Value = client
        .get(format!(
            "{base}/api/v1/projects/{project_id}/cases?suite_id={child_id}&direct_only=true&limit=50"
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let keys: Vec<String> = ordered["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["key"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(keys.len(), 12);
    // Keys are prefix-2 .. prefix-13 (first case was on parent).
    let nums: Vec<i64> = keys
        .iter()
        .map(|k| {
            k.rsplit('-')
                .next()
                .unwrap()
                .parse::<i64>()
                .expect("numeric suffix")
        })
        .collect();
    let mut sorted = nums.clone();
    sorted.sort();
    assert_eq!(nums, sorted, "cases must be numeric-ordered, got {keys:?}");
    // Explicitly reject string sort: if string-sorted, 10 would appear before 2.
    let pos2 = keys
        .iter()
        .position(|k| k == &format!("{prefix}-2"))
        .unwrap();
    let pos10 = keys
        .iter()
        .position(|k| k == &format!("{prefix}-10"))
        .unwrap();
    assert!(
        pos2 < pos10,
        "{prefix}-2 must precede {prefix}-10, got {keys:?}"
    );
}

#[tokio::test]
async fn perf_shape_10x10x200_recursive_counts_and_lazy_pages() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let database_url = std::env::var("DATABASE_URL").unwrap();
    let base = spawn_app().await;
    let client = reqwest::Client::new();
    let key = uniq("PF");

    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&serde_json::json!({
            "key": key,
            "name": "Perf Folder Shape"
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let project_id = project["id"].as_str().unwrap().to_string();
    let prefix = project["case_key_prefix"].as_str().unwrap().to_string();
    let project_uuid = uuid::Uuid::parse_str(&project_id).unwrap();

    // Seed 10×10 folders × 200 cases via SQL (API would be too slow for 20k).
    let pool = sqlx::PgPool::connect(&database_url)
        .await
        .expect("connect for seed");

    let mut root_ids = Vec::with_capacity(10);
    for i in 0..10 {
        let id: uuid::Uuid = sqlx::query_scalar(
            r#"INSERT INTO suites (project_id, parent_id, name, sort_order)
               VALUES ($1, NULL, $2, $3) RETURNING id"#,
        )
        .bind(project_uuid)
        .bind(format!("Root-{i}"))
        .bind(i)
        .fetch_one(&pool)
        .await
        .unwrap();
        root_ids.push(id);
    }

    let mut leaf_ids = Vec::with_capacity(100);
    for (ri, root_id) in root_ids.iter().enumerate() {
        for j in 0..10i32 {
            let id: uuid::Uuid = sqlx::query_scalar(
                r#"INSERT INTO suites (project_id, parent_id, name, sort_order)
                   VALUES ($1, $2, $3, $4) RETURNING id"#,
            )
            .bind(project_uuid)
            .bind(root_id)
            .bind(format!("Leaf-{ri}-{j}"))
            .bind(j)
            .fetch_one(&pool)
            .await
            .unwrap();
            leaf_ids.push(id);
        }
    }

    // Bulk insert 200 cases per leaf = 20_000. Keys PREFIX-1 .. PREFIX-20000.
    let mut n: i64 = 0;
    for leaf in &leaf_ids {
        let mut rows = String::from(
            "INSERT INTO test_cases (project_id, suite_id, key, title, status, priority, sort_order) VALUES ",
        );
        for i in 0..200 {
            n += 1;
            if i > 0 {
                rows.push(',');
            }
            rows.push_str(&format!(
                "('{project_uuid}','{leaf}','{prefix}-{n}','Case {n}','ready','medium',0)"
            ));
        }
        sqlx::query(&rows).execute(&pool).await.unwrap();
    }
    sqlx::query("UPDATE projects SET next_case_number = $2 WHERE id = $1")
        .bind(project_uuid)
        .bind(n as i32)
        .execute(&pool)
        .await
        .unwrap();

    let suites: Value = client
        .get(format!("{base}/api/v1/projects/{project_id}/suites"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let arr = suites.as_array().unwrap();
    assert_eq!(arr.len(), 110); // 10 roots + 100 leaves

    let first_root = arr.iter().find(|s| s["name"] == "Root-0").unwrap();
    assert_eq!(first_root["case_count"], 2000, "10 leaves × 200");
    assert_eq!(first_root["direct_case_count"], 0);

    let first_leaf = arr.iter().find(|s| s["name"] == "Leaf-0-0").unwrap();
    assert_eq!(first_leaf["case_count"], 200);
    assert_eq!(first_leaf["direct_case_count"], 200);

    let leaf_id = first_leaf["id"].as_str().unwrap();
    let page1: Value = client
        .get(format!(
            "{base}/api/v1/projects/{project_id}/cases?suite_id={leaf_id}&direct_only=true&limit=100&offset=0"
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(page1["total"], 200);
    assert_eq!(page1["items"].as_array().unwrap().len(), 100);

    let page2: Value = client
        .get(format!(
            "{base}/api/v1/projects/{project_id}/cases?suite_id={leaf_id}&direct_only=true&limit=100&offset=100"
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(page2["items"].as_array().unwrap().len(), 100);

    // Parent selection includes all 2000 descendants.
    let root_id = first_root["id"].as_str().unwrap();
    let parent_page: Value = client
        .get(format!(
            "{base}/api/v1/projects/{project_id}/cases?suite_id={root_id}&limit=1"
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(parent_page["total"], 2000);

    let all: Value = client
        .get(format!("{base}/api/v1/projects/{project_id}/cases?limit=1"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(all["total"], 20_000);
}
