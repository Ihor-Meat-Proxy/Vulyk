//! Parsers for automated result formats (JUnit XML, Vulyk JSON, Playwright, pytest).

mod case_key;
mod junit;
mod playwright_json;
mod pytest_json;
mod vulyk_json;

pub use case_key::{
    extract_case_key, extract_case_key_with_project, resolve_case_key, resolve_case_key_with_project,
};
pub use junit::parse_junit;
pub use playwright_json::parse_playwright_json;
pub use pytest_json::parse_pytest_json;
pub use vulyk_json::parse_vulyk_json;

use crate::models::BulkResultItem;

#[derive(Debug, Clone)]
pub struct ParsedIngest {
    pub results: Vec<BulkResultItem>,
    pub run_name: Option<String>,
}

pub fn parse_payload(format: &str, content: &str) -> Result<ParsedIngest, String> {
    parse_payload_with_project(format, content, None)
}

/// Parse an ingest payload and resolve case keys (including compact `{projectKey}{n}` forms).
pub fn parse_payload_with_project(
    format: &str,
    content: &str,
    project_key: Option<&str>,
) -> Result<ParsedIngest, String> {
    match format {
        "junit" | "junit_xml" | "xml" => {
            let mut results = parse_junit(content)?;
            enrich_case_keys(&mut results, project_key);
            Ok(ParsedIngest {
                results,
                run_name: None,
            })
        }
        "vulyk_json" | "json" | "vulyk" => {
            let doc = parse_vulyk_json(content)?;
            let mut results = doc.results;
            enrich_case_keys(&mut results, project_key);
            Ok(ParsedIngest {
                results,
                run_name: doc.run_name,
            })
        }
        "playwright_json" | "playwright" => {
            let mut results = parse_playwright_json(content)?;
            enrich_case_keys(&mut results, project_key);
            Ok(ParsedIngest {
                results,
                run_name: Some("Playwright".into()),
            })
        }
        "pytest_json" | "pytest" => {
            let mut results = parse_pytest_json(content)?;
            enrich_case_keys(&mut results, project_key);
            Ok(ParsedIngest {
                results,
                run_name: Some("pytest".into()),
            })
        }
        other => Err(format!(
            "unsupported format '{other}'; expected junit, vulyk_json, playwright_json, or pytest_json"
        )),
    }
}

fn enrich_case_keys(results: &mut [BulkResultItem], project_key: Option<&str>) {
    for row in results.iter_mut() {
        row.case_key = resolve_case_key_with_project(
            row.case_key.as_deref(),
            row.title.as_deref(),
            row.external_id.as_deref(),
            Some(row.message.as_str()).filter(|s| !s.is_empty()),
            project_key,
        );
    }
}
