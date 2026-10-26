//! Campaigns, iterations, and automated-suite DTOs.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct Campaign {
    pub id: Uuid,
    pub project_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub name: String,
    pub description: String,
    pub sort_order: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateCampaign {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub parent_id: Option<Uuid>,
    #[serde(default)]
    pub sort_order: i32,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateCampaign {
    pub name: Option<String>,
    pub description: Option<String>,
    pub parent_id: Option<Option<Uuid>>,
    pub sort_order: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct Iteration {
    pub id: Uuid,
    pub project_id: Uuid,
    pub campaign_id: Uuid,
    pub name: String,
    pub reference: String,
    pub description: String,
    pub status: String,
    pub mode: String,
    pub cadence: String,
    pub scheduled_start: Option<DateTime<Utc>>,
    pub scheduled_end: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateIteration {
    pub name: String,
    #[serde(default)]
    pub reference: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_iteration_status")]
    pub status: String,
    #[serde(default = "default_iteration_mode")]
    pub mode: String,
    #[serde(default)]
    pub cadence: String,
    pub scheduled_start: Option<DateTime<Utc>>,
    pub scheduled_end: Option<DateTime<Utc>>,
    /// Optional case ids to assign into the iteration.
    #[serde(default)]
    pub case_ids: Vec<Uuid>,
    /// Optional suite ids — all cases in those suites are assigned.
    #[serde(default)]
    pub suite_ids: Vec<Uuid>,
}

fn default_iteration_status() -> String {
    "Undefined".into()
}

fn default_iteration_mode() -> String {
    "mixed".into()
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateIteration {
    pub name: Option<String>,
    pub reference: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
    pub mode: Option<String>,
    pub cadence: Option<String>,
    pub scheduled_start: Option<Option<DateTime<Utc>>>,
    pub scheduled_end: Option<Option<DateTime<Utc>>>,
    pub campaign_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct IterationItem {
    pub id: Uuid,
    pub iteration_id: Uuid,
    pub case_id: Uuid,
    pub status: String,
    pub sort_order: i32,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct IterationItemView {
    pub item: IterationItem,
    pub case_key: String,
    pub case_title: String,
    pub case_priority: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct IterationStatusCounts {
    pub ready: i64,
    pub running: i64,
    pub passed: i64,
    pub failed: i64,
    pub blocked: i64,
    pub skipped: i64,
    pub untestable: i64,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PassRatePoint {
    pub run_id: Uuid,
    pub run_name: String,
    pub created_at: DateTime<Utc>,
    pub pass_rate: f64,
    pub passed: i64,
    pub failed: i64,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct FlakyTestSummary {
    pub case_id: Uuid,
    pub case_key: String,
    pub case_title: String,
    pub flips: i64,
    pub latest_status: String,
    pub statuses: Vec<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SlowTestSummary {
    pub case_id: Uuid,
    pub case_key: String,
    pub case_title: String,
    pub duration_ms: i64,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct IterationDashboard {
    pub iteration: Iteration,
    pub campaign_name: String,
    pub campaign_path: Vec<String>,
    pub status_counts: IterationStatusCounts,
    pub pass_rate_trend: Vec<PassRatePoint>,
    pub flaky_tests: Vec<FlakyTestSummary>,
    pub slowest_tests: Vec<SlowTestSummary>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ExecutionsTreeNode {
    pub id: Uuid,
    pub kind: String,
    pub name: String,
    pub reference: Option<String>,
    pub status: Option<String>,
    pub mode: Option<String>,
    pub parent_id: Option<Uuid>,
    /// Nested campaign/iteration nodes (schema flattened to avoid recursive OpenAPI).
    #[schema(value_type = Vec<Object>)]
    pub children: Vec<ExecutionsTreeNode>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AutomatedSuite {
    pub id: Uuid,
    pub project_id: Uuid,
    pub iteration_id: Option<Uuid>,
    pub name: String,
    pub status: String,
    pub source: String,
    pub created_by_name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<i64>,
    pub pass_count: i32,
    pub fail_count: i32,
    pub skip_count: i32,
    pub blocked_count: i32,
    #[schema(value_type = Object)]
    pub environment: Value,
    #[schema(value_type = Object)]
    pub artifacts: Value,
    pub external_ref: Option<String>,
    pub suite_name: Option<String>,
    pub ci_provider: Option<String>,
    pub pipeline_url: Option<String>,
    pub job_url: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AutomatedSuitesQuery {
    pub status: Option<String>,
    pub created_by: Option<String>,
    pub q: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
    #[serde(default = "default_sort")]
    pub sort: String,
}

fn default_limit() -> i64 {
    25
}

fn default_sort() -> String {
    "created_at_desc".into()
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SuiteResultRow {
    pub result_id: Uuid,
    pub case_id: Option<Uuid>,
    pub case_key: Option<String>,
    pub case_title: Option<String>,
    pub status: String,
    pub message: String,
    pub environment: Option<String>,
    #[schema(value_type = Object)]
    pub environment_vars: Value,
    pub dataset: Option<String>,
    pub executed_at: DateTime<Utc>,
    pub duration_ms: Option<i32>,
    pub attachment_count: i64,
    pub external_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CaseExecutionHistoryItem {
    pub result_id: Uuid,
    pub path: String,
    pub plan_type: String,
    pub exec_number: i64,
    pub mode: String,
    pub status: String,
    pub executed_by: Option<String>,
    pub executed_on: DateTime<Utc>,
    pub linked_issues: i64,
    pub run_id: Uuid,
    pub iteration_id: Option<Uuid>,
    pub duration_ms: Option<i32>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CaseExecutionHistoryPage {
    pub items: Vec<CaseExecutionHistoryItem>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
    pub pass_rate: f64,
    pub sparkline: Vec<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct PipelineIngestRequest {
    /// `junit`, `vulyk_json`, `playwright_json`, or `pytest_json`.
    pub format: String,
    pub content: String,
    #[serde(default)]
    pub async_mode: bool,
    #[serde(default = "default_ci_source")]
    pub source: String,
    /// Campaign name (created if missing). Defaults to "CI".
    pub campaign: Option<String>,
    /// Iteration name (created if missing). Defaults to pipeline/ref name.
    pub iteration: Option<String>,
    pub iteration_reference: Option<String>,
    pub cadence: Option<String>,
    /// Display name for the automated suite / run.
    pub run_name: Option<String>,
    /// CI bot / token / actor name shown in the suites table.
    pub created_by_name: Option<String>,
    pub external_ref: Option<String>,
    #[serde(default)]
    #[schema(value_type = Object)]
    pub environment: Value,
    /// Artifact links: Playwright HTML report, trace, video, logs, etc.
    #[serde(default)]
    #[schema(value_type = Object)]
    pub artifacts: Value,
}

fn default_ci_source() -> String {
    "ci".into()
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PipelineIngestResponse {
    pub campaign_id: Uuid,
    pub iteration_id: Uuid,
    pub run_id: Uuid,
    pub inserted: i64,
    pub updated: i64,
    pub unmatched_case_keys: Vec<String>,
    pub job_id: Option<Uuid>,
    pub parsed: i64,
}
