//! pytest JSON / pytest-json-report style parser.

use chrono::Utc;
use serde::Deserialize;
use serde_json::json;

use crate::models::BulkResultItem;

use super::case_key::extract_case_key;

#[derive(Debug, Deserialize)]
struct PytestReport {
    #[serde(default)]
    tests: Vec<PytestTest>,
    #[serde(default)]
    #[allow(dead_code)]
    created: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct PytestTest {
    #[serde(default)]
    nodeid: String,
    #[serde(default)]
    outcome: String,
    #[serde(default)]
    call: Option<PytestCall>,
    #[serde(default)]
    keywords: Vec<String>,
    #[serde(default)]
    metadata: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct PytestCall {
    #[serde(default)]
    duration: Option<f64>,
    #[serde(default)]
    longrepr: Option<serde_json::Value>,
}

/// Parse pytest-json-report (or similar) into bulk result items.
pub fn parse_pytest_json(content: &str) -> Result<Vec<BulkResultItem>, String> {
    let report: PytestReport =
        serde_json::from_str(content).map_err(|e| format!("invalid pytest JSON: {e}"))?;
    if report.tests.is_empty() {
        // Also accept a bare array of tests.
        if let Ok(tests) = serde_json::from_str::<Vec<PytestTest>>(content) {
            return Ok(tests.into_iter().map(to_item).collect());
        }
        return Err("pytest JSON contained no tests".into());
    }
    Ok(report.tests.into_iter().map(to_item).collect())
}

fn to_item(t: PytestTest) -> BulkResultItem {
    let status = crate::models::normalize_result_status(&t.outcome);
    let message = t
        .call
        .as_ref()
        .and_then(|c| c.longrepr.as_ref())
        .map(|v| match v {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        })
        .unwrap_or_default();
    let duration_ms = t
        .call
        .as_ref()
        .and_then(|c| c.duration)
        .map(|d| (d * 1000.0).round() as i32);
    let mut key = extract_case_key(&t.nodeid);
    if key.is_none() {
        for kw in &t.keywords {
            if let Some(k) = extract_case_key(kw) {
                key = Some(k);
                break;
            }
        }
    }
    if key.is_none() {
        if let Some(meta) = &t.metadata {
            if let Some(k) = meta.get("case_key").and_then(|v| v.as_str()) {
                key = Some(k.to_ascii_uppercase());
            } else if let Some(k) = meta.get("vulyk_key").and_then(|v| v.as_str()) {
                key = Some(k.to_ascii_uppercase());
            }
        }
    }
    BulkResultItem {
        status,
        message,
        case_id: None,
        case_key: key,
        external_id: Some(t.nodeid.clone()),
        title: Some(t.nodeid.clone()),
        duration_ms,
        executed_at: Some(Utc::now()),
        raw: Some(json!({ "reporter": "pytest", "keywords": t.keywords })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{FAILED, PASSED};

    #[test]
    fn parses_pytest_json_report() {
        let json = r#"{
          "created": 1.0,
          "tests": [
            {
              "nodeid": "tests/test_api.py::test_health[KC-001]",
              "outcome": "passed",
              "call": {"duration": 0.02},
              "keywords": ["KC-001"]
            },
            {
              "nodeid": "tests/test_api.py::test_login",
              "outcome": "failed",
              "call": {"duration": 0.1, "longrepr": "assert False"},
              "metadata": {"case_key": "KC-002"}
            }
          ]
        }"#;
        let items = parse_pytest_json(json).expect("parse");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].status, PASSED);
        assert_eq!(items[0].case_key.as_deref(), Some("KC-001"));
        assert_eq!(items[1].status, FAILED);
        assert_eq!(items[1].case_key.as_deref(), Some("KC-002"));
    }
}
