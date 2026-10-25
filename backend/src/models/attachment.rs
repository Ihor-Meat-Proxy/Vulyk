use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct AttachmentMeta {
    pub id: Uuid,
    pub project_id: Uuid,
    pub entity_type: String,
    pub entity_id: Uuid,
    pub filename: String,
    pub content_type: String,
    pub size_bytes: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateAttachment {
    pub entity_type: String,
    pub entity_id: Uuid,
    pub filename: String,
    #[serde(default = "default_content_type")]
    pub content_type: String,
    /// Base64-encoded file body (kept small for Phase 1; object storage later).
    pub content_base64: String,
}

fn default_content_type() -> String {
    "application/octet-stream".into()
}
