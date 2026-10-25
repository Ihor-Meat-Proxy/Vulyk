use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct TestRun {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub status: String,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateRun {
    pub name: String,
    #[serde(default = "default_run_status")]
    pub status: String,
}

fn default_run_status() -> String {
    "draft".into()
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateRun {
    pub name: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct TestResult {
    pub id: Uuid,
    pub run_id: Uuid,
    pub case_id: Option<Uuid>,
    pub status: String,
    pub message: String,
    pub source: String,
    pub raw: Option<Value>,
    pub executed_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct IngestResultItem {
    pub case_id: Option<Uuid>,
    pub case_key: Option<String>,
    pub status: String,
    #[serde(default)]
    pub message: String,
    #[serde(default = "default_source")]
    pub source: String,
    pub raw: Option<Value>,
}

fn default_source() -> String {
    "api".into()
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct BulkIngestResults {
    pub results: Vec<IngestResultItem>,
}
