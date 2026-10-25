//! Parsers for automated result formats (JUnit XML, Vulyk JSON).

mod junit;
mod vulyk_json;

pub use junit::parse_junit;
pub use vulyk_json::parse_vulyk_json;

use crate::models::BulkResultItem;

#[derive(Debug, Clone)]
pub struct ParsedIngest {
    pub results: Vec<BulkResultItem>,
    pub run_name: Option<String>,
}

pub fn parse_payload(format: &str, content: &str) -> Result<ParsedIngest, String> {
    match format {
        "junit" | "junit_xml" | "xml" => {
            let results = parse_junit(content)?;
            Ok(ParsedIngest {
                results,
                run_name: None,
            })
        }
        "vulyk_json" | "json" | "vulyk" => {
            let doc = parse_vulyk_json(content)?;
            Ok(ParsedIngest {
                results: doc.results,
                run_name: doc.run_name,
            })
        }
        other => Err(format!(
            "unsupported format '{other}'; expected junit or vulyk_json"
        )),
    }
}
