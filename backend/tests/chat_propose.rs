//! Chat propose/confirm with stubbed mock LLM (no network).

use std::time::Duration;

use serde_json::Value;

async fn spawn_app() -> String {
    std::env::set_var("VULYK_LLM_MOCK", "1");
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
async fn chat_propose_cases_then_confirm_persists_with_auto_key() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }

    let base = spawn_app().await;
    let client = reqwest::Client::new();

    let key = format!(
        "CP{}",
        uuid::Uuid::new_v4().simple().to_string()[..5].to_uppercase()
    );
    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&serde_json::json!({ "key": key, "name": "Chat Propose" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let project_id = project["id"].as_str().unwrap();

    let _cfg: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/llm/configs"))
        .json(&serde_json::json!({
            "provider": "mock",
            "name": "Mock chat",
            "model": "mock-1",
            "is_default": true
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();

    let tools: Value = client
        .get(format!("{base}/api/v1/llm/chat-tools"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let names: Vec<&str> = tools
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert!(names.contains(&"propose_test_cases"));
    assert!(names.contains(&"propose_requirements"));
    assert!(names.contains(&"list_folders"));
    assert!(names.contains(&"search_cases"));

    let proposed: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/llm/chat"))
        .json(&serde_json::json!({
            "message": "Create a test case for password reset email flow"
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(proposed["provider"], "mock");
    assert_eq!(proposed["kind"], "propose_cases");
    assert!(!proposed["proposal"]["cases"].as_array().unwrap().is_empty());
    assert!(proposed["confirmation_prompt"]
        .as_str()
        .unwrap()
        .contains("right project and folder"));

    // Nothing persisted yet.
    let before: Value = client
        .get(format!(
            "{base}/api/v1/projects/{project_id}/cases?limit=50"
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(before["items"].as_array().unwrap().len(), 0);

    let mut proposal = proposed["proposal"].clone();
    proposal["cases"][0]["title"] = serde_json::json!("Password reset via email");
    proposal["cases"][0]["priority"] = serde_json::json!("high");
    proposal["placement"]["new_folder_name"] = serde_json::json!("Auth flows");

    let confirmed: Value = client
        .post(format!(
            "{base}/api/v1/projects/{project_id}/llm/chat/confirm"
        ))
        .json(&serde_json::json!({
            "proposal": proposal,
            "provider": "mock",
            "model": "mock-1"
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(confirmed["cases"].as_array().unwrap().len(), 1);
    let case = &confirmed["cases"][0]["case"];
    assert_eq!(case["title"], "Password reset via email");
    assert!(case["key"].as_str().unwrap().contains('-'));
    assert!(confirmed["created_folder_id"].as_str().is_some());
    assert!(!confirmed["events"].as_array().unwrap().is_empty());
    let summaries: String = confirmed["events"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|e| e["summary"].as_str())
        .collect::<Vec<_>>()
        .join(" | ");
    assert!(summaries.contains("via chat"));
    assert!(summaries.contains("mock"));
}

#[tokio::test]
async fn chat_propose_requirements_and_clarify() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }

    let base = spawn_app().await;
    let client = reqwest::Client::new();

    let key = format!(
        "CR{}",
        uuid::Uuid::new_v4().simple().to_string()[..5].to_uppercase()
    );
    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&serde_json::json!({ "key": key, "name": "Chat Reqs" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let project_id = project["id"].as_str().unwrap();

    let req_prop: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/llm/chat"))
        .json(&serde_json::json!({
            "message": "Create a requirement for SSO login acceptance"
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(req_prop["kind"], "propose_requirements");
    assert!(!req_prop["proposal"]["requirements"]
        .as_array()
        .unwrap()
        .is_empty());

    let clarify: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/llm/chat"))
        .json(&serde_json::json!({
            "message": "Create cases — which folder or Auth folder should I use?"
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(clarify["kind"], "clarify");
    assert!(clarify["clarify_question"].as_str().unwrap().len() > 5);
}

#[tokio::test]
async fn chat_confirm_forbidden_for_developer_role() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }

    std::env::set_var("VULYK_LLM_MOCK", "1");
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
    let base = format!("http://{addr}");
    let client = reqwest::Client::new();

    let admin_email = format!("chat-admin-{}@example.com", uuid::Uuid::new_v4().simple());
    let password = "Password1!";
    let hash = vulyk_api::auth::hash_password(password).expect("hash");
    let _: uuid::Uuid = sqlx::query_scalar(
        r#"INSERT INTO users (email, display_name, password_hash, is_system_admin)
           VALUES ($1, $2, $3, TRUE)
           RETURNING id"#,
    )
    .bind(&admin_email)
    .bind("Chat Admin")
    .bind(hash)
    .fetch_one(&pool)
    .await
    .expect("insert admin");

    let admin_login: Value = client
        .post(format!("{base}/api/v1/auth/login"))
        .json(&serde_json::json!({ "email": admin_email, "password": password }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let admin_token = admin_login["token"].as_str().unwrap();

    let key = format!(
        "CD{}",
        uuid::Uuid::new_v4().simple().to_string()[..5].to_uppercase()
    );
    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .header("Authorization", format!("Bearer {admin_token}"))
        .json(&serde_json::json!({ "key": key, "name": "Chat Dev RBAC" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let project_id = project["id"].as_str().unwrap();

    let dev_email = format!("dev-{}@example.com", uuid::Uuid::new_v4().simple());
    let user: Value = client
        .post(format!("{base}/api/v1/users"))
        .header("Authorization", format!("Bearer {admin_token}"))
        .json(&serde_json::json!({
            "email": dev_email,
            "password": password,
            "display_name": "Dev User"
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let user_id = user["id"].as_str().unwrap();

    let mem = client
        .post(format!("{base}/api/v1/projects/{project_id}/memberships"))
        .header("Authorization", format!("Bearer {admin_token}"))
        .json(&serde_json::json!({
            "user_id": user_id,
            "role": "developer"
        }))
        .send()
        .await
        .unwrap();
    assert!(
        mem.status().is_success(),
        "membership upsert failed: {}",
        mem.text().await.unwrap_or_default()
    );

    let dev_login: Value = client
        .post(format!("{base}/api/v1/auth/login"))
        .json(&serde_json::json!({ "email": dev_email, "password": password }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let dev_token = dev_login["token"].as_str().unwrap();

    let proposed: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/llm/chat"))
        .header("Authorization", format!("Bearer {dev_token}"))
        .json(&serde_json::json!({
            "message": "Create a test case for checkout"
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(proposed["kind"], "propose_cases");

    let denied = client
        .post(format!(
            "{base}/api/v1/projects/{project_id}/llm/chat/confirm"
        ))
        .header("Authorization", format!("Bearer {dev_token}"))
        .json(&serde_json::json!({
            "proposal": proposed["proposal"],
            "provider": "mock",
            "model": "mock-1"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(denied.status(), 403);
}
