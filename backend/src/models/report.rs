use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct StatusCounts {
    pub total: i64,
    pub passed: i64,
    pub failed: i64,
    pub skipped: i64,
    pub blocked: i64,
    pub error: i64,
    pub untested: i64,
    pub retest: i64,
    pub other: i64,
}

impl StatusCounts {
    pub fn pass_rate(&self) -> f64 {
        // `error` is retained for API compat but new ingest maps JUnit error → Failed.
        let decided = self.passed + self.failed + self.error;
        if decided == 0 {
            0.0
        } else {
            (self.passed as f64) / (decided as f64) * 100.0
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RunSummaryReport {
    pub run_id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub status: String,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub counts: StatusCounts,
    pub pass_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TrendPoint {
    pub bucket: String,
    pub run_id: Option<Uuid>,
    pub run_name: Option<String>,
    pub counts: StatusCounts,
    pub pass_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PassFailTrends {
    pub project_id: Uuid,
    pub granularity: String,
    pub points: Vec<TrendPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RequirementCoverageRow {
    pub requirement_id: Uuid,
    pub external_key: Option<String>,
    pub title: String,
    pub source: String,
    pub linked_cases: i64,
    pub cases_with_results: i64,
    pub cases_passed: i64,
    pub cases_failed: i64,
    pub coverage_percent: f64,
    pub pass_percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RequirementCoverageReport {
    pub project_id: Uuid,
    pub total_requirements: i64,
    pub covered_requirements: i64,
    pub coverage_percent: f64,
    pub rows: Vec<RequirementCoverageRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct FlakyCase {
    pub case_id: Uuid,
    pub case_key: String,
    pub title: String,
    pub runs_seen: i64,
    pub passes: i64,
    pub failures: i64,
    pub flake_score: f64,
    pub last_statuses: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct FlakyReport {
    pub project_id: Uuid,
    pub lookback_runs: i64,
    pub cases: Vec<FlakyCase>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ProjectDashboard {
    pub project_id: Uuid,
    pub project_key: String,
    pub project_name: String,
    pub total_cases: i64,
    pub total_runs: i64,
    pub total_requirements: i64,
    pub recent_run_summaries: Vec<RunSummaryReport>,
    pub trends: PassFailTrends,
    pub coverage: RequirementCoverageReport,
    pub flaky: FlakyReport,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct TrendsQuery {
    /// `run` (default) or `day`
    pub granularity: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct FlakyQuery {
    pub lookback_runs: Option<i64>,
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ExportQuery {
    /// `csv` (default) or `pdf`
    pub format: Option<String>,
    /// `dashboard` (default), `coverage`, `trends`, `flaky`, `run`, `run_results`
    pub report: Option<String>,
    pub run_id: Option<Uuid>,
    /// Filters for `report=run_results` (same as RunResultsQuery).
    pub status: Option<String>,
    pub folder: Option<String>,
    pub suite_id: Option<Uuid>,
    pub priority: Option<String>,
    pub assignee: Option<String>,
    pub q: Option<String>,
    pub sort: Option<String>,
}

/// Detailed run header for the reports drill-down view.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RunReportDetail {
    pub run_id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub description: String,
    pub status: String,
    /// Raw source (`manual`, `ci`, `junit`, `api`, …).
    pub source: String,
    /// Coarse kind for UI: `manual` | `automated` | `ci`.
    pub source_kind: String,
    pub external_ref: Option<String>,
    pub created_by_name: Option<String>,
    pub environment: serde_json::Value,
    pub artifacts: serde_json::Value,
    pub meta: serde_json::Value,
    pub branch: Option<String>,
    pub commit: Option<String>,
    pub pipeline_url: Option<String>,
    pub environment_label: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<i64>,
    pub counts: StatusCounts,
    pub pass_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RunResultDefect {
    pub id: Uuid,
    pub external_key: Option<String>,
    pub external_url: String,
    pub title: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RunResultAttachment {
    pub id: Uuid,
    pub filename: String,
    pub content_type: String,
    pub size_bytes: i64,
}

/// One case row in a run report (paginated).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RunCaseResultRow {
    pub case_id: Uuid,
    pub case_key: String,
    pub case_title: String,
    pub folder_path: String,
    pub suite_id: Option<Uuid>,
    pub priority: String,
    pub status: String,
    pub run_item_id: Option<Uuid>,
    pub result_id: Option<Uuid>,
    pub assignee: Option<String>,
    pub executed_by_name: Option<String>,
    pub duration_ms: Option<i64>,
    pub comment: String,
    pub attachment_count: i64,
    pub defect_count: i64,
    pub defects: Vec<RunResultDefect>,
    pub attachments: Vec<RunResultAttachment>,
    pub previous_status: Option<String>,
    /// `newly_failing` | `fixed` | `flaky` | `unchanged` | null
    pub comparison: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RunCaseResultsPage {
    pub items: Vec<RunCaseResultRow>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RunResultStepRow {
    pub position: i32,
    pub action: String,
    pub expected_result: String,
    pub actual_result: String,
    pub status: String,
    pub comment: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RunResultCommentEntry {
    pub result_id: Uuid,
    pub status: String,
    pub message: String,
    pub executed_at: DateTime<Utc>,
    pub executed_by_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RunCaseResultExpand {
    pub case_id: Uuid,
    pub run_item_id: Option<Uuid>,
    pub result_id: Option<Uuid>,
    pub steps: Vec<RunResultStepRow>,
    pub comment_history: Vec<RunResultCommentEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RunResultGroup {
    pub key: String,
    pub label: String,
    pub counts: StatusCounts,
    pub pass_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RunResultGroups {
    pub group_by: String,
    pub groups: Vec<RunResultGroup>,
}

#[derive(Debug, Deserialize, ToSchema, utoipa::IntoParams)]
pub struct RunResultsQuery {
    pub status: Option<String>,
    /// Folder path prefix (e.g. `Auth / Login`) or exact match.
    pub folder: Option<String>,
    pub suite_id: Option<Uuid>,
    pub priority: Option<String>,
    pub assignee: Option<String>,
    pub q: Option<String>,
    /// `key`, `title`, `status`, `duration`, `folder`, `priority`, `assignee`
    /// with optional `_asc` / `_desc` suffix (default `key_asc`).
    pub sort: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize, ToSchema, utoipa::IntoParams)]
pub struct RunGroupsQuery {
    /// `folder` (default) or `status`
    pub group_by: Option<String>,
    pub status: Option<String>,
    pub folder: Option<String>,
    pub suite_id: Option<Uuid>,
    pub priority: Option<String>,
    pub assignee: Option<String>,
    pub q: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CoverageLinkedCase {
    pub case_id: Uuid,
    pub case_key: String,
    pub title: String,
    pub latest_status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CoverageDetailRow {
    pub requirement_id: Uuid,
    pub external_key: Option<String>,
    pub title: String,
    pub source: String,
    pub covered: bool,
    pub linked_cases: Vec<CoverageLinkedCase>,
    pub cases_passed: i64,
    pub cases_failed: i64,
    pub pass_percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RequirementCoverageDetail {
    pub project_id: Uuid,
    pub total_requirements: i64,
    pub covered_requirements: i64,
    pub uncovered_requirements: i64,
    pub coverage_percent: f64,
    pub covered: Vec<CoverageDetailRow>,
    pub uncovered: Vec<CoverageDetailRow>,
}
