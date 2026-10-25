//! Documented Vulyk JSON result format (`vulyk/results@v1`).

use serde::Deserialize;

use crate::models::BulkResultItem;

#[derive(Debug, Deserialize)]
pub struct VulykResultsDocument {
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub run: Option<VulykRunMeta>,
    pub results: Vec<BulkResultItem>,
}

#[derive(Debug, Deserialize)]
pub struct VulykRunMeta {
    pub name: Option<String>,
}

impl VulykResultsDocument {
    pub fn run_name(&self) -> Option<String> {
        self.run.as_ref().and_then(|r| r.name.clone())
    }
}

pub fn parse_vulyk_json(content: &str) -> Result<VulykDoc, String> {
    let doc: VulykResultsDocument =
        serde_json::from_str(content).map_err(|e| format!("invalid Vulyk JSON: {e}"))?;

    if let Some(ref fmt) = doc.format {
        if !fmt.is_empty()
            && fmt != "vulyk/results@v1"
            && fmt != "vulyk_json"
            && !fmt.starts_with("vulyk/results")
        {
            return Err(format!("unsupported Vulyk format marker '{fmt}'"));
        }
    }

    if doc.results.is_empty() {
        return Err("Vulyk JSON results array is empty".into());
    }

    for (i, item) in doc.results.iter().enumerate() {
        if item.status.trim().is_empty() {
            return Err(format!("results[{i}].status is required"));
        }
    }

    let run_name = doc.run_name();
    Ok(VulykDoc {
        results: doc
            .results
            .into_iter()
            .map(|mut item| {
                item.status = normalize_status(&item.status);
                item
            })
            .collect(),
        run_name,
    })
}

#[derive(Debug)]
pub struct VulykDoc {
    pub results: Vec<BulkResultItem>,
    pub run_name: Option<String>,
}

fn normalize_status(s: &str) -> String {
    crate::models::normalize_result_status(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_v1_document() {
        let raw = r#"{
          "format": "vulyk/results@v1",
          "run": { "name": "CI smoke" },
          "results": [
            {
              "external_id": "api::health",
              "case_key": "DEMO-1",
              "status": "passed",
              "duration_ms": 12
            },
            {
              "title": "Login",
              "status": "fail",
              "message": "timeout"
            }
          ]
        }"#;
        let doc = parse_vulyk_json(raw).expect("parse");
        assert_eq!(doc.run_name.as_deref(), Some("CI smoke"));
        assert_eq!(doc.results.len(), 2);
        assert_eq!(doc.results[1].status, crate::models::FAILED);
    }
}
