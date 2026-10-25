use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;

use super::TestStep;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct TestRun {
    pub id: Uuid,
    pub project_id: Uuid,
    pub plan_id: Option<Uuid>,
    pub name: String,
    pub description: String,
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

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct RunItem {
    pub id: Uuid,
    pub run_id: Uuid,
    pub case_id: Uuid,
    pub assignee: Option<String>,
    pub status: String,
    pub sort_order: i32,
    pub comment: String,
    pub executed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RunItemView {
    pub item: RunItem,
    pub case_key: String,
    pub case_title: String,
    pub case_priority: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RunProgress {
    pub total: i64,
    pub untested: i64,
    pub passed: i64,
    pub failed: i64,
    pub blocked: i64,
    pub skipped: i64,
    pub retest: i64,
    pub completed: i64,
    pub percent: f64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TestRunDetail {
    pub run: TestRun,
    pub progress: RunProgress,
    pub items: Vec<RunItemView>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateRun {
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// MCP/CLI-compatible initial status (default draft; ignored when `start` is true).
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default = "default_run_source")]
    pub source: String,
    pub external_ref: Option<String>,
    #[serde(default)]
    #[schema(value_type = Object)]
    pub meta: Value,
    pub plan_id: Option<Uuid>,
    #[serde(default)]
    pub suite_ids: Vec<Uuid>,
    #[serde(default)]
    pub case_ids: Vec<Uuid>,
    #[serde(default)]
    pub case_statuses: Vec<String>,
    #[serde(default)]
    pub tag_names: Vec<String>,
    /// When true, include every project case if no other selectors are set.
    #[serde(default)]
    pub include_all_cases: bool,
    /// Start the run immediately (status=in_progress) when creating with items.
    #[serde(default)]
    pub start: bool,
}

fn default_run_source() -> String {
    "manual".into()
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateRun {
    pub name: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
    pub external_ref: Option<String>,
    #[schema(value_type = Object)]
    pub meta: Option<Value>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateRunItem {
    pub status: Option<String>,
    pub assignee: Option<String>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct TestResult {
    pub id: Uuid,
    pub run_id: Uuid,
    pub case_id: Option<Uuid>,
    pub run_item_id: Option<Uuid>,
    pub status: String,
    pub message: String,
    pub source: String,
    pub external_id: Option<String>,
    pub duration_ms: Option<i32>,
    #[schema(value_type = Option<Object>)]
    pub raw: Option<Value>,
    pub assignee: Option<String>,
    pub elapsed_ms: Option<i32>,
    /// User who recorded this result (nullable for anonymous / CI ingest).
    pub executed_by: Option<Uuid>,
    /// Case version at the time of execution.
    pub case_version: Option<i32>,
    pub executed_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

/// A result row enriched for history timelines (run name, executor, counts).
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TestResultHistoryItem {
    pub result: TestResult,
    pub run_name: Option<String>,
    pub executed_by_name: Option<String>,
    pub executed_by_email: Option<String>,
    pub attachment_count: i64,
    pub defect_count: i64,
    pub steps: Vec<ResultStep>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CaseResultsSummary {
    pub case_id: Uuid,
    pub latest_status: Option<String>,
    pub latest_executed_at: Option<DateTime<Utc>>,
    pub total_results: i64,
    pub passed: i64,
    pub failed: i64,
    pub blocked: i64,
    pub skipped: i64,
    pub retest: i64,
    /// Pass rate over decided results (Passed / (Passed + Failed)).
    pub pass_rate: f64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CaseResultsPage {
    pub summary: CaseResultsSummary,
    pub items: Vec<TestResultHistoryItem>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct ResultStep {
    pub id: Uuid,
    pub result_id: Uuid,
    pub step_id: Option<Uuid>,
    pub position: i32,
    pub action: String,
    pub expected_result: String,
    pub status: String,
    pub actual_result: String,
    pub comment: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TestResultDetail {
    pub result: TestResult,
    pub steps: Vec<ResultStep>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateResultStep {
    pub step_id: Option<Uuid>,
    pub position: i32,
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub expected_result: String,
    pub status: String,
    #[serde(default)]
    pub actual_result: String,
    #[serde(default)]
    pub comment: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateResult {
    pub case_id: Option<Uuid>,
    pub run_item_id: Option<Uuid>,
    pub status: String,
    #[serde(default)]
    pub message: String,
    #[serde(default = "default_manual")]
    pub source: String,
    pub raw: Option<Value>,
    pub assignee: Option<String>,
    pub elapsed_ms: Option<i32>,
    #[serde(default)]
    pub steps: Vec<CreateResultStep>,
    #[serde(default = "default_true")]
    pub sync_run_item: bool,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RunItemExecution {
    pub item: RunItemView,
    pub steps: Vec<TestStep>,
    pub latest_result: Option<TestResultDetail>,
    /// Chronological result history for this run item (append-only timeline).
    #[serde(default)]
    pub history: Vec<TestResultHistoryItem>,
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
    /// Deprecated: results are append-only. When true with a matching external_id,
    /// a new history row is still inserted (never overwritten). Kept for API compat.
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

fn default_true() -> bool {
    true
}

fn default_manual() -> String {
    "manual".into()
}

fn default_source() -> String {
    "api".into()
}
