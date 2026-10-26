//! Integration tests for case PDF export.

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
async fn export_pdf_contains_key_title_cyrillic_and_mixed_results() {
    if std::env::var("DATABASE_URL").is_err() {
        eprintln!("skipping: DATABASE_URL not set");
        return;
    }

    let base = spawn_app().await;
    let client = reqwest::Client::new();

    let key = format!(
        "XP{}",
        uuid::Uuid::new_v4().simple().to_string()[..6].to_uppercase()
    );
    let project: Value = client
        .post(format!("{base}/api/v1/projects"))
        .json(&serde_json::json!({
            "key": key,
            "name": "Export PDF Project",
            "description": "case export"
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

    let case: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/cases"))
        .json(&serde_json::json!({
            "title": "Експорт кейсу в PDF",
            "description": "Перевірка кирилиці в експорті",
            "precondition": "Користувач увійшов",
            "priority": "high",
            "status": "ready",
            "external_key": "e2e:export.spec.ts",
            "steps": [
                {
                    "action": "Натиснути Export as PDF",
                    "expected_result": "Файл PDF завантажується"
                }
            ]
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
    let case_key = case["case"]["key"].as_str().unwrap();

    // Manual result
    let run: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/runs"))
        .json(&serde_json::json!({
            "name": "Manual export check",
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
            "message": "manual ok",
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    // Automated (CI) result: create a CI-sourced run and bulk-insert a result.
    let ci_run: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/runs"))
        .json(&serde_json::json!({
            "name": "CI export suite",
            "source": "junit",
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
    let ci_run_id = ci_run["run"]["id"]
        .as_str()
        .or_else(|| ci_run["id"].as_str())
        .expect("ci run id");
    // Ensure ordering: automated is newer than the manual result above.
    tokio::time::sleep(Duration::from_millis(50)).await;
    client
        .post(format!("{base}/api/v1/runs/{ci_run_id}/results:bulk"))
        .json(&serde_json::json!({
            "upsert": true,
            "results": [{
                "case_key": case_key,
                "status": "failed",
                "message": "assertion failed on кирилиця",
                "duration_ms": 1200
            }]
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    // Export by key with locale=uk and results=5
    let pdf_res = client
        .get(format!(
            "{base}/api/v1/projects/{project_id}/cases/{case_key}/export.pdf?results=5&locale=uk"
        ))
        .header("Accept-Language", "uk-UA")
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let ctype = pdf_res
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let disposition = pdf_res
        .headers()
        .get("content-disposition")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let pdf = pdf_res.bytes().await.unwrap();
    assert!(ctype.contains("application/pdf"), "ctype={ctype}");
    assert!(
        disposition.contains(case_key) && disposition.contains(".pdf"),
        "disposition={disposition}"
    );
    assert!(pdf.starts_with(b"%PDF"), "missing PDF magic");

    let text = String::from_utf8_lossy(&pdf);
    assert!(text.contains("%VULYK_EXPORT"), "missing export manifest");
    assert!(text.contains(case_key), "missing case key");
    assert!(
        text.contains("Експорт кейсу в PDF"),
        "missing Cyrillic title"
    );
    assert!(
        text.contains("Натиснути Export as PDF"),
        "missing Cyrillic step"
    );
    // Newest first: CI (automated) before manual
    let ci = text
        .find("CI export suite")
        .expect("automated run in manifest");
    let manual = text
        .find("Manual export check")
        .expect("manual run in manifest");
    assert!(ci < manual, "expected automated (newer) before manual");
    assert!(text.contains("automated"), "missing automated type");
    assert!(text.contains("manual"), "missing manual type");

    // Also by UUID path without project prefix
    let pdf2 = client
        .get(format!(
            "{base}/api/v1/cases/{case_id}/export.pdf?results=2"
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .bytes()
        .await
        .unwrap();
    assert!(pdf2.starts_with(b"%PDF"));

    // Bulk of one → PDF
    let single = client
        .post(format!("{base}/api/v1/projects/{project_id}/cases/export"))
        .json(&serde_json::json!({ "case_ids": [case_id], "results": 5, "locale": "en" }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    assert!(single
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .contains("pdf"));

    // Second case for ZIP
    let case_b: Value = client
        .post(format!("{base}/api/v1/projects/{project_id}/cases"))
        .json(&serde_json::json!({
            "title": "Second case",
            "steps": [{ "action": "A", "expected_result": "B" }]
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let case_b_id = case_b["case"]["id"].as_str().unwrap();

    let zip_res = client
        .post(format!("{base}/api/v1/projects/{project_id}/cases/export"))
        .json(&serde_json::json!({
            "case_ids": [case_id, case_b_id],
            "results": 3
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let zip_ctype = zip_res
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let zip_bytes = zip_res.bytes().await.unwrap();
    assert!(
        zip_ctype.contains("zip") || zip_bytes.starts_with(b"PK"),
        "expected zip, ctype={zip_ctype}"
    );
    assert!(zip_bytes.starts_with(b"PK"), "ZIP magic");
}
