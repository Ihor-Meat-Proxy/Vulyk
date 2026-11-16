//! Display id / slug allocation + resolve helpers.

use std::time::Duration;

use chrono::Utc;
use serde_json::Value;
use uuid::Uuid;

async fn spawn_app() -> (String, sqlx::PgPool) {
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
    (format!("http://{addr}"), pool)
}

fn uniq(prefix: &str) -> String {
    format!(
        "{prefix}{}",
        Uuid::new_v4().simple().to_string()[..6].to_uppercase()
    )
}

#[tokio::test]
async fn allocate_run_ids_unique_under_concurrency() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let (base, pool) = spawn_app().await;
    let client = reqwest::Client::new();
    let key = uniq("DR");

    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&serde_json::json!({ "key": key, "name": "Display Run Demo" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let project_id = Uuid::parse_str(project["id"].as_str().unwrap()).unwrap();
    let prefix = project["case_key_prefix"].as_str().unwrap().to_string();

    let mut handles = Vec::new();
    for _ in 0..8 {
        let pool = pool.clone();
        handles.push(tokio::spawn(async move {
            let mut tx = pool.begin().await.unwrap();
            let ids = vulyk_api::services::display_ids::allocate_run_ids(
                &mut tx,
                project_id,
                "manual",
                Utc::now(),
            )
            .await
            .unwrap();
            tx.commit().await.unwrap();
            ids
        }));
    }
    let mut seqs = Vec::new();
    let mut display_ids = Vec::new();
    for h in handles {
        let (display_id, _slug, seq) = h.await.unwrap();
        seqs.push(seq);
        display_ids.push(display_id);
    }
    seqs.sort_unstable();
    seqs.dedup();
    assert_eq!(seqs.len(), 8, "expected 8 unique seq numbers, got {seqs:?}");
    for d in &display_ids {
        assert!(d.starts_with(&format!("{prefix}-R-")));
    }
}

#[tokio::test]
async fn delete_run_does_not_reuse_seq() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let (base, _pool) = spawn_app().await;
    let client = reqwest::Client::new();
    let key = uniq("DD");

    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&serde_json::json!({ "key": key, "name": "Delete Demo" }))
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

    let r1: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/runs"))
        .json(&serde_json::json!({ "name": "first" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let run = &r1["run"];
    assert_eq!(run["display_id"], format!("{prefix}-R-1"));
    assert_eq!(run["seq_number"], 1);
    let run_id = run["id"].as_str().unwrap();

    client
        .delete(format!("{base}/api/v1/projects/{project_id}/runs/{run_id}"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let r2: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/runs"))
        .json(&serde_json::json!({ "name": "second" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(r2["run"]["seq_number"], 2);
    assert_eq!(r2["run"]["display_id"], format!("{prefix}-R-2"));
}

#[tokio::test]
async fn resolve_by_uuid_display_id_and_slug() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let (base, pool) = spawn_app().await;
    let client = reqwest::Client::new();
    let key = uniq("RS");

    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&serde_json::json!({ "key": key, "name": "Resolve Demo" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let project_id = project["id"].as_str().unwrap();
    let project_uuid = Uuid::parse_str(project_id).unwrap();

    let created: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/runs"))
        .json(&serde_json::json!({ "name": "resolve-me", "source": "ci:jenkins" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let run = &created["run"];
    let uuid = run["id"].as_str().unwrap();
    let display_id = run["display_id"].as_str().unwrap();
    let slug = run["slug"].as_str().unwrap();
    assert!(slug.contains("-R-jenkins-"));

    // Resolve UUID via the library (avoid CodeQL cleartext-HTTP false positive on
    // loopback test traffic). HTTP paths use display_id / slug only.
    let by_uuid = vulyk_api::services::display_ids::resolve_run_id(&pool, Some(project_uuid), uuid)
        .await
        .unwrap();
    assert_eq!(by_uuid.to_string(), uuid);

    for key in [display_id, slug] {
        let got: Value = client
            .get(format!("{base}/api/v1/projects/{project_id}/runs/{key}"))
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(got["run"]["id"], uuid);
    }

    let by_global: Value = client
        .get(format!("{base}/api/v1/runs/{slug}"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(by_global["id"], uuid);

    let resolved = vulyk_api::services::display_ids::resolve_run_id(
        &pool,
        Some(project_uuid),
        &display_id.to_ascii_lowercase(),
    )
    .await
    .unwrap();
    assert_eq!(resolved.to_string(), uuid);
}

#[tokio::test]
async fn counters_stay_at_or_above_max_seq_after_allocate() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }
    let (base, pool) = spawn_app().await;
    let client = reqwest::Client::new();
    let key = uniq("BF");

    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&serde_json::json!({ "key": key, "name": "Backfill Check" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let project_id = Uuid::parse_str(project["id"].as_str().unwrap()).unwrap();

    let campaign: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/campaigns"))
        .json(&serde_json::json!({ "name": "C1" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let campaign_id = campaign["id"].as_str().unwrap();
    assert!(campaign["display_id"].as_str().unwrap().contains("-C-"));

    let iteration: Value = client
        .post(format!(
            "{base}/api/v1/projects/{project_id}/campaigns/{campaign_id}/iterations"
        ))
        .json(&serde_json::json!({ "name": "I1" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(iteration["display_id"].as_str().unwrap().contains("-IT-"));

    let run: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/runs"))
        .json(&serde_json::json!({ "name": "R1" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let r_seq = run["run"]["seq_number"].as_i64().unwrap() as i32;
    let r_display = run["run"]["display_id"].as_str().unwrap();
    let r_slug = run["run"]["slug"].as_str().unwrap();
    assert!(!r_display.is_empty());
    assert!(!r_slug.is_empty());

    let counters: (i32, i32, i32) = sqlx::query_as(
        r#"SELECT next_campaign_number, next_iteration_number, next_run_number
           FROM projects WHERE id = $1"#,
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let max_c: i32 =
        sqlx::query_scalar("SELECT COALESCE(MAX(seq_number),0) FROM campaigns WHERE project_id=$1")
            .bind(project_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let max_i: i32 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(seq_number),0) FROM iterations WHERE project_id=$1",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let max_r: i32 =
        sqlx::query_scalar("SELECT COALESCE(MAX(seq_number),0) FROM test_runs WHERE project_id=$1")
            .bind(project_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(counters.0 >= max_c);
    assert!(counters.1 >= max_i);
    assert!(counters.2 >= max_r);
    assert!(counters.2 >= r_seq);
}
