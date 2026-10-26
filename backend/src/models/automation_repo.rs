use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AutomationRepoPublic {
    pub id: Uuid,
    pub project_id: Uuid,
    pub provider: String,
    pub remote_url: String,
    pub branch: String,
    pub subpath: String,
    pub has_token: bool,
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
    pub project_id: Uuid,
    pub provider: String,
    pub remote_url: String,
    pub branch: String,
    pub subpath: String,
    pub token_ciphertext: Option<Vec<u8>>,
    pub token_nonce: Option<Vec<u8>>,
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
    pub fn to_public(&self) -> AutomationRepoPublic {
        AutomationRepoPublic {
            id: self.id,
            project_id: self.project_id,
            provider: self.provider.clone(),
            remote_url: self.remote_url.clone(),
            branch: self.branch.clone(),
            subpath: self.subpath.clone(),
            has_token: self.token_ciphertext.is_some(),
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
pub struct UpsertAutomationRepo {
    pub provider: String,
    pub remote_url: String,
    #[serde(default = "default_branch")]
    pub branch: String,
    #[serde(default)]
    pub subpath: String,
    /// Omit to leave unchanged; empty string clears.
    pub token: Option<String>,
}

fn default_branch() -> String {
    "main".into()
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
