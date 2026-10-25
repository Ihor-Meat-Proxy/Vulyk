use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct CiConfig {
    pub id: Uuid,
    pub project_id: Uuid,
    pub provider: String,
    pub name: String,
    #[schema(value_type = Object)]
    pub config: Value,
    pub webhook_secret: Option<String>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateCiConfig {
    /// `github` or `gitlab`
    pub provider: String,
    pub name: String,
    #[schema(value_type = Object)]
    pub config: Value,
    pub webhook_secret: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateCiConfig {
    pub name: Option<String>,
    #[schema(value_type = Object)]
    pub config: Option<Value>,
    pub webhook_secret: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct CiDispatch {
    pub id: Uuid,
    pub project_id: Uuid,
    pub ci_config_id: Uuid,
    pub run_id: Option<Uuid>,
    pub status: String,
    pub external_id: Option<String>,
    pub external_url: Option<String>,
    #[schema(value_type = Object)]
    pub inputs: Value,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct DispatchCiRequest {
    #[serde(default)]
    #[schema(value_type = Object)]
    pub inputs: Value,
    /// Optional Vulyk run to attach; created automatically when omitted.
    pub run_id: Option<Uuid>,
    pub run_name: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct WebhookAck {
    pub received: bool,
    pub dispatch_id: Option<Uuid>,
    pub status: Option<String>,
}
