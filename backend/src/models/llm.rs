use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct LlmConfigPublic {
    pub id: Uuid,
    pub project_id: Option<Uuid>,
    pub provider: String,
    pub name: String,
    pub base_url: Option<String>,
    pub model: String,
    pub has_api_key: bool,
    pub extra: Value,
    pub is_default: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct LlmConfigRow {
    pub id: Uuid,
    pub project_id: Option<Uuid>,
    pub provider: String,
    pub name: String,
    pub base_url: Option<String>,
    pub model: String,
    pub api_key_ciphertext: Option<Vec<u8>>,
    pub api_key_nonce: Option<Vec<u8>>,
    pub extra: Value,
    pub is_default: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl LlmConfigRow {
    pub fn to_public(&self) -> LlmConfigPublic {
        LlmConfigPublic {
            id: self.id,
            project_id: self.project_id,
            provider: self.provider.clone(),
            name: self.name.clone(),
            base_url: self.base_url.clone(),
            model: self.model.clone(),
            has_api_key: self.api_key_ciphertext.is_some(),
            extra: self.extra.clone(),
            is_default: self.is_default,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateLlmConfig {
    pub provider: String,
    pub name: String,
    pub base_url: Option<String>,
    #[serde(default)]
    pub model: String,
    pub api_key: Option<String>,
    #[serde(default = "default_extra")]
    pub extra: Value,
    #[serde(default)]
    pub is_default: bool,
    /// When true, store as a global config (`project_id IS NULL`) available to all projects.
    #[serde(default)]
    pub global: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateLlmConfig {
    pub name: Option<String>,
    pub base_url: Option<String>,
    pub model: Option<String>,
    /// Set to empty string to clear the stored key.
    pub api_key: Option<String>,
    pub extra: Option<Value>,
    pub is_default: Option<bool>,
}

fn default_extra() -> Value {
    Value::Object(Default::default())
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct GenerateCasesRequest {
    /// Free-text requirements (used when requirement_ids is empty).
    #[serde(default)]
    pub requirements_text: String,
    #[serde(default)]
    pub requirement_ids: Vec<Uuid>,
    #[serde(default = "default_count")]
    pub count: u32,
    pub suite_id: Option<Uuid>,
    /// When true, persist generated drafts as cases (still reviewable).
    #[serde(default)]
    pub persist: bool,
    pub config_id: Option<Uuid>,
}

fn default_count() -> u32 {
    3
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct GeneratedCaseDraft {
    pub key: String,
    pub title: String,
    pub description: String,
    pub precondition: String,
    pub priority: String,
    pub steps: Vec<crate::models::CreateStep>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GenerateCasesResponse {
    pub provider: String,
    pub model: String,
    pub drafts: Vec<GeneratedCaseDraft>,
    pub persisted: Vec<crate::models::TestCaseDetail>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct GenerateCodeRequest {
    pub case_ids: Vec<Uuid>,
    #[serde(default = "default_framework")]
    pub framework: String,
    pub config_id: Option<Uuid>,
    /// When the project has multiple automation repos, select which to index.
    pub repo_id: Option<Uuid>,
}

fn default_framework() -> String {
    "playwright".into()
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GenerateCodeResponse {
    pub provider: String,
    pub model: String,
    pub framework: String,
    pub code: String,
    /// Repo files included in the LLM prompt (empty when no automation repo).
    #[serde(default)]
    pub context_files: Vec<crate::models::ContextFileRef>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AskTestsRequest {
    pub question: String,
    #[serde(default)]
    pub case_ids: Vec<Uuid>,
    pub config_id: Option<Uuid>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AskTestsResponse {
    pub provider: String,
    pub model: String,
    pub answer: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct DiffPreviewRequest {
    pub repo_id: Uuid,
    /// Compare base (e.g. `main`). Ignored when `pull_number` is set (uses PR target).
    #[serde(default)]
    pub base_ref: Option<String>,
    /// Compare head (branch or SHA). Ignored when `pull_number` is set (uses PR head).
    #[serde(default)]
    pub head_ref: Option<String>,
    /// Open PR/MR number (GitHub/Gitea/Forgejo) or MR iid (GitLab).
    pub pull_number: Option<i64>,
    #[serde(default)]
    pub include_paths: Vec<String>,
    #[serde(default)]
    pub exclude_paths: Vec<String>,
    /// Soft token budget for the preview selection (default 8000).
    pub token_budget: Option<i64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DiffPreviewResponse {
    pub base_ref: String,
    pub head_ref: String,
    pub base_sha: Option<String>,
    pub head_sha: Option<String>,
    pub pull_number: Option<i64>,
    pub pull_url: Option<String>,
    pub files: Vec<crate::services::automation_repo_diff::DiffFileEntry>,
    pub selected_files: Vec<crate::services::automation_repo_diff::DiffFileEntry>,
    pub total_approx_tokens: i64,
    pub selected_approx_tokens: i64,
    pub token_budget: i64,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct GenerateCasesFromDiffRequest {
    pub repo_id: Uuid,
    #[serde(default)]
    pub base_ref: Option<String>,
    #[serde(default)]
    pub head_ref: Option<String>,
    pub pull_number: Option<i64>,
    #[serde(default)]
    pub include_paths: Vec<String>,
    #[serde(default)]
    pub exclude_paths: Vec<String>,
    pub token_budget: Option<i64>,
    #[serde(default = "default_count")]
    pub count: u32,
    pub suite_id: Option<Uuid>,
    #[serde(default = "default_persist_true")]
    pub persist: bool,
    pub config_id: Option<Uuid>,
    /// Extra free-text guidance appended after the diff.
    #[serde(default)]
    pub extra_instructions: String,
}

fn default_persist_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct CaseGenerationSource {
    pub case_id: Uuid,
    pub source: String,
    pub repo_id: Option<Uuid>,
    pub provider: String,
    pub pull_number: Option<i32>,
    pub pull_url: Option<String>,
    pub base_ref: String,
    pub head_ref: String,
    pub commit_sha: Option<String>,
    pub included_paths: Vec<String>,
    #[serde(default)]
    #[schema(value_type = Object)]
    pub meta: serde_json::Value,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct GenerateCasesFromDiffResponse {
    pub provider: String,
    pub model: String,
    pub drafts: Vec<GeneratedCaseDraft>,
    pub persisted: Vec<crate::models::TestCaseDetail>,
    pub generation: Option<CaseGenerationSource>,
    pub selected_files: Vec<crate::services::automation_repo_diff::DiffFileEntry>,
    pub selected_approx_tokens: i64,
    pub head_sha: Option<String>,
    pub base_ref: String,
    pub head_ref: String,
    pub pull_number: Option<i64>,
    pub pull_url: Option<String>,
}
