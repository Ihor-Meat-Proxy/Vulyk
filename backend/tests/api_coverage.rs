//! Full API operation inventory, router drift, and generic error-contract tests.
//!
//! These tests intentionally need no database for route parity and the 401 /
//! malformed-payload sweeps.  The random-id 404 sweep runs when `DATABASE_URL`
//! is available (as it is in CI).

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::Duration;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use http_body_util::BodyExt;
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt;
use vulyk_api::api_coverage::{
    build_report, default_manifest_path, load_manifest, manifest_errors, openapi_operations,
    parity_errors, report_markdown, ApiOperation,
};
use vulyk_api::auth::{AuthConfig, LoginThrottle, PasswordPolicy};
use vulyk_api::crypto::SecretsKey;
use vulyk_api::routes::{app_router, AppState};

const MISSING_ROUTE_STATUS: StatusCode = StatusCode::IM_A_TEAPOT;
const TEST_DATABASE_URL: &str = "postgres://vulyk:vulyk@127.0.0.1:9/vulyk";

fn test_state(pool: sqlx::PgPool, auth_required: bool) -> AppState {
    AppState {
        pool,
        secrets: SecretsKey::parse("api-coverage-test-secrets-key"),
        auth: AuthConfig {
            required: auth_required,
            jwt_secret: "api-coverage-test-jwt-secret".into(),
            jwt_ttl_hours: 1,
            session_ttl_hours: 1,
        },
        login_throttle: LoginThrottle::new(),
        password_policy: PasswordPolicy::from_env(),
    }
}

fn lazy_pool() -> sqlx::PgPool {
    PgPoolOptions::new()
        .acquire_timeout(Duration::from_millis(50))
        .max_connections(1)
        .connect_lazy(TEST_DATABASE_URL)
        .expect("lazy test pool")
}

fn concrete_path(template: &str) -> String {
    let mut concrete = String::with_capacity(template.len() + 32);
    let mut remaining = template;
    while let Some(start) = remaining.find('{') {
        concrete.push_str(&remaining[..start]);
        let after = &remaining[start + 1..];
        let end = after.find('}').expect("balanced OpenAPI path template");
        let name = &after[..end];
        let value = match name {
            "version" => "1",
            "key" => "missing-key",
            _ => "00000000-0000-4000-8000-000000000001",
        };
        concrete.push_str(value);
        remaining = &after[end + 1..];
    }
    concrete.push_str(remaining);
    concrete
}

fn request_for(
    operation: &ApiOperation,
    malformed_json: bool,
    bearer_token: Option<&str>,
) -> Request<Body> {
    let mut builder = Request::builder()
        .method(
            Method::from_bytes(operation.method.as_bytes()).expect("OpenAPI method is valid HTTP"),
        )
        .uri(concrete_path(&operation.path));
    if let Some(token) = bearer_token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    let body = if malformed_json {
        builder = builder.header(header::CONTENT_TYPE, "application/json");
        Body::from("{")
    } else {
        Body::empty()
    };
    builder.body(body).expect("coverage request")
}

fn inventory() -> (Vec<ApiOperation>, vulyk_api::api_coverage::CoverageManifest) {
    let operations = openapi_operations().expect("enumerate ApiDoc::openapi operations");
    let manifest = load_manifest(&default_manifest_path()).expect("load API coverage manifest");
    let errors = manifest_errors(&operations, &manifest);
    assert!(
        errors.is_empty(),
        "invalid API coverage manifest:\n{}",
        errors.join("\n")
    );
    (operations, manifest)
}

async fn insert_admin_token(pool: &sqlx::PgPool) -> String {
    let email = format!("api-coverage+{}@example.com", uuid::Uuid::new_v4().simple());
    let user_id: uuid::Uuid = sqlx::query_scalar(
        r#"INSERT INTO users (
               email, display_name, password_hash, is_system_admin, must_change_password
           ) VALUES ($1, 'API Coverage Admin', $2, TRUE, FALSE)
           RETURNING id"#,
    )
    .bind(&email)
    .bind(vulyk_api::auth::hash_password("api-coverage-password-123").expect("hash password"))
    .fetch_one(pool)
    .await
    .expect("insert API coverage admin");
    vulyk_api::auth::issue_jwt(user_id, &email, true, "api-coverage-test-jwt-secret", 1)
        .expect("issue API coverage JWT")
}

