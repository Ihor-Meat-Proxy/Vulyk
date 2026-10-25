//! JUnit / XUnit XML parser (pytest, Playwright, Surefire, cargo-nextest friendly).

use chrono::Utc;
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use serde_json::json;

use crate::models::BulkResultItem;

#[derive(Default)]
struct CaseBuilder {
    name: String,
    classname: String,
    time_secs: Option<f64>,
    status: String,
    message: String,
}

/// Parse JUnit XML into bulk result items.
/// Accepts `<testsuites>` / `<testsuite>` / `<testcase>` trees and concatenated documents.
pub fn parse_junit(xml: &str) -> Result<Vec<BulkResultItem>, String> {
    let trimmed = xml.trim();
    if trimmed.is_empty() {
        return Err("empty JUnit payload".into());
    }

    // cargo / some runners emit multiple XML documents concatenated.
    let chunks: Vec<String> = if trimmed.matches("<?xml").count() > 1 {
        trimmed
            .split("<?xml")
            .filter(|s| !s.trim().is_empty())
            .map(|s| format!("<?xml{s}"))
            .collect()
    } else {
        vec![trimmed.to_string()]
    };

    let mut out = Vec::new();
    for chunk in chunks {
        out.extend(parse_document(&chunk)?);
    }
    Ok(out)
}

fn parse_document(xml: &str) -> Result<Vec<BulkResultItem>, String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut results = Vec::new();
    let mut current: Option<CaseBuilder> = None;
    let mut buf = Vec::new();
    let mut capture_failure = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(e)) if e.local_name().as_ref() == b"testcase" => {
                results.push(finish_case(case_from_attrs(&e)?));
            }
            Ok(Event::Start(e)) if e.local_name().as_ref() == b"testcase" => {
                current = Some(case_from_attrs(&e)?);
            }
            Ok(Event::Empty(e))
                if matches!(e.local_name().as_ref(), b"failure" | b"error" | b"skipped") =>
            {
                apply_outcome(&mut current, e.local_name().as_ref(), &e);
            }
            Ok(Event::Start(e))
                if matches!(e.local_name().as_ref(), b"failure" | b"error" | b"skipped") =>
            {
                apply_outcome(&mut current, e.local_name().as_ref(), &e);
                capture_failure = e.local_name().as_ref() != b"skipped";
            }
            Ok(Event::Text(t)) if capture_failure => {
                if let Some(ref mut c) = current {
                    let text = t.unescape().map(|s| s.to_string()).unwrap_or_default();
                    if c.message.is_empty() {
                        c.message = text;
                    } else if !text.is_empty() {
                        c.message.push('\n');
                        c.message.push_str(&text);
                    }
                }
            }
            Ok(Event::CData(t)) if capture_failure => {
                if let Some(ref mut c) = current {
                    let text = String::from_utf8_lossy(&t).into_owned();
                    if c.message.is_empty() {
                        c.message = text;
                    } else if !text.is_empty() {
                        c.message.push('\n');
                        c.message.push_str(&text);
                    }
                }
            }
            Ok(Event::End(e)) => match e.local_name().as_ref() {
                b"testcase" => {
                    if let Some(c) = current.take() {
                        results.push(finish_case(c));
                    }
                    capture_failure = false;
                }
                b"failure" | b"error" => capture_failure = false,
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(err) => return Err(format!("invalid JUnit XML: {err}")),
            _ => {}
        }
        buf.clear();
    }
    Ok(results)
}

fn case_from_attrs(e: &quick_xml::events::BytesStart<'_>) -> Result<CaseBuilder, String> {
    let mut builder = CaseBuilder {
        status: crate::models::PASSED.into(),
        ..Default::default()
    };
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"name" => builder.name = attr_string(&attr.value),
            b"classname" => builder.classname = attr_string(&attr.value),
            b"time" => builder.time_secs = attr_string(&attr.value).parse().ok(),
            b"status" => {
                let s = attr_string(&attr.value);
                if !s.is_empty() {
                    builder.status = crate::models::normalize_result_status(&s);
                }
            }
            _ => {}
        }
    }
    Ok(builder)
}

fn apply_outcome(
    current: &mut Option<CaseBuilder>,
    tag: &[u8],
    e: &quick_xml::events::BytesStart<'_>,
) {
    let Some(c) = current.as_mut() else {
        return;
    };
    // JUnit: skipped → Skipped, error → Failed (no separate Error status).
    c.status = match tag {
        b"error" => crate::models::FAILED.into(),
        b"skipped" => crate::models::SKIPPED.into(),
        _ => crate::models::FAILED.into(),
    };
    for attr in e.attributes().flatten() {
        if attr.key.as_ref() == b"message" {
            c.message = attr_string(&attr.value);
        }
    }
}

fn finish_case(c: CaseBuilder) -> BulkResultItem {
    let external_id = if c.classname.is_empty() {
        c.name.clone()
    } else {
        format!("{}::{}", c.classname, c.name)
    };
    let duration_ms = c
        .time_secs
        .map(|t| (t * 1000.0).round() as i32)
        .filter(|&ms| ms >= 0);
    BulkResultItem {
        status: c.status,
        message: c.message,
        case_id: None,
        case_key: None,
        external_id: Some(external_id.clone()),
        title: Some(if c.classname.is_empty() {
            c.name
        } else {
            format!("{} — {}", c.classname, c.name)
        }),
        duration_ms,
        executed_at: Some(Utc::now()),
        raw: Some(json!({ "external_id": external_id })),
    }
}

fn attr_string(value: &[u8]) -> String {
    String::from_utf8_lossy(value).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{FAILED, PASSED, SKIPPED};

    #[test]
    fn parses_pytest_style_suite() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<testsuites>
  <testsuite name="pytest" tests="3" failures="1" errors="0" skipped="1">
    <testcase classname="tests.test_api" name="test_health" time="0.012"/>
    <testcase classname="tests.test_api" name="test_login" time="0.050">
      <failure message="assert 401 == 200">boom</failure>
    </testcase>
    <testcase classname="tests.test_api" name="test_skip" time="0.001">
      <skipped message="not ready"/>
    </testcase>
  </testsuite>
</testsuites>"#;
        let items = parse_junit(xml).expect("parse");
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].status, PASSED);
        assert_eq!(
            items[0].external_id.as_deref(),
            Some("tests.test_api::test_health")
        );
        assert_eq!(items[1].status, FAILED);
        assert!(items[1].message.contains("401") || items[1].message.contains("boom"));
        assert_eq!(items[2].status, SKIPPED);
    }

    #[test]
    fn maps_junit_error_to_failed() {
        let xml = r#"<?xml version="1.0"?>
<testsuite name="s" tests="1" errors="1">
  <testcase name="boom" classname="t">
    <error message="panic"/>
  </testcase>
</testsuite>"#;
        let items = parse_junit(xml).expect("parse");
        assert_eq!(items[0].status, FAILED);
    }

    #[test]
    fn parses_playwright_style() {
        let xml = r#"<?xml version="1.0"?>
<testsuites id="" name="" tests="1" failures="0" skipped="0" errors="0" time="1.2">
  <testsuite name="chromium" timestamp="2026-01-01T00:00:00.000Z" tests="1" failures="0" skipped="0" time="1.1" errors="0">
    <testcase name="landing page loads" classname="landing.spec.ts" time="1.05"/>
  </testsuite>
</testsuites>"#;
        let items = parse_junit(xml).expect("parse");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].status, PASSED);
        assert_eq!(items[0].duration_ms, Some(1050));
    }
}
