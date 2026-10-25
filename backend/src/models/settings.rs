use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct IntegrationSetting {
    pub id: Uuid,
    pub project_id: Option<Uuid>,
    pub kind: String,
    pub name: String,
    #[schema(value_type = Object)]
    pub config: Value,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpsertIntegrationSetting {
    pub project_id: Option<Uuid>,
    pub kind: String,
    pub name: String,
    #[serde(default)]
    #[schema(value_type = Object)]
    pub config: Value,
    #[serde(default)]
    pub enabled: bool,
}
