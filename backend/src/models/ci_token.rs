use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;

use super::TestRun;

#[derive(Debug, Clone, Serialize, sqlx::FromRow, ToSchema)]
pub struct ProjectCiTokenPublic {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub token_prefix: String,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateProjectCiToken {
    pub name: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CreateProjectCiTokenResponse {
    pub token: ProjectCiTokenPublic,
    /// Plaintext — shown once. Prefer env `VULYK_CI_TOKEN` / `VULYK_API_TOKEN`.
    pub secret: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct OpenCiRunRequest {
    /// Display name (fallback for suite_name).
    pub name: Option<String>,
    /// CI provider hint: `github`, `gitlab`, `other`.
    pub provider: Option<String>,
    /// `owner/repo` or path.
    pub repo: Option<String>,
    pub branch: Option<String>,
    pub commit: Option<String>,
    pub pipeline_url: Option<String>,
    pub job_url: Option<String>,
    pub suite_name: Option<String>,
    /// Idempotency key (e.g. `github:run_id:attempt`).
    pub external_ref: Option<String>,
    pub campaign_id: Option<Uuid>,
    pub iteration_id: Option<Uuid>,
    /// Campaign / iteration names (auto-create when ids omitted).
    pub campaign: Option<String>,
    pub iteration: Option<String>,
    pub cadence: Option<String>,
    #[serde(default)]
    #[schema(value_type = Object)]
    pub environment: Option<Value>,
    #[serde(default)]
    #[schema(value_type = Object)]
    pub artifacts: Option<Value>,
    #[serde(default)]
    #[schema(value_type = Object)]
    pub meta: Option<Value>,
    pub created_by_name: Option<String>,
    /// When true, create as `in_progress` instead of `open`.
    #[serde(default)]
    pub start: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct OpenCiRunResponse {
    pub run: TestRun,
    /// True when an existing run was returned for the same `external_ref`.
    pub idempotent: bool,
}

#[derive(Debug, Default, Deserialize, ToSchema)]
pub struct StartCiRunRequest {
    #[serde(default)]
    pub pipeline_url: Option<String>,
    #[serde(default)]
    pub job_url: Option<String>,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub commit: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CloseCiRunRequest {
    #[serde(default)]
    pub cancelled: bool,
    pub message: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CiRunResultsResponse {
    pub run_id: Uuid,
    pub status: String,
    pub inserted: i64,
    pub updated: i64,
    pub unmatched_case_keys: Vec<String>,
    pub idempotent_replays: i64,
    pub job_id: Option<Uuid>,
}
