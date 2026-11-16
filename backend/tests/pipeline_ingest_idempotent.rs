//! Pipeline ingest campaign/iteration find-or-create is race-safe.

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

#[tokio::test]
async fn concurrent_pipeline_ingest_shares_campaign_and_iteration() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }

    let base = spawn_app().await;
    let client = reqwest::Client::new();

    let key = format!(
        "J{}",
        uuid::Uuid::new_v4().simple().to_string()[..6].to_uppercase()
    );
    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&json!({ "key": key, "name": "Jenkins Pipeline" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let project_id = project["id"].as_str().unwrap().to_string();

    let xml = r#"<?xml version="1.0"?><testsuite name="s" tests="1"><testcase name="ok" time="0.1"/></testsuite>"#;
    let body = json!({
        "format": "junit",
        "content": xml,
        "source": "ci",
        "run_name": "pipeline-ingest",
        "campaign": "CI",
        "iteration": "pipeline-ingest",
        "created_by_name": "jenkins",
    });

    let a = client
        .post(format!("{base}/api/v1/projects/{project_id}/ingest"))
        .json(&body)
        .send();
    let b = client
        .post(format!("{base}/api/v1/projects/{project_id}/ingest"))
        .json(&body)
        .send();
    let (ra, rb) = tokio::join!(a, b);
    let ra = ra
        .unwrap()
        .error_for_status()
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    let rb = rb
        .unwrap()
        .error_for_status()
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();

    assert_eq!(ra["campaign_id"], rb["campaign_id"]);
    assert_eq!(ra["iteration_id"], rb["iteration_id"]);
    assert_ne!(
        ra["run_id"], rb["run_id"],
        "each ingest still gets its own suite run"
    );

    let iteration_id = ra["iteration_id"].as_str().unwrap();
    let iteration: Value = client
        .get(format!(
            "{base}/api/v1/projects/{project_id}/iterations/{iteration_id}"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_ne!(
        iteration["status"], "Running",
        "empty-item iteration must not stay Running after ingest sync"
    );
}
