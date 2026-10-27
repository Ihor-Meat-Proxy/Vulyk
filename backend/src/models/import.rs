use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;

/// Field mapping from source columns/keys to Vulyk case fields.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct FieldMapping {
    #[serde(default = "map_title")]
    pub title: String,
    #[serde(default = "map_key")]
    pub key: String,
    #[serde(default = "map_description")]
    pub description: String,
    #[serde(default = "map_precondition")]
    pub precondition: String,
    #[serde(default = "map_priority")]
    pub priority: String,
    #[serde(default = "map_status")]
    pub status: String,
    #[serde(default = "map_suite")]
    pub suite: String,
    #[serde(default = "map_steps")]
    pub steps: String,
    /// Map of Vulyk custom field key → source column/key.
    #[serde(default)]
    pub custom_fields: HashMap<String, String>,
}

impl Default for FieldMapping {
    fn default() -> Self {
        Self {
            title: map_title(),
            key: map_key(),
            description: map_description(),
            precondition: map_precondition(),
            priority: map_priority(),
            status: map_status(),
            suite: map_suite(),
            steps: map_steps(),
            custom_fields: HashMap::new(),
        }
    }
}

fn map_title() -> String {
    "title".into()
}
fn map_key() -> String {
    "key".into()
}
fn map_description() -> String {
    "description".into()
}
fn map_precondition() -> String {
    "precondition".into()
}
fn map_priority() -> String {
    "priority".into()
}
fn map_status() -> String {
    "status".into()
}
fn map_suite() -> String {
    "suite".into()
}
fn map_steps() -> String {
    "steps".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ImportCaseDraft {
    pub key: Option<String>,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub precondition: String,
    #[serde(default)]
    pub priority: String,
    #[serde(default)]
    pub status: String,
    pub suite_name: Option<String>,
    #[serde(default)]
    pub steps: Vec<ImportStepDraft>,
    #[serde(default)]
    pub external_id: Option<String>,
    #[serde(default)]
    pub source_fields: Value,
    /// Custom field values keyed by field definition key.
    #[serde(default)]
    pub custom_fields: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ImportStepDraft {
    pub action: String,
    #[serde(default)]
    pub expected_result: String,
    #[serde(default)]
    pub data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ImportPreview {
    pub importer_id: String,
    pub format: String,
    pub total: usize,
    pub cases: Vec<ImportCaseDraft>,
    #[serde(default)]
    pub warnings: Vec<String>,
    pub field_mapping: FieldMapping,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ImportReport {
    pub importer_id: String,
    pub created_cases: usize,
    pub created_suites: usize,
    pub skipped: usize,
    pub warnings: Vec<String>,
    pub case_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ImporterInfo {
    pub id: String,
    pub name: String,
    pub supported_formats: Vec<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ImportPreviewRequest {
    pub importer_id: String,
    pub format: String,
    /// Raw file contents (UTF-8 text or base64 for binary — Phase 1 expects UTF-8).
    pub content: String,
    #[serde(default)]
    pub field_mapping: FieldMapping,
    #[serde(default)]
    pub options: Value,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ImportRunRequest {
    pub importer_id: String,
    pub format: String,
    pub content: String,
    #[serde(default)]
    pub field_mapping: FieldMapping,
    #[serde(default)]
    pub options: Value,
    /// When true, only return preview without persisting.
    #[serde(default)]
    pub dry_run: bool,
}
