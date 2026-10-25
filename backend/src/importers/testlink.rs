use async_trait::async_trait;
use serde_json::json;

use super::persist::persist_cases;
use super::{ImportContext, ImportInput, Importer};
use crate::error::{AppError, AppResult};
use crate::models::{ImportCaseDraft, ImportPreview, ImportReport, ImportStepDraft};

pub struct TestLinkImporter;

#[async_trait]
impl Importer for TestLinkImporter {
    fn id(&self) -> &'static str {
        "testlink"
    }

    fn name(&self) -> &str {
        "TestLink"
    }

    fn supported_formats(&self) -> &[&str] {
        &["xml"]
    }

    async fn preview(&self, input: ImportInput) -> AppResult<ImportPreview> {
        let cases = parse_xml(&input.content)?;
        Ok(ImportPreview {
            importer_id: self.id().into(),
            format: "xml".into(),
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

fn parse_xml(content: &str) -> AppResult<Vec<ImportCaseDraft>> {
    let mut cases = Vec::new();
    for chunk in content.split("<testcase").skip(1) {
        let name = attr(chunk, "name").unwrap_or("Untitled");
        let external_id = attr(chunk, "internalid")
            .or_else(|| attr(chunk, "id"))
            .map(str::to_string);
        let summary = tag_text(chunk, "summary").unwrap_or_default();
        let preconditions = tag_text(chunk, "preconditions").unwrap_or_default();
        let mut steps = Vec::new();
        if let Some(steps_block) = tag_inner(chunk, "steps") {
            for step_chunk in steps_block.split("<step>").skip(1) {
                let action = tag_text(step_chunk, "actions").unwrap_or_default();
                let expected = tag_text(step_chunk, "expectedresults").unwrap_or_default();
                if !action.is_empty() {
                    steps.push(ImportStepDraft {
                        action,
                        expected_result: expected,
                        data: String::new(),
                    });
                }
            }
        }
        cases.push(ImportCaseDraft {
            key: external_id.clone(),
            title: name.to_string(),
            description: summary,
            precondition: preconditions,
            priority: "medium".into(),
            status: "draft".into(),
            suite_name: None,
            steps,
            external_id,
            source_fields: json!({ "name": name }),
        });
    }
    if cases.is_empty() {
        return Err(AppError::BadRequest(
            "testlink xml: no testcase nodes".into(),
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

fn tag_inner<'a>(chunk: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = chunk.find(&open)? + open.len();
    let end = chunk[start..].find(&close)? + start;
    Some(&chunk[start..end])
}

fn tag_text(chunk: &str, tag: &str) -> Option<String> {
    let inner = tag_inner(chunk, tag)?;
    let trimmed = inner
        .replace("<![CDATA[", "")
        .replace("]]>", "")
        .trim()
        .to_string();
    Some(trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::FieldMapping;

    #[tokio::test]
    async fn previews_testlink_xml() {
        let content = include_str!("../../tests/fixtures/importers/testlink.xml");
        let preview = TestLinkImporter
            .preview(ImportInput {
                format: "xml".into(),
                content: content.into(),
                field_mapping: FieldMapping::default(),
                options: json!({}),
            })
            .await
            .expect("preview");
        assert_eq!(preview.cases[0].title, "User login");
        assert_eq!(preview.cases[0].steps.len(), 2);
    }
}
