use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct Requirement {
    pub id: Uuid,
    pub project_id: Uuid,
    pub external_key: Option<String>,
    pub external_id: Option<String>,
    pub title: String,
    pub description: String,
    pub source: String,
    pub url: String,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateRequirement {
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_source")]
    pub source: String,
    pub external_key: Option<String>,
    pub external_id: Option<String>,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateRequirement {
    pub title: Option<String>,
    pub description: Option<String>,
    pub external_key: Option<String>,
    pub external_id: Option<String>,
    pub url: Option<String>,
    pub metadata: Option<Value>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct LinkCaseRequirement {
    pub case_id: Uuid,
    pub requirement_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct CaseRequirementLink {
    pub case_id: Uuid,
    pub requirement_id: Uuid,
    pub created_at: DateTime<Utc>,
}

fn default_source() -> String {
    "manual".into()
}
