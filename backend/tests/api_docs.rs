//! Self-hosted API docs + OpenAPI JSON (audit F2).

use std::time::Duration;

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
            jwt_secret: "test-secret-api-docs".into(),
            jwt_ttl_hours: 12,
            session_ttl_hours: 24,
        },
        login_throttle: vulyk_api::auth::LoginThrottle::new(),
        password_policy: vulyk_api::auth::PasswordPolicy::from_env(),
    };
    let app = vulyk_api::routes::app_router(state).layer(axum::middleware::from_fn(
        vulyk_api::security_headers::security_headers_middleware,
    ));
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
async fn openapi_json_and_self_hosted_docs() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }

    let base = spawn_app().await;
    let client = reqwest::Client::new();

    let spec = client
        .get(format!("{base}/api/openapi.json"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    assert!(
        spec.headers()
            .get(reqwest::header::CONTENT_SECURITY_POLICY)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|csp| csp.contains("script-src 'self'")),
        "openapi.json should use docs CSP"
    );
    let body: serde_json::Value = spec.json().await.unwrap();
    assert!(
        body.get("openapi").and_then(|v| v.as_str()).is_some(),
        "expected openapi field"
    );
    assert!(
        body.pointer("/paths").is_some(),
        "expected paths in OpenAPI document"
    );

    let docs = client
        .get(format!("{base}/api/docs"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let csp = docs
        .headers()
        .get(reqwest::header::CONTENT_SECURITY_POLICY)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert!(
        csp.contains("script-src 'self'"),
        "docs CSP must allow same-origin scripts: {csp}"
    );
    assert!(
        !csp.contains("cdn.jsdelivr") && !csp.contains("jsdelivr"),
        "docs CSP must not allow a CDN: {csp}"
    );
    assert_eq!(csp, vulyk_api::security_headers::DOCS_CSP);
    let html = docs.text().await.unwrap();
    assert!(
        html.contains("/api/docs/assets/scalar.js"),
        "docs HTML must reference self-hosted Scalar"
    );
    assert!(
        !html.contains("cdn.jsdelivr.net") && !html.contains("https://"),
        "docs HTML must not load remote scripts"
    );
    assert!(
        !html.contains("fonts.scalar.com"),
        "docs HTML must not load Scalar font CDN"
    );
    assert!(
        html.contains("withDefaultFonts") && html.contains("false"),
        "docs must disable Scalar default fonts"
    );
    assert!(
        html.contains("hideSearch")
            && html.contains("hideTestRequestButton")
            && html.contains("bearer_auth")
            && html.contains(r#""url":"/""#),
        "docs Scalar config must enable search, Try it, bearer auth, same-origin server"
    );
    assert!(
        html.contains("\"openapi\"") || html.contains("openapi"),
        "docs HTML should embed the OpenAPI spec"
    );
    assert!(
        body.pointer("/servers/0/url").and_then(|v| v.as_str()) == Some("/"),
        "OpenAPI servers[0].url should be same-origin /"
    );
    assert!(
        body.pointer("/components/securitySchemes/bearer_auth")
            .is_some(),
        "OpenAPI must expose bearer_auth for Scalar Auth"
    );

    let js = client
        .get(format!("{base}/api/docs/assets/scalar.js"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    assert!(
        js.headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|ct| ct.contains("javascript")),
        "scalar.js content-type"
    );
    let bytes = js.bytes().await.unwrap();
    assert!(
        bytes.len() > 100_000,
        "scalar.js too small: {}",
        bytes.len()
    );

    // JSON API paths keep the strict CSP.
    let health = client
        .get(format!("{base}/health"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let api_csp = health
        .headers()
        .get(reqwest::header::CONTENT_SECURITY_POLICY)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(api_csp, vulyk_api::security_headers::API_CSP);
}
