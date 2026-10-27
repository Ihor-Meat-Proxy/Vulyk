use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AutomationRepoPublic {
    pub id: Uuid,
    pub name: String,
    /// Legacy singular project link (first/default); prefer `project_ids`.
    pub project_id: Option<Uuid>,
    pub project_ids: Vec<Uuid>,
    pub provider: String,
    pub remote_url: String,
    pub branch: String,
    pub subpath: String,
    pub has_token: bool,
    /// Encrypted Actions write token is set (workflow_dispatch).
    pub has_actions_token: bool,
    /// Workflow file/id for `workflow_dispatch` (empty = not configured).
    pub ci_workflow_file: String,
    pub ci_workflow_ref: String,
    pub index_status: String,
    pub last_indexed_at: Option<DateTime<Utc>>,
    pub last_index_error: Option<String>,
    pub commit_sha: Option<String>,
    pub file_count: i32,
    pub chunk_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct AutomationRepoRow {
    pub id: Uuid,
    pub name: String,
    pub project_id: Option<Uuid>,
    pub provider: String,
    pub remote_url: String,
    pub branch: String,
    pub subpath: String,
    pub token_ciphertext: Option<Vec<u8>>,
    pub token_nonce: Option<Vec<u8>>,
    pub actions_token_ciphertext: Option<Vec<u8>>,
    pub actions_token_nonce: Option<Vec<u8>>,
    pub ci_workflow_file: String,
    pub ci_workflow_ref: String,
    pub index_status: String,
    pub last_indexed_at: Option<DateTime<Utc>>,
    pub last_index_error: Option<String>,
    pub commit_sha: Option<String>,
    pub file_count: i32,
    pub chunk_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl AutomationRepoRow {
    pub fn to_public(&self, project_ids: Vec<Uuid>) -> AutomationRepoPublic {
        let project_id = project_ids.first().copied().or(self.project_id);
        AutomationRepoPublic {
            id: self.id,
            name: self.name.clone(),
            project_id,
            project_ids,
            provider: self.provider.clone(),
            remote_url: self.remote_url.clone(),
            branch: self.branch.clone(),
            subpath: self.subpath.clone(),
            has_token: self.token_ciphertext.is_some(),
            has_actions_token: self.actions_token_ciphertext.is_some(),
            ci_workflow_file: self.ci_workflow_file.clone(),
            ci_workflow_ref: self.ci_workflow_ref.clone(),
            index_status: self.index_status.clone(),
            last_indexed_at: self.last_indexed_at,
            last_index_error: self.last_index_error.clone(),
            commit_sha: self.commit_sha.clone(),
            file_count: self.file_count,
            chunk_count: self.chunk_count,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateAutomationRepo {
    pub name: String,
    pub provider: String,
    pub remote_url: String,
    #[serde(default = "default_branch")]
    pub branch: String,
    #[serde(default)]
    pub subpath: String,
    pub token: Option<String>,
    /// GitHub Actions write token (workflow_dispatch); encrypted at rest.
    pub actions_token: Option<String>,
    #[serde(default)]
    pub ci_workflow_file: Option<String>,
    #[serde(default)]
    pub ci_workflow_ref: Option<String>,
    /// Projects to attach (many-to-many).
    #[serde(default)]
    pub project_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateAutomationRepo {
    pub name: Option<String>,
    pub provider: Option<String>,
    pub remote_url: Option<String>,
    pub branch: Option<String>,
    pub subpath: Option<String>,
    /// Omit to leave unchanged; empty string clears.
    pub token: Option<String>,
    /// Omit to leave unchanged; empty string clears.
    pub actions_token: Option<String>,
    pub ci_workflow_file: Option<String>,
    pub ci_workflow_ref: Option<String>,
    pub project_ids: Option<Vec<Uuid>>,
}

/// Legacy singular upsert body (project-scoped shim).
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpsertAutomationRepo {
    pub provider: String,
    pub remote_url: String,
    #[serde(default = "default_branch")]
    pub branch: String,
    #[serde(default)]
    pub subpath: String,
    pub token: Option<String>,
    pub actions_token: Option<String>,
    pub ci_workflow_file: Option<String>,
    pub ci_workflow_ref: Option<String>,
    pub name: Option<String>,
}

fn default_branch() -> String {
    "main".into()
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SetRepoProjects {
    pub project_ids: Vec<Uuid>,
    pub default_project_id: Option<Uuid>,
}

/// Probe connectivity from unsaved form values (no repo id required).
#[derive(Debug, Deserialize, ToSchema)]
pub struct TestAutomationRepoBody {
    pub provider: String,
    pub remote_url: String,
    #[serde(default = "default_branch")]
    pub branch: String,
    #[serde(default)]
    pub subpath: String,
    pub token: Option<String>,
    /// Optional projects used for RBAC when auth is required.
    #[serde(default)]
    pub project_ids: Vec<Uuid>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AutomationRepoTestResult {
    pub ok: bool,
    pub message: String,
    pub commit_sha: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AutomationRepoIndexResult {
    pub status: String,
    pub commit_sha: Option<String>,
    pub file_count: i32,
    pub chunk_count: i32,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ContextFileRef {
    pub path: String,
    pub language: String,
    pub score: f64,
    pub reason: String,
}

/// Trigger a GitHub workflow_dispatch for selected cases / a prior suite.
#[derive(Debug, Deserialize, ToSchema)]
pub struct RunInCiRequest {
    /// Automation repo that holds the Actions write token + workflow settings.
    pub repo_id: Option<Uuid>,
    /// Case UUIDs to include (keys resolved server-side).
    #[serde(default)]
    pub case_ids: Vec<Uuid>,
    /// Explicit case keys (merged with resolved ids).
    #[serde(default)]
    pub case_keys: Vec<String>,
    /// Re-run cases from an existing suite/run.
    pub source_run_id: Option<Uuid>,
    pub run_name: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RunInCiResponse {
    pub run: crate::models::TestRun,
    pub dispatch: crate::models::CiDispatch,
    pub case_keys: Vec<String>,
}
