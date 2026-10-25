use async_trait::async_trait;
use serde_json::Value;

use super::mapping::{draft_from_row, normalize_priority, DraftDefaults};
use super::persist::persist_cases;
use super::{ImportContext, ImportInput, Importer};
use crate::error::{AppError, AppResult};
use crate::models::{ImportCaseDraft, ImportPreview, ImportReport, ImportStepDraft};

pub struct XrayImporter;

#[async_trait]
impl Importer for XrayImporter {
    fn id(&self) -> &'static str {
        "xray"
    }

    fn name(&self) -> &str {
        "Jira Xray"
    }

    fn supported_formats(&self) -> &[&str] {
        &["json", "csv"]
    }

    async fn preview(&self, input: ImportInput) -> AppResult<ImportPreview> {
        let cases = match input.format.as_str() {
            "json" => parse_json(&input.content, &input.field_mapping)?,
            "csv" => {
                let mut mapping = input.field_mapping.clone();
                if mapping.title == "title" {
                    mapping.title = "Summary".into();
                    mapping.key = "Issue Key".into();
                    mapping.description = "Description".into();
                }
                super::csv_generic::GenericCsvImporter
                    .preview(ImportInput {
                        format: "csv".into(),
                        content: input.content.clone(),
                        field_mapping: mapping.clone(),
                        options: input.options.clone(),
                    })
                    .await?
                    .cases
            }
            other => {
                return Err(AppError::BadRequest(format!(
                    "xray unsupported format: {other}"
                )))
            }
        };
        Ok(ImportPreview {
            importer_id: self.id().into(),
            format: input.format,
            total: cases.len(),
            cases,
            warnings: Vec::new(),
            field_mapping: input.field_mapping,
        })
    }

    async fn run(&self, ctx: ImportContext, input: ImportInput) -> AppResult<ImportReport> {
        let preview = self.preview(input).await?;
        persist_cases(&ctx.pool, ctx.project_id, self.id(), &preview.cases).await
    }
}

fn parse_json(
    content: &str,
    mapping: &crate::models::FieldMapping,
) -> AppResult<Vec<ImportCaseDraft>> {
    let value: Value = serde_json::from_str(content)
        .map_err(|e| AppError::BadRequest(format!("xray json: {e}")))?;
    let tests = value
        .get("tests")
        .or_else(|| value.as_array().map(|_| &value))
        .and_then(|v| v.as_array())
        .ok_or_else(|| AppError::BadRequest("xray json: expected tests array".into()))?;

    let mut out = Vec::new();
    for test in tests {
        let mut row = test.clone();
        if let Some(obj) = row.as_object_mut() {
            if let Some(fields) = obj.get("fields").cloned() {
                if let Some(summary) = fields.get("summary") {
                    obj.insert("summary".into(), summary.clone());
                }
                if let Some(desc) = fields.get("description") {
                    obj.insert("description".into(), desc.clone());
                }
            }
            if let Some(key) = obj.get("key").cloned() {
                obj.insert("key".into(), key);
            }
            // Xray steps often under testScript.steps
            if obj.get("steps").is_none() {
                if let Some(steps) = test.pointer("/testScript/steps") {
                    obj.insert("steps".into(), steps.clone());
                }
            }
        }
        let mut draft = draft_from_row(
            &row,
            mapping,
            DraftDefaults {
                external_id: test.get("key").and_then(|v| v.as_str()).map(str::to_string),
                ..Default::default()
            },
        );
        if draft.steps.is_empty() {
            if let Some(steps) = test.pointer("/testScript/steps").and_then(|v| v.as_array()) {
                draft.steps = steps
                    .iter()
                    .map(|s| ImportStepDraft {
                        action: s
                            .get("action")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        expected_result: s
                            .get("result")
                            .or_else(|| s.get("expectedResult"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        data: s
                            .get("data")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                    })
                    .collect();
            }
        }
        draft.priority = normalize_priority(&draft.priority);
        out.push(draft);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::importers::default_mapping_for;

    #[tokio::test]
    async fn previews_xray_json() {
        let content = include_str!("../../tests/fixtures/importers/xray.json");
        let preview = XrayImporter
            .preview(ImportInput {
                format: "json".into(),
                content: content.into(),
                field_mapping: default_mapping_for("xray"),
                options: serde_json::json!({}),
            })
            .await
            .expect("preview");
        assert_eq!(preview.total, 1);
        assert_eq!(preview.cases[0].key.as_deref(), Some("CALC-1"));
        assert!(!preview.cases[0].steps.is_empty());
    }
}
