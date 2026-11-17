use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, sqlx::FromRow, ToSchema)]
pub struct OutboundWebhookEndpoint {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub url: String,
    pub secret_ciphertext: Vec<u8>,
    pub secret_nonce: Vec<u8>,
    pub events: Vec<String>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct OutboundWebhookEndpointPublic {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub url: String,
    pub has_secret: bool,
    pub events: Vec<String>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<OutboundWebhookEndpoint> for OutboundWebhookEndpointPublic {
    fn from(row: OutboundWebhookEndpoint) -> Self {
        Self {
            id: row.id,
            project_id: row.project_id,
            name: row.name,
            url: row.url,
            has_secret: !row.secret_ciphertext.is_empty(),
            events: row.events,
            enabled: row.enabled,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateOutboundWebhook {
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub events: Vec<String>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateOutboundWebhook {
    pub name: Option<String>,
    pub url: Option<String>,
    pub events: Option<Vec<String>>,
    pub enabled: Option<bool>,
    #[serde(default)]
    pub rotate_secret: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CreateOutboundWebhookResponse {
    pub endpoint: OutboundWebhookEndpointPublic,
    /// Plaintext signing secret — shown once on create or secret rotation.
    pub secret: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow, ToSchema)]
pub struct OutboundWebhookDelivery {
    pub id: Uuid,
    pub endpoint_id: Uuid,
    pub project_id: Uuid,
    pub event_type: String,
    #[schema(value_type = Object)]
    pub payload: Value,
    pub status: String,
    pub attempt_count: i32,
    pub http_status: Option<i32>,
    pub response_body: Option<String>,
    pub last_error: Option<String>,
    pub job_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub delivered_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct OutboundWebhookDeliveryPublic {
    pub id: Uuid,
    pub endpoint_id: Uuid,
    pub project_id: Uuid,
    pub event_type: String,
    #[schema(value_type = Object)]
    pub payload: Value,
    pub status: String,
    pub attempt_count: i32,
    pub http_status: Option<i32>,
    pub response_body: Option<String>,
    pub last_error: Option<String>,
    pub job_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub delivered_at: Option<DateTime<Utc>>,
}

impl From<OutboundWebhookDelivery> for OutboundWebhookDeliveryPublic {
    fn from(d: OutboundWebhookDelivery) -> Self {
        Self {
            id: d.id,
            endpoint_id: d.endpoint_id,
            project_id: d.project_id,
            event_type: d.event_type,
            payload: d.payload,
            status: d.status,
            attempt_count: d.attempt_count,
            http_status: d.http_status,
            response_body: d.response_body,
            last_error: d.last_error,
            job_id: d.job_id,
            created_at: d.created_at,
            updated_at: d.updated_at,
            delivered_at: d.delivered_at,
        }
    }
}
