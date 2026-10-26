//! Option lists, custom fields, and config export/import integration tests.

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
            jwt_secret: "test-secret-cf".into(),
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

fn unique(prefix: &str) -> String {
    format!(
        "{prefix}_{}",
        &uuid::Uuid::new_v4().simple().to_string()[..8]
    )
}

async fn insert_admin(pool: &sqlx::PgPool, email: &str, password: &str) -> uuid::Uuid {
    let hash = hash_password(password).expect("hash");
    sqlx::query_scalar(
        r#"INSERT INTO users (email, display_name, password_hash, is_system_admin)
           VALUES ($1, $2, $3, TRUE)
           RETURNING id"#,
    )
    .bind(email)
    .bind("CF Admin")
    .bind(hash)
    .fetch_one(pool)
    .await
    .expect("insert admin")
}

async fn login(client: &reqwest::Client, base: &str, email: &str, password: &str) -> String {
    let login: Value = client
        .post(format!("{base}/api/v1/auth/login"))
        .json(&serde_json::json!({ "email": email, "password": password }))
        .send()
        .await
        .expect("login")
        .error_for_status()
        .expect("login ok")
        .json()
        .await
        .expect("json");
    login["token"].as_str().expect("token").to_string()
}

#[tokio::test]
async fn lists_custom_fields_values_and_config_roundtrip() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }

    let (base, pool) = spawn_app(true).await;
    let client = reqwest::Client::new();
    let email = format!("{}@example.com", unique("cfadmin"));
    let password = "TestPass1";
    insert_admin(&pool, &email, password).await;
    let token = login(&client, &base, &email, password).await;
    let auth =
        |req: reqwest::RequestBuilder| req.header("Authorization", format!("Bearer {token}"));

    // Create project
    let pkey = unique("CFP").to_uppercase();
    let project: Value = auth(
        client
            .post(format!("{base}/api/v1/projects"))
            .json(&serde_json::json!({ "key": pkey, "name": "CF Project" })),
    )
    .send()
    .await
    .expect("project")
    .error_for_status()
    .expect("project ok")
    .json()
    .await
    .expect("project json");
    let project_id = project["id"].as_str().expect("pid");

    // Option list
    let list_key = unique("browsers");
    let list: Value = auth(client.post(format!("{base}/api/v1/admin/lists")).json(
        &serde_json::json!({
            "key": list_key,
            "name": "Browsers",
            "items": [
                { "label": "Chrome", "key": "chrome", "sort_order": 0 },
                { "label": "Firefox", "key": "firefox", "sort_order": 1 }
            ]
        }),
    ))
    .send()
    .await
    .expect("list")
    .error_for_status()
    .expect("list ok")
    .json()
    .await
    .expect("list json");
    let list_id = list["id"].as_str().expect("list id");
    assert_eq!(list["items"].as_array().expect("items").len(), 2);

    // Custom fields (all projects)
    let auto_key = unique("automated");
    let auto: Value = auth(
        client
            .post(format!("{base}/api/v1/admin/custom-fields"))
            .json(&serde_json::json!({
                "key": auto_key,
                "name": "Automated?",
                "field_type": "checkbox",
                "entity": "test_case",
                "scope": "all",
                "default_value": false,
                "show_in_list": true,
                "show_in_filters": true
            })),
    )
    .send()
    .await
    .expect("auto")
    .error_for_status()
    .expect("auto ok")
    .json()
    .await
    .expect("auto json");
    let auto_id = auto["id"].as_str().expect("auto id");

    let browsers_key = unique("browsers_field");
    let browsers: Value = auth(
        client
            .post(format!("{base}/api/v1/admin/custom-fields"))
            .json(&serde_json::json!({
                "key": browsers_key,
                "name": "Browsers",
                "field_type": "multi_select",
                "entity": "test_case",
                "option_list_id": list_id,
                "scope": "all",
                "default_value": [],
                "show_in_list": true,
                "show_in_filters": true
            })),
    )
    .send()
    .await
    .expect("browsers")
    .error_for_status()
    .expect("browsers ok")
    .json()
    .await
    .expect("browsers json");
    let browsers_id = browsers["id"].as_str().expect("browsers id");

    // Project-resolved fields include both
    let fields: Value = auth(client.get(format!(
        "{base}/api/v1/projects/{project_id}/custom-fields?entity=test_case"
    )))
    .send()
    .await
    .expect("fields")
    .error_for_status()
    .expect("fields ok")
    .json()
    .await
    .expect("fields json");
    let field_ids: Vec<&str> = fields
        .as_array()
        .expect("arr")
        .iter()
        .filter_map(|f| f["id"].as_str())
        .collect();
    assert!(field_ids.contains(&auto_id));
    assert!(field_ids.contains(&browsers_id));

    // Create case + set values
    let suite: Value = auth(
        client
            .post(format!("{base}/api/v1/projects/{project_id}/suites"))
            .json(&serde_json::json!({ "name": "Suite" })),
    )
    .send()
    .await
    .expect("suite")
    .error_for_status()
    .expect("suite ok")
    .json()
    .await
    .expect("suite json");
    let suite_id = suite["id"].as_str().expect("suite id");

    let detail: Value = auth(
        client
            .post(format!("{base}/api/v1/projects/{project_id}/cases"))
            .json(&serde_json::json!({
                "title": "CF case",
                "suite_id": suite_id,
                "steps": [{ "action": "A", "expected_result": "B" }]
            })),
    )
    .send()
    .await
    .expect("case")
    .error_for_status()
    .expect("case ok")
    .json()
    .await
    .expect("case json");
    let case_id = detail["case"]["id"].as_str().expect("case id");

    let updated: Value = auth(
        client
            .post(format!(
                "{base}/api/v1/projects/{project_id}/cases/{case_id}/custom-fields"
            ))
            .json(&serde_json::json!([
                { "field_id": auto_id, "value": true },
                { "field_id": browsers_id, "value": ["chrome", "firefox"] }
            ])),
    )
    .send()
    .await
    .expect("set cf")
    .error_for_status()
    .expect("set ok")
    .json()
    .await
    .expect("set json");
    let cfs = updated["custom_fields"].as_array().expect("cfs");
    let auto_val = cfs
        .iter()
        .find(|c| c["field_id"] == auto_id)
        .expect("auto val");
    assert_eq!(auto_val["value"], true);

    // Deprecate list item in use
    let item_id = list["items"][0]["id"].as_str().expect("item");
    let dep: Value = auth(client.delete(format!(
        "{base}/api/v1/admin/lists/{list_id}/items/{item_id}"
    )))
    .send()
    .await
    .expect("dep")
    .error_for_status()
    .expect("dep ok")
    .json()
    .await
    .expect("dep json");
    assert_eq!(dep["action"], "deprecated");

    // Config export / import dry-run
    let export_resp = auth(
        client
            .post(format!("{base}/api/v1/admin/config/export"))
            .json(&serde_json::json!({
                "project_scope": "all",
                "format": "json",
                "include_folder_structure": true
            })),
    )
    .send()
    .await
    .expect("export")
    .error_for_status()
    .expect("export ok");
    let export_body = export_resp.text().await.expect("export body");
    let bundle: Value = serde_json::from_str(&export_body).expect("bundle json");
    assert_eq!(bundle["version"], "1.0");
    assert!(!bundle["option_lists"].as_array().unwrap().is_empty());
    assert!(bundle["custom_fields"].as_array().unwrap().len() >= 2);

    let preview: Value = auth(
        client
            .post(format!("{base}/api/v1/admin/config/import/preview"))
            .json(&serde_json::json!({
                "bundle": bundle,
                "strategy": "merge",
                "dry_run": true
            })),
    )
    .send()
    .await
    .expect("preview")
    .error_for_status()
    .expect("preview ok")
    .json()
    .await
    .expect("preview json");
    assert_eq!(preview["dry_run"], true);
    assert_eq!(preview["applied"], false);

    // Non-admin cannot manage lists
    let user_email = format!("{}@example.com", unique("editor"));
    let hash = hash_password(password).unwrap();
    let user_id: uuid::Uuid = sqlx::query_scalar(
        r#"INSERT INTO users (email, display_name, password_hash, is_system_admin)
           VALUES ($1, 'Editor', $2, FALSE) RETURNING id"#,
    )
    .bind(&user_email)
    .bind(hash)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO memberships (project_id, user_id, role)
           VALUES ($1, $2, 'test_automation_engineer')"#,
    )
    .bind(uuid::Uuid::parse_str(project_id).unwrap())
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap();
    let editor_token = login(&client, &base, &user_email, password).await;
    let forbidden = client
        .get(format!("{base}/api/v1/admin/lists"))
        .header("Authorization", format!("Bearer {editor_token}"))
        .send()
        .await
        .expect("forbidden");
    assert_eq!(forbidden.status().as_u16(), 403);
}
