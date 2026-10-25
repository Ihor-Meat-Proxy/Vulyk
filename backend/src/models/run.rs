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
    pub source: String,
    pub external_ref: Option<String>,
    #[schema(value_type = Object)]
    pub meta: Value,
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
    #[serde(default = "default_run_source")]
    pub source: String,
    pub external_ref: Option<String>,
    #[serde(default)]
    #[schema(value_type = Object)]
    pub meta: Value,
}

fn default_run_status() -> String {
    "draft".into()
}

fn default_run_source() -> String {
    "manual".into()
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateRun {
    pub name: Option<String>,
    pub status: Option<String>,
    pub external_ref: Option<String>,
    #[schema(value_type = Object)]
    pub meta: Option<Value>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct TestResult {
    pub id: Uuid,
    pub run_id: Uuid,
    pub case_id: Option<Uuid>,
    pub status: String,
    pub message: String,
    pub source: String,
    pub external_id: Option<String>,
    pub duration_ms: Option<i32>,
    #[schema(value_type = Option<Object>)]
    pub raw: Option<Value>,
    pub executed_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

/// Legacy/simple bulk item used by MCP-friendly `POST .../results`.
#[derive(Debug, Deserialize, ToSchema)]
pub struct IngestResultItem {
    pub case_id: Option<Uuid>,
    pub case_key: Option<String>,
    pub status: String,
    #[serde(default)]
    pub message: String,
    #[serde(default = "default_source")]
    pub source: String,
    pub external_id: Option<String>,
    pub duration_ms: Option<i32>,
    pub raw: Option<Value>,
}

fn default_source() -> String {
    "api".into()
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct BulkIngestResults {
    pub results: Vec<IngestResultItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BulkResultItem {
    pub status: String,
    #[serde(default)]
    pub message: String,
    pub case_id: Option<Uuid>,
    pub case_key: Option<String>,
    pub external_id: Option<String>,
    pub title: Option<String>,
    pub duration_ms: Option<i32>,
    pub executed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    #[schema(value_type = Option<Object>)]
    pub raw: Option<Value>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct BulkResultsRequest {
    pub results: Vec<BulkResultItem>,
    /// When true, replace existing results with the same external_id.
    #[serde(default)]
    pub upsert: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BulkResultsResponse {
    pub inserted: i64,
    pub updated: i64,
    pub unmatched_case_keys: Vec<String>,
    pub job_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct IngestRequest {
    /// `junit` or `vulyk_json`
    pub format: String,
    /// Raw payload (XML or JSON text). Prefer multipart/file via CLI for large bodies.
    pub content: String,
    /// When true (or payload is large), enqueue a background job.
    #[serde(default)]
    pub async_mode: bool,
    #[serde(default = "default_ingest_source")]
    pub source: String,
}

fn default_ingest_source() -> String {
    "junit".into()
}

#[derive(Debug, Serialize, ToSchema)]
pub struct IngestResponse {
    pub inserted: i64,
    pub updated: i64,
    pub unmatched_case_keys: Vec<String>,
    pub job_id: Option<Uuid>,
    pub parsed: i64,
}
