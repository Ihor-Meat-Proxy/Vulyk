use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct Integration {
    pub id: Uuid,
    pub project_id: Uuid,
    pub kind: String,
    pub name: String,
    pub base_url: String,
    pub config: Value,
    /// Secrets are never serialized to API responses (redacted in routes).
    #[serde(skip_serializing)]
    pub secrets: Value,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct IntegrationPublic {
    pub id: Uuid,
    pub project_id: Uuid,
    pub kind: String,
    pub name: String,
    pub base_url: String,
    pub config: Value,
    pub has_secrets: bool,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Integration> for IntegrationPublic {
    fn from(value: Integration) -> Self {
        let has_secrets = value
            .secrets
            .as_object()
            .map(|o| !o.is_empty())
            .unwrap_or(false);
        Self {
            id: value.id,
            project_id: value.project_id,
            kind: value.kind,
            name: value.name,
            base_url: value.base_url,
            config: value.config,
            has_secrets,
            enabled: value.enabled,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateIntegration {
    pub kind: String,
    pub name: String,
    #[serde(default)]
    pub base_url: String,
    #[serde(default = "empty_object")]
    pub config: Value,
    #[serde(default = "empty_object")]
    pub secrets: Value,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateIntegration {
    pub name: Option<String>,
    pub base_url: Option<String>,
    pub config: Option<Value>,
    pub secrets: Option<Value>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct Defect {
    pub id: Uuid,
    pub project_id: Uuid,
    pub integration_id: Option<Uuid>,
    pub result_id: Option<Uuid>,
    pub case_id: Option<Uuid>,
    pub external_key: Option<String>,
    pub external_url: String,
    pub title: String,
    pub status: String,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateDefectRequest {
    pub integration_id: Uuid,
    pub result_id: Option<Uuid>,
    pub case_id: Option<Uuid>,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub steps_to_reproduce: String,
}

fn empty_object() -> Value {
    json!({})
}

fn default_true() -> bool {
    true
}
