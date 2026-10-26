use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::{CustomFieldValue, Tag};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct TestCase {
    pub id: Uuid,
    pub project_id: Uuid,
    pub suite_id: Option<Uuid>,
    pub key: String,
    pub title: String,
    pub description: String,
    pub precondition: String,
    pub priority: String,
    pub status: String,
    pub version: i32,
    #[serde(default)]
    pub external_key: Option<String>,
    #[serde(default)]
    pub sort_order: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct TestStep {
    pub id: Uuid,
    pub case_id: Uuid,
    pub position: i32,
    pub action: String,
    pub expected_result: String,
    pub data: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TestCaseDetail {
    pub case: TestCase,
    pub steps: Vec<TestStep>,
    #[serde(default)]
    pub tags: Vec<Tag>,
    #[serde(default)]
    pub custom_fields: Vec<CustomFieldValue>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct CreateStep {
    pub action: String,
    #[serde(default)]
    pub expected_result: String,
    #[serde(default)]
    pub data: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateCase {
    /// Ignored when present — keys are allocated automatically. Kept optional for API compat.
    #[serde(default)]
    pub key: Option<String>,
    pub title: String,
    pub suite_id: Option<Uuid>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub precondition: String,
    #[serde(default = "default_priority")]
    pub priority: String,
    #[serde(default = "default_status")]
    pub status: String,
    #[serde(default)]
    pub steps: Vec<CreateStep>,
    #[serde(default)]
    pub tag_ids: Vec<Uuid>,
    #[serde(default)]
    pub tag_names: Vec<String>,
    /// Original external identifier from an import (stored separately from the Vulyk key).
    #[serde(default)]
    pub external_key: Option<String>,
    #[serde(default)]
    pub sort_order: Option<i32>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateCase {
    pub title: Option<String>,
    pub suite_id: Option<Uuid>,
    /// When true, clear suite_id (move case to project root / unfiled).
    #[serde(default)]
    pub clear_suite: bool,
    pub description: Option<String>,
    pub precondition: Option<String>,
    pub priority: Option<String>,
    pub status: Option<String>,
    pub steps: Option<Vec<CreateStep>>,
    pub tag_ids: Option<Vec<Uuid>>,
    pub tag_names: Option<Vec<String>>,
    #[serde(default)]
    pub change_summary: Option<String>,
    pub sort_order: Option<i32>,
    pub external_key: Option<String>,
}

fn default_priority() -> String {
    "medium".into()
}

fn default_status() -> String {
    "draft".into()
}
