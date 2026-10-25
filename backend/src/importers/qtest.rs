use async_trait::async_trait;
use serde_json::Value;

use super::mapping::{draft_from_row, DraftDefaults};
use super::persist::persist_cases;
use super::{ImportContext, ImportInput, Importer};
use crate::error::{AppError, AppResult};
use crate::models::{ImportCaseDraft, ImportPreview, ImportReport, ImportStepDraft};

pub struct QtestImporter;

#[async_trait]
impl Importer for QtestImporter {
    fn id(&self) -> &'static str {
        "qtest"
    }

    fn name(&self) -> &str {
        "qTest"
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
        .map_err(|e| AppError::BadRequest(format!("qtest json: {e}")))?;
    let items = value
        .get("items")
        .or_else(|| value.get("test_cases"))
        .or_else(|| value.as_array().map(|_| &value))
        .and_then(|v| v.as_array())
        .ok_or_else(|| AppError::BadRequest("qtest: expected items array".into()))?;

    Ok(items
        .iter()
        .map(|c| {
            let mut row = c.clone();
            if let Some(obj) = row.as_object_mut() {
                if let Some(name) = c.get("name").cloned() {
                    obj.insert("title".into(), name);
                }
                if let Some(pid) = c.get("pid").cloned() {
                    obj.insert("key".into(), pid);
                }
            }
            let mut draft = draft_from_row(
                &row,
                mapping,
                DraftDefaults {
                    title: c.get("name").and_then(|v| v.as_str()).map(str::to_string),
                    external_id: c
                        .get("id")
                        .map(|v| v.to_string().trim_matches('"').to_string()),
                    ..Default::default()
                },
            );
            if draft.steps.is_empty() {
                if let Some(steps) = c.get("test_steps").and_then(|v| v.as_array()) {
                    draft.steps = steps
                        .iter()
                        .map(|s| ImportStepDraft {
                            action: s
                                .get("description")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            expected_result: s
                                .get("expected")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            data: String::new(),
                        })
                        .collect();
                }
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
    async fn previews_qtest_json() {
        let content = include_str!("../../tests/fixtures/importers/qtest.json");
        let preview = QtestImporter
            .preview(ImportInput {
                format: "json".into(),
                content: content.into(),
                field_mapping: FieldMapping::default(),
                options: serde_json::json!({}),
            })
            .await
            .expect("preview");
        assert_eq!(preview.cases[0].key.as_deref(), Some("TC-12"));
    }
}