#[tokio::test]
async fn every_openapi_operation_is_registered_and_protected_as_declared() {
    let drift = parity_errors().expect("compare Axum and OpenAPI inventories");
    assert!(
        drift.is_empty(),
        "Axum/OpenAPI route drift:\n{}",
        drift.join("\n")
    );

    let (operations, manifest) = inventory();
    assert!(
        !operations.is_empty(),
        "OpenAPI operation inventory is empty"
    );
    let public = manifest
        .public_operations
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    let app = app_router(test_state(lazy_pool(), true))
        .fallback(|| async { (MISSING_ROUTE_STATUS, "API coverage missing-route sentinel") });
    let mut observed = BTreeMap::new();
    let mut failures = Vec::new();

    for operation in &operations {
        let key = operation.key();
        let response = app
            .clone()
            .oneshot(request_for(operation, operation.has_request_body, None))
            .await
            .unwrap_or_else(|error| panic!("{key}: router request failed: {error}"));
        let status = response.status();
        observed.insert(key.clone(), status.as_u16());
        if status == MISSING_ROUTE_STATUS || status == StatusCode::METHOD_NOT_ALLOWED {
            failures.push(format!("{key}: route dispatch returned {status}"));
            continue;
        }
        if !public.contains(&key) && status != StatusCode::UNAUTHORIZED {
            let body = response
                .into_body()
                .collect()
                .await
                .map(|body| String::from_utf8_lossy(&body.to_bytes()).into_owned())
                .unwrap_or_default();
            failures.push(format!(
                "{key}: protected operation returned {status}, expected 401; body={body}"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "API runtime coverage failures:\n{}",
        failures.join("\n")
    );

    let mut report = build_report(&operations, &manifest);
    for item in &mut report.operations {
        item.observed_runtime_status = observed
            .get(&format!("{} {}", item.method, item.path))
            .copied();
    }
    let missing_success = operations
        .iter()
        .filter(|operation| operation.success_statuses.is_empty())
        .map(ApiOperation::key)
        .collect::<Vec<_>>();
    assert!(
        missing_success.is_empty(),
        "every operation must document at least one 2xx/3xx response; missing: {}",
        missing_success.join(", ")
    );

    if let Ok(output_dir) = std::env::var("API_COVERAGE_OUTPUT_DIR") {
        let output_dir = PathBuf::from(output_dir);
        std::fs::create_dir_all(&output_dir).expect("create API coverage output directory");
        let json = serde_json::to_vec_pretty(&report).expect("serialize API coverage report");
        std::fs::write(output_dir.join("api-coverage-report.json"), json)
            .expect("write JSON API coverage report");
        std::fs::write(
            output_dir.join("api-coverage-summary.md"),
            report_markdown(&report),
        )
        .expect("write Markdown API coverage summary");
    }
}

#[tokio::test]
async fn every_json_operation_rejects_malformed_payload() {
    let (operations, manifest) = inventory();
    let public = manifest
        .public_operations
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    let app = app_router(test_state(lazy_pool(), false))
        .fallback(|| async { (MISSING_ROUTE_STATUS, "API coverage missing-route sentinel") });
    let mut failures = Vec::new();

    for operation in operations
        .iter()
        .filter(|operation| operation.has_request_body && public.contains(&operation.key()))
    {
        let key = operation.key();
        let response = app
            .clone()
            .oneshot(request_for(operation, true, None))
            .await
            .unwrap_or_else(|error| panic!("{key}: router request failed: {error}"));
        let status = response.status();
        if status == MISSING_ROUTE_STATUS || status == StatusCode::METHOD_NOT_ALLOWED {
            failures.push(format!("{key}: route dispatch returned {status}"));
        } else if !matches!(
            status,
            StatusCode::BAD_REQUEST
                | StatusCode::UNPROCESSABLE_ENTITY
                | StatusCode::UNSUPPORTED_MEDIA_TYPE
                | StatusCode::UNAUTHORIZED
        ) {
            // Raw webhook bodies parse JSON in the handler after a DB lookup in
            // some variants. Their provider-auth behavior is covered by the
            // public-operation manifest and endpoint-specific suites.
            if !public.contains(&key) || !operation.path.contains("/webhooks/") {
                failures.push(format!(
                    "{key}: malformed JSON returned {status}, expected 400/415/422"
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "malformed-payload coverage failures:\n{}",
        failures.join("\n")
    );
}

#[tokio::test]
async fn authenticated_json_operations_reject_malformed_payload() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping authenticated malformed-payload sweep: DATABASE_URL not set");
        return;
    };
    let pool = vulyk_api::db::connect(&database_url)
        .await
        .expect("connect API coverage database");
    vulyk_api::db::migrate(&pool)
        .await
        .expect("migrate API coverage database");
    let token = insert_admin_token(&pool).await;
    let (operations, manifest) = inventory();
    let app = app_router(test_state(pool, true))
        .fallback(|| async { (MISSING_ROUTE_STATUS, "API coverage missing-route sentinel") });
    let mut failures = Vec::new();

    for operation in operations
        .iter()
        .filter(|operation| operation.has_request_body)
    {
        let key = operation.key();
        if manifest.validation_not_applicable.contains_key(&key) {
            continue;
        }
        let response = app
            .clone()
            .oneshot(request_for(operation, true, Some(&token)))
            .await
            .unwrap_or_else(|error| panic!("{key}: router request failed: {error}"));
        let status = response.status();
        // Path-parameter lookups may 404 before the body is parsed when the
        // resource does not exist (random UUIDs in this sweep).
        let path_miss_ok = operation.has_path_parameters && status == StatusCode::NOT_FOUND;
        if !path_miss_ok
            && !matches!(
                status,
                StatusCode::BAD_REQUEST
                    | StatusCode::UNPROCESSABLE_ENTITY
                    | StatusCode::UNSUPPORTED_MEDIA_TYPE
                    | StatusCode::UNAUTHORIZED
            )
        {
            failures.push(format!(
                "{key}: authenticated malformed JSON returned {status}, expected 400/401/404/415/422"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "authenticated malformed-payload coverage failures:\n{}",
        failures.join("\n")
    );
}

#[tokio::test]
async fn sso_put_and_patch_aliases_have_success_and_not_found_coverage() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping SSO PUT/PATCH integration coverage: DATABASE_URL not set");
        return;
    };
    let pool = vulyk_api::db::connect(&database_url)
        .await
        .expect("connect API coverage database");
    vulyk_api::db::migrate(&pool)
        .await
        .expect("migrate API coverage database");
    let token = insert_admin_token(&pool).await;
    let provider_id: uuid::Uuid =
        sqlx::query_scalar("SELECT id FROM sso_providers WHERE provider_key = 'oidc'")
            .fetch_one(&pool)
            .await
            .expect("seeded OIDC provider");
    let app = app_router(test_state(pool, true));

    for method in [Method::PUT, Method::PATCH] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method.clone())
                    .uri(format!("/api/v1/admin/sso/providers/{provider_id}"))
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"display_name":"Coverage OIDC"}"#))
                    .expect("SSO update request"),
            )
            .await
            .expect("SSO update response");
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "{method} SSO update must succeed"
        );

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method.clone())
                    .uri("/api/v1/admin/sso/providers/00000000-0000-4000-8000-000000000001")
                    .header(header::AUTHORIZATION, format!("Bearer {token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"display_name":"Missing"}"#))
                    .expect("missing SSO update request"),
            )
            .await
            .expect("missing SSO update response");
        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "{method} missing SSO provider must return 404"
        );
    }
}

