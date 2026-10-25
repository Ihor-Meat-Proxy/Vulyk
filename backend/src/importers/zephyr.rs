use async_trait::async_trait;
use serde_json::Value;

use super::mapping::{draft_from_row, normalize_priority, DraftDefaults};
use super::persist::persist_cases;
use super::{ImportContext, ImportInput, Importer};
use crate::error::{AppError, AppResult};
use crate::models::{ImportCaseDraft, ImportPreview, ImportReport, ImportStepDraft};

/// Zephyr Scale / Squad JSON export importer.
pub struct ZephyrImporter;

#[async_trait]
impl Importer for ZephyrImporter {
    fn id(&self) -> &'static str {
        "zephyr"
    }

    fn name(&self) -> &str {
        "Zephyr Scale / Squad"
    }

    fn supported_formats(&self) -> &[&str] {
        &["json"]
    }

    async fn preview(&self, input: ImportInput) -> AppResult<ImportPreview> {
        let cases = parse_json(&input.content, &input.field_mapping)?;
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
        .map_err(|e| AppError::BadRequest(format!("zephyr json: {e}")))?;
    let cases = value
        .get("testCases")
        .or_else(|| value.get("cases"))
        .or_else(|| value.as_array().map(|_| &value))
        .and_then(|v| v.as_array())
        .ok_or_else(|| AppError::BadRequest("zephyr: expected testCases array".into()))?;

    let mut out = Vec::new();
    for tc in cases {
        let mut row = tc.clone();
        if let Some(obj) = row.as_object_mut() {
            if let Some(folder) = tc.pointer("/folder/name").cloned() {
                obj.insert("folder".into(), folder);
            }
            if let Some(script) = tc.get("testScript").cloned() {
                obj.insert("testScript".into(), script);
            }
        }
        let mut draft = draft_from_row(
            &row,
            mapping,
            DraftDefaults {
                external_id: tc
                    .get("key")
                    .or_else(|| tc.get("id"))
                    .map(|v| v.to_string().trim_matches('"').to_string()),
                ..Default::default()
            },
        );
        if draft.steps.is_empty() {
            if let Some(steps) = tc.pointer("/testScript/steps").and_then(|v| v.as_array()) {
                draft.steps = steps
                    .iter()
                    .map(|s| ImportStepDraft {
                        action: s
                            .get("description")
                            .or_else(|| s.get("action"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        expected_result: s
                            .get("expectedResult")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        data: s
                            .get("testData")
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
    async fn previews_zephyr_json() {
        let content = include_str!("../../tests/fixtures/importers/zephyr.json");
        let preview = ZephyrImporter
            .preview(ImportInput {
                format: "json".into(),
                content: content.into(),
                field_mapping: default_mapping_for("zephyr"),
                options: serde_json::json!({}),
            })
            .await
            .expect("preview");
        assert_eq!(preview.cases[0].title, "Checkout with valid card");
    }
}
