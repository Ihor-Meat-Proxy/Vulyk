use async_trait::async_trait;
use serde_json::{Map, Value};

use super::mapping::{draft_from_row, DraftDefaults};
use super::persist::persist_cases;
use super::{ImportContext, ImportInput, Importer};
use crate::error::{AppError, AppResult};
use crate::models::{ImportPreview, ImportReport};

pub struct GenericCsvImporter;

#[async_trait]
impl Importer for GenericCsvImporter {
    fn id(&self) -> &'static str {
        "csv"
    }

    fn name(&self) -> &str {
        "Generic CSV"
    }

    fn supported_formats(&self) -> &[&str] {
        &["csv"]
    }

    async fn preview(&self, input: ImportInput) -> AppResult<ImportPreview> {
        let cases = parse_csv(&input.content, &input.field_mapping)?;
        Ok(ImportPreview {
            importer_id: self.id().into(),
            format: "csv".into(),
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

fn parse_csv(
    content: &str,
    mapping: &crate::models::FieldMapping,
) -> AppResult<Vec<crate::models::ImportCaseDraft>> {
    let mut reader = csv::ReaderBuilder::new()
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(content.as_bytes());
    let headers = reader
        .headers()
        .map_err(|e| AppError::BadRequest(format!("csv headers: {e}")))?
        .clone();
    let mut out = Vec::new();
    for (idx, record) in reader.records().enumerate() {
        let record = record.map_err(|e| AppError::BadRequest(format!("csv row {idx}: {e}")))?;
        let mut map = Map::new();
        for (i, header) in headers.iter().enumerate() {
            let val = record.get(i).unwrap_or("").to_string();
            map.insert(header.to_string(), Value::String(val));
        }
        let row = Value::Object(map);
        out.push(draft_from_row(&row, mapping, DraftDefaults::default()));
    }
    if out.is_empty() {
        return Err(AppError::BadRequest("csv contained no data rows".into()));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::FieldMapping;

    #[tokio::test]
    async fn previews_generic_csv() {
        let content = include_str!("../../tests/fixtures/importers/generic.csv");
        let preview = GenericCsvImporter
            .preview(ImportInput {
                format: "csv".into(),
                content: content.into(),
                field_mapping: FieldMapping::default(),
                options: serde_json::json!({}),
            })
            .await
            .expect("preview");
        assert_eq!(preview.total, 2);
        assert_eq!(preview.cases[0].title, "Open login page");
        assert_eq!(preview.cases[0].steps.len(), 2);
    }
}
