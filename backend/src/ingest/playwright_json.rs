//! Playwright JSON reporter parser (`playwright test --reporter=json`).

use chrono::Utc;
use serde::Deserialize;
use serde_json::json;

use crate::models::BulkResultItem;

use super::case_key::extract_case_key;

#[derive(Debug, Deserialize)]
struct PlaywrightReport {
    #[serde(default)]
    suites: Vec<PwSuite>,
}

#[derive(Debug, Deserialize)]
struct PwSuite {
    #[serde(default)]
    title: String,
    #[serde(default)]
    file: String,
    #[serde(default)]
    suites: Vec<PwSuite>,
    #[serde(default)]
    specs: Vec<PwSpec>,
}

#[derive(Debug, Deserialize)]
struct PwSpec {
    #[serde(default)]
    title: String,
    #[serde(default)]
    ok: bool,
    #[serde(default)]
    tests: Vec<PwTest>,
}

#[derive(Debug, Deserialize)]
struct PwTest {
    #[serde(default)]
    status: String,
    #[serde(default)]
    results: Vec<PwResult>,
    #[serde(default, rename = "projectName")]
    project_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PwResult {
    #[serde(default)]
    status: String,
    #[serde(default)]
    duration: Option<f64>,
    #[serde(default)]
    error: Option<PwError>,
    #[serde(default, rename = "startTime")]
    #[allow(dead_code)]
    start_time: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PwError {
    #[serde(default)]
    message: String,
    #[serde(default)]
    stack: String,
}

/// Parse Playwright JSON report into bulk result items.
pub fn parse_playwright_json(content: &str) -> Result<Vec<BulkResultItem>, String> {
    let report: PlaywrightReport =
        serde_json::from_str(content).map_err(|e| format!("invalid Playwright JSON: {e}"))?;
    let mut out = Vec::new();
    walk_suites(&report.suites, "", &mut out);
    if out.is_empty() {
        return Err("Playwright JSON contained no test results".into());
    }
    Ok(out)
}

fn walk_suites(suites: &[PwSuite], prefix: &str, out: &mut Vec<BulkResultItem>) {
    for suite in suites {
        let path = if prefix.is_empty() {
            if !suite.file.is_empty() {
                suite.file.clone()
            } else {
                suite.title.clone()
            }
        } else if suite.title.is_empty() {
            prefix.to_string()
        } else {
            format!("{prefix} › {}", suite.title)
        };
        for spec in &suite.specs {
            let title = if path.is_empty() {
                spec.title.clone()
            } else {
                format!("{path} › {}", spec.title)
            };
            let (status, message, duration_ms, env) = resolve_spec(spec);
            let key = extract_case_key(&title).or_else(|| extract_case_key(&spec.title));
            out.push(BulkResultItem {
                status,
                message,
                case_id: None,
                case_key: key,
                external_id: Some(title.clone()),
                title: Some(title),
                duration_ms,
                executed_at: Some(Utc::now()),
                raw: Some(json!({
                    "reporter": "playwright",
                    "environment": env,
                    "ok": spec.ok,
                })),
            });
        }
        walk_suites(&suite.suites, &path, out);
    }
}

fn resolve_spec(spec: &PwSpec) -> (String, String, Option<i32>, String) {
    let mut status = if spec.ok {
        crate::models::PASSED.to_string()
    } else {
        crate::models::FAILED.to_string()
    };
    let mut message = String::new();
    let mut duration_ms = None;
    let mut env = String::from("Playwright");

    if let Some(test) = spec.tests.first() {
        if let Some(ref project) = test.project_name {
            if !project.is_empty() {
                env = format!("Playwright/{project}");
            }
        }
        if let Some(result) = test.results.first() {
            let raw = if !result.status.is_empty() {
                result.status.as_str()
            } else {
                test.status.as_str()
            };
            if !raw.is_empty() {
                status = crate::models::normalize_result_status(raw);
            }
            duration_ms = result.duration.map(|d| d.round() as i32);
            if let Some(err) = &result.error {
                message = if err.stack.is_empty() {
                    err.message.clone()
                } else {
                    format!("{}\n{}", err.message, err.stack)
                };
            }
        } else if !test.status.is_empty() {
            status = crate::models::normalize_result_status(&test.status);
        }
    }
    (status, message, duration_ms, env)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{FAILED, PASSED};

    #[test]
    fn parses_nested_suites_and_keys() {
        let json = r#"{
          "suites": [{
            "title": "",
            "file": "landing.spec.ts",
            "suites": [],
            "specs": [{
              "title": "KC-001 landing loads",
              "ok": true,
              "tests": [{
                "status": "expected",
                "projectName": "chromium",
                "results": [{"status": "passed", "duration": 1234}]
              }]
            }, {
              "title": "broken checkout",
              "ok": false,
              "tests": [{
                "status": "unexpected",
                "results": [{
                  "status": "failed",
                  "duration": 500,
                  "error": {"message": "boom", "stack": "at test"}
                }]
              }]
            }]
          }]
        }"#;
        let items = parse_playwright_json(json).expect("parse");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].status, PASSED);
        assert_eq!(items[0].case_key.as_deref(), Some("KC-001"));
        assert_eq!(items[1].status, FAILED);
        assert!(items[1].message.contains("boom"));
    }
}
