//! Outbound webhook delivery + HMAC signature integration test.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::{
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
    Router,
};
use serde_json::{json, Value};

#[derive(Clone, Default)]
struct Sink {
    bodies: Arc<Mutex<Vec<Vec<u8>>>>,
    headers: Arc<Mutex<Vec<HeaderMap>>>,
}

async fn hook(State(sink): State<Sink>, headers: HeaderMap, body: Bytes) -> StatusCode {
    sink.headers.lock().unwrap().push(headers);
    sink.bodies.lock().unwrap().push(body.to_vec());
    StatusCode::OK
}

async fn spawn_sink() -> (String, Sink) {
    let sink = Sink::default();
    let app = Router::new()
        .route("/hook", post(hook))
        .with_state(sink.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("serve sink");
    });
    tokio::time::sleep(Duration::from_millis(30)).await;
    (format!("http://{addr}/hook"), sink)
}

#[tokio::test]
async fn delivers_signed_webhook_payload() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }

    std::env::set_var("VULYK_EGRESS_ALLOW_PRIVATE", "1");
    let secrets = vulyk_api::crypto::SecretsKey::parse("test-secrets-key-outbound-wh");

    let (hook_url, sink) = spawn_sink().await;

    let pool = vulyk_api::db::connect(
        &std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for integration tests"),
    )
    .await
    .expect("connect");
    vulyk_api::db::migrate(&pool).await.expect("migrate");

    let password = format!("owh-{}", uuid::Uuid::new_v4());
    let user_id: uuid::Uuid = sqlx::query_scalar(
        r#"INSERT INTO users (email, password_hash, display_name)
           VALUES ($1, crypt($2, gen_salt('bf')), 'Outbound WH')
           RETURNING id"#,
    )
    .bind(format!("owh-{}@test.local", uuid::Uuid::new_v4()))
    .bind(&password)
    .fetch_one(&pool)
    .await
    .expect("user");

    let project_id: uuid::Uuid = sqlx::query_scalar(
        r#"INSERT INTO projects (name, key, case_key_prefix)
           VALUES ('OWH', 'OWH', 'OWH')
           RETURNING id"#,
    )
    .fetch_one(&pool)
    .await
    .expect("project");

    sqlx::query(
        r#"INSERT INTO memberships (project_id, user_id, role)
           VALUES ($1, $2, 'lead')"#,
    )
    .bind(project_id)
    .bind(user_id)
    .execute(&pool)
    .await
    .expect("membership");

    let plain_secret = vulyk_api::services::outbound_webhooks::generate_secret();
    let (ct, nonce) = secrets.encrypt(&plain_secret).expect("encrypt");

    let endpoint_id: uuid::Uuid = sqlx::query_scalar(
        r#"INSERT INTO outbound_webhook_endpoints
             (project_id, name, url, secret_ciphertext, secret_nonce, events, enabled)
           VALUES ($1, 'sink', $2, $3, $4, ARRAY['case.created']::text[], true)
           RETURNING id"#,
    )
    .bind(project_id)
    .bind(&hook_url)
    .bind(ct)
    .bind(nonce)
    .fetch_one(&pool)
    .await
    .expect("endpoint");

    let data = json!({ "id": uuid::Uuid::new_v4(), "key": "OWH-1", "title": "Demo" });
    vulyk_api::services::outbound_webhooks::enqueue_event(
        &pool,
        project_id,
        vulyk_api::services::outbound_webhooks::EVENT_CASE_CREATED,
        data,
    )
    .await;

    for _ in 0..40 {
        let status: Option<String> = sqlx::query_scalar(
            "SELECT status FROM outbound_webhook_deliveries WHERE endpoint_id = $1 ORDER BY created_at DESC LIMIT 1",
        )
        .bind(endpoint_id)
        .fetch_optional(&pool)
        .await
        .expect("status");
        if status.as_deref() == Some("succeeded") {
            break;
        }
        let _ = vulyk_api::jobs::process_one(&pool, &secrets, "owh-test").await;
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let status: String = sqlx::query_scalar(
        "SELECT status FROM outbound_webhook_deliveries WHERE endpoint_id = $1 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(endpoint_id)
    .fetch_one(&pool)
    .await
    .expect("final status");
    assert_eq!(status, "succeeded");

    let bodies = sink.bodies.lock().unwrap();
    assert_eq!(bodies.len(), 1, "expected one POST");
    let body = &bodies[0];
    let parsed: Value = serde_json::from_slice(body).expect("json body");
    assert_eq!(parsed["event"], "case.created");
    assert_eq!(parsed["project_id"], project_id.to_string());

    let headers = sink.headers.lock().unwrap();
    let sig = headers[0]
        .get("x-vulyk-signature")
        .and_then(|v| v.to_str().ok())
        .expect("signature header");
    assert!(vulyk_api::integrations::github::verify_signature(
        &plain_secret,
        body,
        sig
    ));
}
