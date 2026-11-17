//! JUnit / XUnit XML parser (pytest, Playwright, Surefire, cargo-nextest friendly).
//! Recognizes Xray/TestRail-style properties: test_key, test_id, case_key, vulyk_key,
//! and automation_id / vulyk_automation_id / vulyk:automation_id.

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
    /// Captured `<system-out>` / `<system-err>` (and skip body) for key extraction.
    body_text: String,
    /// Explicit key from `<property name="test_key|test_id|case_key|vulyk_key" …>`.
    property_key: Option<String>,
    /// Explicit automation id from `<property name="automation_id|…" …>`.
    property_automation_id: Option<String>,
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
    let mut capture_text = false;
    let mut capture_to_body = false;
    let mut in_testcase = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(e)) if e.local_name().as_ref() == b"testcase" => {
                results.push(finish_case(case_from_attrs(&e)?, None));
            }
            Ok(Event::Start(e)) if e.local_name().as_ref() == b"testcase" => {
                current = Some(case_from_attrs(&e)?);
                in_testcase = true;
            }
            Ok(Event::Empty(e)) if e.local_name().as_ref() == b"property" && in_testcase => {
                apply_property(&mut current, &e);
            }
            Ok(Event::Start(e)) if e.local_name().as_ref() == b"property" && in_testcase => {
                apply_property(&mut current, &e);
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
                // Capture body text for failures/errors and skipped (key may live in body).
                capture_text = true;
                capture_to_body = e.local_name().as_ref() == b"skipped";
            }
            Ok(Event::Start(e))
                if matches!(e.local_name().as_ref(), b"system-out" | b"system-err")
                    && in_testcase =>
            {
                capture_text = true;
                capture_to_body = true;
            }
            Ok(Event::Text(t)) if capture_text => {
                if let Some(ref mut c) = current {
                    let text = t
                        .decode()
                        .ok()
                        .map(|decoded| {
                            quick_xml::escape::unescape(&decoded)
                                .map(|u| u.into_owned())
                                .unwrap_or_else(|_| decoded.into_owned())
                        })
                        .unwrap_or_default();
                    append_capture(c, &text, capture_to_body);
                }
            }
            Ok(Event::CData(t)) if capture_text => {
                if let Some(ref mut c) = current {
                    let text = String::from_utf8_lossy(&t).into_owned();
                    append_capture(c, &text, capture_to_body);
                }
            }
            Ok(Event::End(e)) => match e.local_name().as_ref() {
                b"testcase" => {
                    if let Some(c) = current.take() {
                        results.push(finish_case(c, None));
                    }
                    capture_text = false;
                    capture_to_body = false;
                    in_testcase = false;
                }
                b"failure" | b"error" | b"skipped" | b"system-out" | b"system-err" => {
                    capture_text = false;
                    capture_to_body = false;
                }
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

fn append_capture(c: &mut CaseBuilder, text: &str, to_body: bool) {
    if text.is_empty() {
        return;
    }
    if to_body {
        if !c.body_text.is_empty() {
            c.body_text.push('\n');
        }
        c.body_text.push_str(text);
        if c.message.is_empty() {
            c.message = text.to_string();
        }
    } else if c.message.is_empty() {
        c.message = text.to_string();
    } else {
        c.message.push('\n');
        c.message.push_str(text);
    }
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

fn apply_property(current: &mut Option<CaseBuilder>, e: &quick_xml::events::BytesStart<'_>) {
    let Some(c) = current.as_mut() else {
        return;
    };
    let mut name = String::new();
    let mut value = String::new();
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"name" => name = attr_string(&attr.value),
            b"value" => value = attr_string(&attr.value),
            _ => {}
        }
    }
    let key_name = name.to_ascii_lowercase();
    let v = value.trim();
    if v.is_empty() {
        return;
    }
    if matches!(
        key_name.as_str(),
        "test_key" | "test_id" | "case_key" | "vulyk_key" | "vulyk:key"
    ) {
        if c.property_key.is_none() {
            // First matching property wins; comma-separated → take first.
            let first = v.split(',').next().unwrap_or(v).trim();
            // Strip TestRail-style C prefix if present; keep Vulyk PREFIX-N as-is.
            let cleaned = first
                .strip_prefix('C')
                .filter(|s| s.chars().all(|c| c.is_ascii_digit()))
                .unwrap_or(first);
            c.property_key = Some(cleaned.to_string());
        }
    } else if matches!(
        key_name.as_str(),
        "automation_id" | "vulyk_automation_id" | "vulyk:automation_id"
    ) {
        if c.property_automation_id.is_none() {
            let first = v.split(',').next().unwrap_or(v).trim();
            if !first.is_empty() {
                c.property_automation_id.replace(first.to_string());
            }
        }
    }
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

fn finish_case(c: CaseBuilder, project_key: Option<&str>) -> BulkResultItem {
    let external_id = if c.classname.is_empty() {
        c.name.clone()
    } else {
        format!("{}::{}", c.classname, c.name)
    };
    let title = if c.classname.is_empty() {
        c.name.clone()
    } else {
        format!("{} — {}", c.classname, c.name)
    };
    let body = {
        let mut parts = Vec::new();
        if !c.message.is_empty() {
            parts.push(c.message.as_str());
        }
        if !c.body_text.is_empty() {
            parts.push(c.body_text.as_str());
        }
        let joined = parts.join("\n");
        if joined.is_empty() {
            None
        } else {
            Some(joined)
        }
    };
    let case_key = super::case_key::resolve_case_key_with_project(
        c.property_key.as_deref(),
        Some(&title),
        Some(&external_id),
        body.as_deref(),
        project_key,
    );
    let duration_ms = c
        .time_secs
        .map(|t| (t * 1000.0).round() as i32)
        .filter(|&ms| ms >= 0);
    BulkResultItem {
        status: c.status,
        message: c.message,
        case_id: None,
        case_key,
        automation_id: c.property_automation_id,
        external_id: Some(external_id.clone()),
        title: Some(title),
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

    #[test]
    fn maps_test_key_property() {
        let xml = r#"<?xml version="1.0"?>
<testsuite name="s" tests="1">
  <testcase classname="tests.Login" name="valid_password" time="1.2">
    <properties>
      <property name="test_key" value="KOC-12"/>
    </properties>
  </testcase>
</testsuite>"#;
        let items = parse_junit(xml).expect("parse");
        assert_eq!(items[0].case_key.as_deref(), Some("KOC-12"));
    }

    #[test]
    fn maps_automation_id_property_without_key_in_title() {
        let xml = r#"<?xml version="1.0"?>
<testsuite name="s" tests="1">
  <testcase classname="tests.Login" name="valid_password" time="0.4">
    <properties>
      <property name="automation_id" value="login.valid_password"/>
    </properties>
  </testcase>
</testsuite>"#;
        let items = parse_junit(xml).expect("parse");
        assert!(items[0].case_key.is_none());
        assert_eq!(
            items[0].automation_id.as_deref(),
            Some("login.valid_password")
        );

        let xml_alias = r#"<?xml version="1.0"?>
<testsuite name="s" tests="1">
  <testcase name="anonymous" time="0.1">
    <properties>
      <property name="vulyk:automation_id" value="suite::anon"/>
    </properties>
  </testcase>
</testsuite>"#;
        let items = parse_junit(xml_alias).expect("parse");
        assert_eq!(items[0].automation_id.as_deref(), Some("suite::anon"));
    }

    #[test]
    fn maps_test_id_property_like_testrail() {
        let xml = r#"<?xml version="1.0"?>
<testsuite name="s" tests="1">
  <testcase name="KOC-99 something" time="0.1">
    <properties>
      <property name="test_id" value="DEMO-3"/>
    </properties>
  </testcase>
</testsuite>"#;
        let items = parse_junit(xml).expect("parse");
        assert_eq!(items[0].case_key.as_deref(), Some("DEMO-3"));
    }

    #[test]
    fn maps_underscore_name_and_skipped_property() {
        let xml = r#"<?xml version="1.0"?>
<testsuite name="s" tests="2" skipped="1">
  <testcase classname="pkg" name="test_VQA_110_login" time="0.1"/>
  <testcase classname="pkg" name="anon_skip" time="0.0">
    <properties>
      <property name="case_key" value="VQA_111"/>
    </properties>
    <skipped message="not ready"/>
  </testcase>
</testsuite>"#;
        let items = parse_junit(xml).expect("parse");
        assert_eq!(items[0].case_key.as_deref(), Some("VQA-110"));
        assert_eq!(items[1].status, SKIPPED);
        assert_eq!(items[1].case_key.as_deref(), Some("VQA-111"));
    }

    #[test]
    fn maps_skipped_key_from_system_out() {
        let xml = r#"<?xml version="1.0"?>
<testsuite name="s" tests="1" skipped="1">
  <testcase classname="pkg" name="anonymous" time="0.0">
    <skipped/>
    <system-out>ignored until VQA_55 is ready</system-out>
  </testcase>
</testsuite>"#;
        let items = parse_junit(xml).expect("parse");
        assert_eq!(items[0].status, SKIPPED);
        assert_eq!(items[0].case_key.as_deref(), Some("VQA-55"));
    }

    #[test]
    fn maps_marker_skip_key_from_message_and_plugin_property() {
        // @pytest.mark.skip(reason="… KEY …") → message= attribute.
        let xml_msg = r#"<?xml version="1.0"?>
<testsuite name="s" tests="1" skipped="1">
  <testcase classname="pkg" name="test_health" time="0.0">
    <skipped message="KOC-12 not ready"/>
  </testcase>
</testsuite>"#;
        let items = parse_junit(xml_msg).expect("parse");
        assert_eq!(items[0].status, SKIPPED);
        assert_eq!(items[0].case_key.as_deref(), Some("KOC-12"));

        // ci/pytest_vulyk.py copies @pytest.mark.vulyk onto <properties> at collection.
        let xml_prop = r#"<?xml version="1.0"?>
<testsuite name="s" tests="1" skipped="1">
  <testcase classname="pkg" name="test_health" time="0.0">
    <properties>
      <property name="case_key" value="KOC-99"/>
    </properties>
    <skipped message="not ready"/>
  </testcase>
</testsuite>"#;
        let items = parse_junit(xml_prop).expect("parse");
        assert_eq!(items[0].status, SKIPPED);
        assert_eq!(items[0].case_key.as_deref(), Some("KOC-99"));
    }
}
