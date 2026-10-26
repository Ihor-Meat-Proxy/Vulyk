//! Case list sorting, enrichment, and per-user column preferences.

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

async fn spawn_app_auth() -> (String, sqlx::PgPool) {
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
            required: true,
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
    (format!("http://{addr}"), pool)
}

fn uniq(prefix: &str) -> String {
    format!(
        "{prefix}{}",
        uuid::Uuid::new_v4().simple().to_string()[..6].to_uppercase()
    )
}

async fn insert_admin_and_login(
    pool: &sqlx::PgPool,
    client: &reqwest::Client,
    base: &str,
) -> String {
    let email = format!("admin-{}@example.com", uniq("A").to_lowercase());
    let password = "Str0ngPass1";
    let hash = vulyk_api::auth::hash_password(password).expect("hash");
    sqlx::query(
        r#"INSERT INTO users (email, display_name, password_hash, is_system_admin)
           VALUES ($1, $2, $3, TRUE)"#,
    )
    .bind(&email)
    .bind("Prefs Admin")
    .bind(hash)
    .execute(pool)
    .await
    .expect("insert admin");

    let login: Value = client
        .post(format!("{base}/api/v1/auth/login"))
        .json(&serde_json::json!({ "email": email, "password": password }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    login["token"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn list_sorts_by_title_and_enriches_fields() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let base = spawn_app().await;
    let client = reqwest::Client::new();
    let key = uniq("CL");

    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&serde_json::json!({ "key": key, "name": "Column List Proj" }))
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
        .json(&serde_json::json!({ "name": "Alpha Folder" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let suite_id = suite["id"].as_str().unwrap();

    for (title, ext) in [
        ("Zebra case", Some("e2e:z")),
        ("Alpha case", None::<&str>),
        ("Middle case", Some("import:1")),
    ] {
        let mut body = serde_json::json!({
            "title": title,
            "suite_id": suite_id,
            "priority": "medium",
            "status": "ready",
            "tag_names": ["beta", "alpha"],
        });
        if let Some(e) = ext {
            body["external_key"] = Value::String(e.into());
        }
        client
            .post(format!("{base}/api/v1/projects/{project_id}/cases"))
            .json(&body)
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
    }

    let page: Value = client
        .get(format!(
            "{base}/api/v1/projects/{project_id}/cases?sort=title&order=asc&limit=50"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();

    let titles: Vec<&str> = page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["title"].as_str().unwrap())
        .collect();
    assert_eq!(titles, vec!["Alpha case", "Middle case", "Zebra case"]);

    let zebra = page["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["title"] == "Zebra case")
        .unwrap();
    assert_eq!(zebra["automated"], true);
    assert_eq!(zebra["folder_name"], "Alpha Folder");
    let tags = zebra["tag_names"].as_array().unwrap();
    assert!(tags.iter().any(|t| t == "alpha"));
    assert!(tags.iter().any(|t| t == "beta"));

    let alpha = page["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["title"] == "Alpha case")
        .unwrap();
    assert_eq!(alpha["automated"], false);

    // Sort by automated desc → linked cases first.
    let auto_page: Value = client
        .get(format!(
            "{base}/api/v1/projects/{project_id}/cases?sort=automated&order=desc&limit=50"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(auto_page["items"][0]["automated"], true);
    assert_eq!(auto_page["items"][2]["automated"], false);

    // Reject unknown sort.
    let bad = client
        .get(format!(
            "{base}/api/v1/projects/{project_id}/cases?sort=not_a_column"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(bad.status(), 400);
}

#[tokio::test]
async fn list_sorts_by_custom_field_and_returns_cf_values() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let base = spawn_app().await;
    let client = reqwest::Client::new();
    let key = uniq("CF");

    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&serde_json::json!({ "key": key, "name": "CF Sort Proj" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let project_id = project["id"].as_str().unwrap();

    let field: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/custom-fields"))
        .json(&serde_json::json!({
            "name": "Severity",
            "field_type": "select",
            "options": ["low", "high"]
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

    for (title, val) in [("Case B", "high"), ("Case A", "low")] {
        let created: Value = client
            .post(format!("{base}/api/v1/projects/{project_id}/cases"))
            .json(&serde_json::json!({ "title": title }))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        let case_id = created["case"]["id"].as_str().unwrap();
        client
            .post(format!(
                "{base}/api/v1/projects/{project_id}/cases/{case_id}/custom-fields"
            ))
            .json(&serde_json::json!([{ "field_id": field_id, "value": val }]))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap();
    }

    let page: Value = client
        .get(format!(
            "{base}/api/v1/projects/{project_id}/cases?sort=cf:{field_id}&order=asc&cf_ids={field_id}&limit=50"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();

    let titles: Vec<&str> = page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["title"].as_str().unwrap())
        .collect();
    // "high" < "low" lexicographically? h < l so high first when asc
    assert_eq!(titles[0], "Case B");
    assert_eq!(titles[1], "Case A");

    let cfs = page["items"][0]["custom_fields"].as_array().unwrap();
    assert_eq!(cfs.len(), 1);
    assert_eq!(cfs[0]["name"], "Severity");
}

#[tokio::test]
async fn column_preferences_persist_per_user_and_reset() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let (base, pool) = spawn_app_auth().await;
    let client = reqwest::Client::new();
    let token = insert_admin_and_login(&pool, &client, &base).await;

    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .header("Authorization", format!("Bearer {token}"))
        .json(&serde_json::json!({
            "key": uniq("PREF"),
            "name": "Prefs Project"
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

    let defaults: Value = client
        .get(format!(
            "{base}/api/v1/projects/{project_id}/preferences/case-list-columns"
        ))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(defaults["columns"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["id"] == "title" && c["visible"] == true));

    let saved: Value = client
        .put(format!(
            "{base}/api/v1/projects/{project_id}/preferences/case-list-columns"
        ))
        .header("Authorization", format!("Bearer {token}"))
        .json(&serde_json::json!({
            "columns": [
                { "id": "title", "visible": true },
                { "id": "key", "visible": true },
                { "id": "automated", "visible": true },
                { "id": "priority", "visible": false },
                { "id": "status", "visible": false }
            ],
            "sort": "title",
            "order": "desc"
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(saved["sort"], "title");
    assert_eq!(saved["order"], "desc");
    assert!(saved["columns"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["id"] == "automated" && c["visible"] == true));

    // Cannot hide title.
    let reject = client
        .put(format!(
            "{base}/api/v1/projects/{project_id}/preferences/case-list-columns"
        ))
        .header("Authorization", format!("Bearer {token}"))
        .json(&serde_json::json!({
            "columns": [{ "id": "title", "visible": false }]
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(reject.status(), 400);

    let reset: Value = client
        .delete(format!(
            "{base}/api/v1/projects/{project_id}/preferences/case-list-columns"
        ))
        .header("Authorization", format!("Bearer {token}"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(reset["sort"].is_null());
    let auto = reset["columns"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "automated")
        .unwrap();
    assert_eq!(auto["visible"], false);
}

#[tokio::test]
async fn last_run_status_from_test_results() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let base = spawn_app().await;
    let client = reqwest::Client::new();
    let key = uniq("LR");

    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&serde_json::json!({ "key": key, "name": "Last Run Proj" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let project_id = project["id"].as_str().unwrap();

    let created: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/cases"))
        .json(&serde_json::json!({ "title": "Has result" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let case_id = created["case"]["id"].as_str().unwrap();

    let run: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/runs"))
        .json(&serde_json::json!({
            "name": "Run 1",
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

    client
        .post(format!(
            "{base}/api/v1/projects/{project_id}/runs/{run_id}/results/execute"
        ))
        .json(&serde_json::json!({
            "run_item_id": item_id,
            "status": "Passed",
            "message": "ok"
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let page: Value = client
        .get(format!(
            "{base}/api/v1/projects/{project_id}/cases?sort=last_run_status&order=asc"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(page["items"][0]["last_run_status"], "Passed");
}
