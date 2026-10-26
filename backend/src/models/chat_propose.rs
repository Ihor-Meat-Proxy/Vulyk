use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::models::{CreateStep, Requirement, TestCaseDetail};

#[derive(Debug, Deserialize, ToSchema)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ChatProposeRequest {
    pub message: String,
    #[serde(default)]
    pub history: Vec<ChatMessage>,
    pub config_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChatResponseKind {
    Answer,
    Clarify,
    ProposeCases,
    ProposeRequirements,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ProposedCaseDraft {
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub precondition: String,
    #[serde(default = "default_priority")]
    pub priority: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub steps: Vec<CreateStep>,
}

fn default_priority() -> String {
    "medium".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ProposedRequirementDraft {
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub acceptance_criteria: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ProposedLinkDraft {
    /// Index into the proposed cases list.
    pub case_index: u32,
    /// Index into the proposed requirements list (same proposal or 0 if only cases).
    #[serde(default)]
    pub requirement_index: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChatPlacement {
    pub project_id: Uuid,
    /// Suite id for cases; requirement folder id for requirements.
    pub folder_id: Option<Uuid>,
    pub folder_path: String,
    /// When set, create this folder (under parent `folder_id`) before inserting.
    pub new_folder_name: Option<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChatProposal {
    pub placement: ChatPlacement,
    #[serde(default)]
    pub cases: Vec<ProposedCaseDraft>,
    #[serde(default)]
    pub requirements: Vec<ProposedRequirementDraft>,
    #[serde(default)]
    pub links: Vec<ProposedLinkDraft>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ChatProposeResponse {
    pub provider: String,
    pub model: String,
    pub kind: ChatResponseKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub answer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clarify_question: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposal: Option<ChatProposal>,
    /// Human-readable confirmation prompt for the UI.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirmation_prompt: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ChatConfirmRequest {
    /// Edited proposal from the preview card.
    pub proposal: ChatProposal,
    pub config_id: Option<Uuid>,
    /// Echo of which model drafted it (for audit).
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub model: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ChatConfirmResponse {
    pub cases: Vec<TestCaseDetail>,
    pub requirements: Vec<Requirement>,
    pub created_folder_id: Option<Uuid>,
    pub links_created: u32,
    pub events: Vec<ChatCreationEventPublic>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow, ToSchema)]
pub struct ChatCreationEventPublic {
    pub id: Uuid,
    pub project_id: Uuid,
    pub provider: String,
    pub model: String,
    pub kind: String,
    pub entity_id: Option<Uuid>,
    pub entity_key: Option<String>,
    pub summary: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RequirementFolder {
    pub id: Uuid,
    pub project_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub name: String,
    pub sort_order: i32,
    pub requirement_count: i64,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateRequirementFolder {
    pub name: String,
    pub parent_id: Option<Uuid>,
    #[serde(default)]
    pub sort_order: i32,
}

/// Loose JSON envelope used when the model lacks native tool calling.
#[derive(Debug, Deserialize)]
pub struct ChatJsonEnvelope {
    #[serde(rename = "type", alias = "kind")]
    pub kind: String,
    #[serde(default)]
    pub answer: Option<String>,
    #[serde(default)]
    pub clarify_question: Option<String>,
    #[serde(default)]
    pub question: Option<String>,
    #[serde(default)]
    pub placement: Option<Value>,
    #[serde(default)]
    pub cases: Vec<ProposedCaseDraft>,
    #[serde(default)]
    pub requirements: Vec<ProposedRequirementDraft>,
    #[serde(default)]
    pub links: Vec<ProposedLinkDraft>,
    #[serde(default)]
    pub folder_id: Option<Uuid>,
    #[serde(default)]
    pub folder_path: Option<String>,
    #[serde(default)]
    pub new_folder_name: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
}
