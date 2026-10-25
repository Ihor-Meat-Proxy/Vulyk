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
    /// `dashboard` (default), `coverage`, `trends`, `flaky`, `run`
    pub report: Option<String>,
    pub run_id: Option<Uuid>,
}
