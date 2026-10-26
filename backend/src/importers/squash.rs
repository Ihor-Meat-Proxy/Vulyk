use async_trait::async_trait;
use serde_json::{json, Value};

use super::mapping::{draft_from_row, DraftDefaults};
use super::persist::persist_cases;
use super::{ImportContext, ImportInput, Importer};
use crate::error::{AppError, AppResult};
use crate::models::{ImportCaseDraft, ImportPreview, ImportReport, ImportStepDraft};

pub struct SquashImporter;

#[async_trait]
impl Importer for SquashImporter {
    fn id(&self) -> &'static str {
        "squash"
    }

    fn name(&self) -> &str {
        "Squash TM"
    }

    fn supported_formats(&self) -> &[&str] {
        &["json", "xml"]
    }

    async fn preview(&self, input: ImportInput) -> AppResult<ImportPreview> {
        let cases = match input.format.as_str() {
            "json" => parse_json(&input.content, &input.field_mapping)?,
            "xml" => parse_xml_as_cases(&input.content)?,
            other => {
                return Err(AppError::BadRequest(format!(
                    "squash unsupported format: {other}"
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
        .map_err(|e| AppError::BadRequest(format!("squash json: {e}")))?;
    let tests = value
        .get("test-cases")
        .or_else(|| value.get("testCases"))
        .or_else(|| value.as_array().map(|_| &value))
        .and_then(|v| v.as_array())
        .ok_or_else(|| AppError::BadRequest("squash: expected test-cases array".into()))?;

    Ok(tests
        .iter()
        .map(|tc| {
            let mut row = tc.clone();
            if let Some(obj) = row.as_object_mut() {
                if let Some(name) = tc.get("name").cloned() {
                    obj.insert("title".into(), name);
                }
                if let Some(ref_name) = tc.get("reference").cloned() {
                    obj.insert("key".into(), ref_name);
                }
            }
            let mut draft = draft_from_row(
                &row,
                mapping,
                DraftDefaults {
                    title: tc.get("name").and_then(|v| v.as_str()).map(str::to_string),
                    external_id: tc
                        .get("id")
                        .map(|v| v.to_string().trim_matches('"').to_string()),
                    ..Default::default()
                },
            );
            if draft.steps.is_empty() {
                if let Some(steps) = tc.get("steps").and_then(|v| v.as_array()) {
                    draft.steps = steps
                        .iter()
                        .map(|s| ImportStepDraft {
                            action: s
                                .get("action")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            expected_result: s
                                .get("expectedResult")
                                .or_else(|| s.get("expected_result"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string(),
                            data: String::new(),
                        })
                        .collect();
                }
            }
            if draft.suite_name.is_none() {
                draft.suite_name = tc
                    .pointer("/folder/name")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
            }
            draft
        })
        .collect())
}

fn parse_xml_as_cases(content: &str) -> AppResult<Vec<ImportCaseDraft>> {
    // Minimal Squash XML: <test-cases><test-case name="..." reference="..."/></test-cases>
    let mut cases = Vec::new();
    for chunk in content.split("<test-case").skip(1) {
        let name = attr(chunk, "name").unwrap_or("Untitled");
        let reference = attr(chunk, "reference");
        cases.push(ImportCaseDraft {
            key: reference.map(str::to_string),
            title: name.to_string(),
            description: String::new(),
            precondition: String::new(),
            priority: "medium".into(),
            status: "draft".into(),
            suite_name: None,
            steps: Vec::new(),
            external_id: reference.map(str::to_string),
            source_fields: json!({ "name": name }),
            custom_fields: Default::default(),
        });
    }
    if cases.is_empty() {
        return Err(AppError::BadRequest(
            "squash xml: no test-case nodes".into(),
        ));
    }
    Ok(cases)
}

fn attr<'a>(chunk: &'a str, name: &str) -> Option<&'a str> {
    let pattern = format!("{name}=\"");
    let start = chunk.find(&pattern)? + pattern.len();
    let end = chunk[start..].find('"')? + start;
    Some(&chunk[start..end])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::FieldMapping;

    #[tokio::test]
    async fn previews_squash_json() {
        let content = include_str!("../../tests/fixtures/importers/squash.json");
        let preview = SquashImporter
            .preview(ImportInput {
                format: "json".into(),
                content: content.into(),
                field_mapping: FieldMapping::default(),
                options: json!({}),
            })
            .await
            .expect("preview");
        assert_eq!(preview.cases[0].title, "Reset password");
    }
}
