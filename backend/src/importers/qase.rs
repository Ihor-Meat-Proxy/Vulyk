use async_trait::async_trait;
use serde_json::Value;

use super::mapping::{draft_from_row, DraftDefaults};
use super::persist::persist_cases;
use super::{ImportContext, ImportInput, Importer};
use crate::error::{AppError, AppResult};
use crate::models::{ImportCaseDraft, ImportPreview, ImportReport, ImportStepDraft};

pub struct QaseImporter;

#[async_trait]
impl Importer for QaseImporter {
    fn id(&self) -> &'static str {
        "qase"
    }

    fn name(&self) -> &str {
        "Qase"
    }

    fn supported_formats(&self) -> &[&str] {
        &["json"]
    }

    async fn preview(&self, input: ImportInput) -> AppResult<ImportPreview> {
        let cases = parse_json(&input.content, &input.field_mapping)?;
        Ok(ImportPreview {
            importer_id: self.id().into(),
            format: "json".into(),
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
        .map_err(|e| AppError::BadRequest(format!("qase json: {e}")))?;
    let entities = value
        .pointer("/result/entities")
        .or_else(|| value.get("entities"))
        .or_else(|| value.get("cases"))
        .or_else(|| value.as_array().map(|_| &value))
        .and_then(|v| v.as_array())
        .ok_or_else(|| AppError::BadRequest("qase: expected entities/cases array".into()))?;

    Ok(entities
        .iter()
        .map(|c| {
            let mut row = c.clone();
            if let Some(obj) = row.as_object_mut() {
                if let Some(title) = c.get("title").cloned() {
                    obj.insert("title".into(), title);
                }
            }
            let mut draft = draft_from_row(
                &row,
                mapping,
                DraftDefaults {
                    external_id: c
                        .get("id")
                        .map(|v| v.to_string().trim_matches('"').to_string()),
                    ..Default::default()
                },
            );
            if draft.key.is_none() {
                draft.key = c
                    .get("id")
                    .map(|v| format!("QASE-{}", v.to_string().trim_matches('"')));
            }
            if draft.steps.is_empty() {
                if let Some(steps) = c.get("steps").and_then(|v| v.as_array()) {
                    draft.steps = steps
                        .iter()
                        .map(|s| ImportStepDraft {
                            action: s
                                .get("action")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            expected_result: s
                                .get("expected_result")
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
            if draft.suite_name.is_none() {
                draft.suite_name = c
                    .get("suite_title")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
            }
            draft
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::FieldMapping;

    #[tokio::test]
    async fn previews_qase_json() {
        let content = include_str!("../../tests/fixtures/importers/qase.json");
        let preview = QaseImporter
            .preview(ImportInput {
                format: "json".into(),
                content: content.into(),
                field_mapping: FieldMapping::default(),
                options: serde_json::json!({}),
            })
            .await
            .expect("preview");
        assert_eq!(preview.cases[0].title, "API health check");
    }
}
