use async_trait::async_trait;
use serde_json::{json, Value};

use super::mapping::{draft_from_row, DraftDefaults};
use super::persist::persist_cases;
use super::{ImportContext, ImportInput, Importer};
use crate::error::{AppError, AppResult};
use crate::models::{ImportCaseDraft, ImportPreview, ImportReport};

pub struct KiwiImporter;

#[async_trait]
impl Importer for KiwiImporter {
    fn id(&self) -> &'static str {
        "kiwi"
    }

    fn name(&self) -> &str {
        "Kiwi TCMS"
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
        .map_err(|e| AppError::BadRequest(format!("kiwi json: {e}")))?;
    let cases = value
        .as_array()
        .or_else(|| value.get("testcases").and_then(|v| v.as_array()))
        .or_else(|| value.get("cases").and_then(|v| v.as_array()))
        .ok_or_else(|| AppError::BadRequest("kiwi: expected testcase array".into()))?;

    Ok(cases
        .iter()
        .map(|c| {
            let mut row = c.clone();
            if let Some(obj) = row.as_object_mut() {
                if let Some(summary) = c.get("summary").cloned() {
                    obj.insert("title".into(), summary);
                }
                if let Some(pk) = c.get("pk").or_else(|| c.get("id")).cloned() {
                    obj.insert("key".into(), json!(format!("KIWI-{}", pk)));
                }
                if let Some(text) = c.get("text").cloned() {
                    obj.insert("description".into(), text);
                }
                if let Some(cat) = c
                    .get("category__name")
                    .or_else(|| c.get("category"))
                    .cloned()
                {
                    obj.insert("suite".into(), cat);
                }
            }
            draft_from_row(
                &row,
                mapping,
                DraftDefaults {
                    title: c
                        .get("summary")
                        .and_then(|v| v.as_str())
                        .map(str::to_string),
                    external_id: c
                        .get("pk")
                        .or_else(|| c.get("id"))
                        .map(|v| v.to_string().trim_matches('"').to_string()),
                    ..Default::default()
                },
            )
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::FieldMapping;

    #[tokio::test]
    async fn previews_kiwi_json() {
        let content = include_str!("../../tests/fixtures/importers/kiwi.json");
        let preview = KiwiImporter
            .preview(ImportInput {
                format: "json".into(),
                content: content.into(),
                field_mapping: FieldMapping::default(),
                options: json!({}),
            })
            .await
            .expect("preview");
        assert_eq!(preview.cases[0].title, "Create project");
    }
}
