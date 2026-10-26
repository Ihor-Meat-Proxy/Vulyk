use async_trait::async_trait;
use serde_json::{json, Value};

use super::mapping::{draft_from_row, normalize_priority, DraftDefaults};
use super::persist::persist_cases;
use super::{ImportContext, ImportInput, Importer};
use crate::error::{AppError, AppResult};
use crate::models::{ImportCaseDraft, ImportPreview, ImportReport, ImportStepDraft};

pub struct TestRailImporter;

#[async_trait]
impl Importer for TestRailImporter {
    fn id(&self) -> &'static str {
        "testrail"
    }

    fn name(&self) -> &str {
        "TestRail"
    }

    fn supported_formats(&self) -> &[&str] {
        &["csv", "xml", "json", "api"]
    }

    async fn preview(&self, input: ImportInput) -> AppResult<ImportPreview> {
        let (cases, warnings) = match input.format.as_str() {
            "csv" => (
                super::csv_generic::GenericCsvImporter
                    .preview(ImportInput {
                        format: "csv".into(),
                        content: input.content.clone(),
                        field_mapping: input.field_mapping.clone(),
                        options: input.options.clone(),
                    })
                    .await?
                    .cases,
                Vec::new(),
            ),
            "json" | "api" => (
                parse_json(&input.content, &input.field_mapping)?,
                Vec::new(),
            ),
            "xml" => (parse_xml(&input.content)?, Vec::new()),
            other => {
                return Err(AppError::BadRequest(format!(
                    "testrail unsupported format: {other}"
                )))
            }
        };
        Ok(ImportPreview {
            importer_id: self.id().into(),
            format: input.format,
            total: cases.len(),
            cases,
            warnings,
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
        .map_err(|e| AppError::BadRequest(format!("testrail json: {e}")))?;
    let cases = value
        .get("cases")
        .or_else(|| value.as_array().map(|_| &value))
        .and_then(|v| v.as_array())
        .ok_or_else(|| AppError::BadRequest("testrail: expected cases array".into()))?;

    Ok(cases
        .iter()
        .map(|c| {
            let mut draft = draft_from_row(
                c,
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
                    .map(|v| format!("TR-{}", v.to_string().trim_matches('"')));
            }
            if draft.steps.is_empty() {
                if let Some(separated) = c.get("custom_steps_separated").and_then(|v| v.as_array())
                {
                    draft.steps = separated
                        .iter()
                        .map(|s| ImportStepDraft {
                            action: s
                                .get("content")
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
            draft.priority = normalize_priority(&draft.priority);
            if draft.suite_name.is_none() {
                draft.suite_name = c
                    .get("section")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
            }
            draft
        })
        .collect())
}

fn parse_xml(content: &str) -> AppResult<Vec<ImportCaseDraft>> {
    let mut cases = Vec::new();
    for chunk in content.split("<case").skip(1) {
        let id = attr(chunk, "id").map(str::to_string);
        let title = tag_text(chunk, "title").unwrap_or_else(|| "Untitled".into());
        cases.push(ImportCaseDraft {
            key: id.clone().map(|i| format!("TR-{i}")),
            title,
            description: tag_text(chunk, "custom_steps").unwrap_or_default(),
            precondition: tag_text(chunk, "custom_preconds").unwrap_or_default(),
            priority: "medium".into(),
            status: "draft".into(),
            suite_name: tag_text(chunk, "section"),
            steps: Vec::new(),
            external_id: id,
            source_fields: json!({}),
            custom_fields: Default::default(),
        });
    }
    if cases.is_empty() {
        return Err(AppError::BadRequest("testrail xml: no case nodes".into()));
    }
    Ok(cases)
}

fn attr<'a>(chunk: &'a str, name: &str) -> Option<&'a str> {
    let pattern = format!("{name}=\"");
    let start = chunk.find(&pattern)? + pattern.len();
    let end = chunk[start..].find('"')? + start;
    Some(&chunk[start..end])
}

fn tag_text(chunk: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = chunk.find(&open)? + open.len();
    let end = chunk[start..].find(&close)? + start;
    Some(chunk[start..end].trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::importers::default_mapping_for;

    #[tokio::test]
    async fn previews_testrail_json_and_csv() {
        let json = include_str!("../../tests/fixtures/importers/testrail.json");
        let preview = TestRailImporter
            .preview(ImportInput {
                format: "json".into(),
                content: json.into(),
                field_mapping: default_mapping_for("testrail"),
                options: json!({}),
            })
            .await
            .expect("json");
        assert_eq!(preview.cases[0].title, "Verify cart totals");

        let csv = include_str!("../../tests/fixtures/importers/testrail.csv");
        let preview = TestRailImporter
            .preview(ImportInput {
                format: "csv".into(),
                content: csv.into(),
                field_mapping: default_mapping_for("testrail"),
                options: json!({}),
            })
            .await
            .expect("csv");
        assert_eq!(preview.total, 1);
    }
}