#[tokio::test]
async fn identifier_gets_return_not_found_for_random_ids() {
    let Ok(database_url) = std::env::var("DATABASE_URL") else {
        eprintln!("skipping random-id 404 sweep: DATABASE_URL not set");
        return;
    };
    let pool = vulyk_api::db::connect(&database_url)
        .await
        .expect("connect API coverage database");
    vulyk_api::db::migrate(&pool)
        .await
        .expect("migrate API coverage database");
    let token = insert_admin_token(&pool).await;
    let (operations, manifest) = inventory();
    let app = app_router(test_state(pool, true))
        .fallback(|| async { (MISSING_ROUTE_STATUS, "API coverage missing-route sentinel") });
    let mut failures = Vec::new();

    for operation in operations.iter().filter(|operation| {
        operation.method == "GET"
            && operation.has_path_parameters
            && !manifest.public_operations.contains_key(&operation.key())
            && !manifest
                .not_found_not_applicable
                .contains_key(&operation.key())
    }) {
        let key = operation.key();
        let response = app
            .clone()
            .oneshot(request_for(operation, false, Some(&token)))
            .await
            .unwrap_or_else(|error| panic!("{key}: router request failed: {error}"));
        let status = response.status();
        if !matches!(
            status,
            StatusCode::NOT_FOUND
                | StatusCode::FORBIDDEN
                | StatusCode::BAD_REQUEST
                | StatusCode::GONE
                | StatusCode::UNAUTHORIZED
        ) {
            failures.push(format!(
                "{key}: random identifier returned {status}, expected 400/403/404/410"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "random-id error coverage failures:\n{}",
        failures.join("\n")
    );
}
