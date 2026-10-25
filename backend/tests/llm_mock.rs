//! LLM + token integration tests using the mock provider (no real API keys).

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
async fn llm_config_encrypts_key_and_mock_generate() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }

    let base = spawn_app().await;
    let client = reqwest::Client::new();

    let key = format!(
        "L{}",
        uuid::Uuid::new_v4().simple().to_string()[..6].to_uppercase()
    );
    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&serde_json::json!({ "key": key, "name": "LLM Project" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let project_id = project["id"].as_str().unwrap();

    let providers: Value = client
        .get(format!("{base}/api/v1/llm/providers"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(providers.as_array().unwrap().len() >= 7);

    let cfg: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/llm/configs"))
        .json(&serde_json::json!({
            "provider": "mock",
            "name": "Mock default",
            "model": "mock-1",
            "api_key": "sk-never-leave-plaintext",
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
    assert_eq!(cfg["has_api_key"], true);
    assert!(cfg.get("api_key").is_none());

    let gen: Value = client
        .post(format!(
            "{base}/api/v1/projects/{project_id}/llm/generate-cases"
        ))
        .json(&serde_json::json!({
            "requirements_text": "Users can reset their password via email link",
            "count": 1,
            "persist": true
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(gen["provider"], "mock");
    assert!(!gen["drafts"].as_array().unwrap().is_empty());
    assert_eq!(gen["persisted"].as_array().unwrap().len(), 1);

    let case_id = gen["persisted"][0]["case"]["id"].as_str().unwrap();
    let code: Value = client
        .post(format!(
            "{base}/api/v1/projects/{project_id}/llm/generate-code"
        ))
        .json(&serde_json::json!({
            "case_ids": [case_id],
            "framework": "playwright"
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(code["code"].as_str().unwrap().contains("playwright"));

    let ask: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/llm/ask"))
        .json(&serde_json::json!({ "question": "What coverage do we have for password reset?" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(!ask["answer"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn api_token_auth_and_runs_for_mcp() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }

    let base = spawn_app().await;
    let client = reqwest::Client::new();

    let minted: Value = client
        .post(format!("{base}/api/v1/tokens"))
        .json(&serde_json::json!({ "name": "mcp-ci" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let secret = minted["secret"].as_str().unwrap().to_string();
    assert!(
        secret.starts_with("vulyk_at_") || secret.starts_with("vulk_"),
        "unexpected token prefix: {secret}"
    );

    let bad = client
        .get(format!("{base}/api/v1/projects"))
        .header("Authorization", "Bearer totally-invalid")
        .send()
        .await
        .unwrap();
    assert_eq!(bad.status(), 401);

    let key = format!(
        "R{}",
        uuid::Uuid::new_v4().simple().to_string()[..6].to_uppercase()
    );
    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .header("Authorization", format!("Bearer {secret}"))
        .json(&serde_json::json!({ "key": key, "name": "Runs" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let project_id = project["id"].as_str().unwrap();

    let run: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/runs"))
        .header("Authorization", format!("Bearer {secret}"))
        .json(&serde_json::json!({ "name": "Nightly" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let run_id = run["run"]["id"]
        .as_str()
        .or_else(|| run["id"].as_str())
        .unwrap();

    let triggered: Value = client
        .post(format!(
            "{base}/api/v1/projects/{project_id}/runs/{run_id}/trigger"
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
    assert_eq!(triggered["status"], "running");

    let results: Value = client
        .post(format!(
            "{base}/api/v1/projects/{project_id}/runs/{run_id}/results"
        ))
        .header("Authorization", format!("Bearer {secret}"))
        .json(&serde_json::json!({
            "results": [{ "status": "passed", "message": "ok", "source": "mcp" }]
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(results.as_array().unwrap().len(), 1);
}
