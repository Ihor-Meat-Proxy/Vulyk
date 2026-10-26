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
