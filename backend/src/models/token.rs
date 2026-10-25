use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, sqlx::FromRow, ToSchema)]
pub struct ApiTokenPublic {
    pub id: Uuid,
    pub name: String,
    pub token_prefix: String,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateApiToken {
    pub name: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CreateApiTokenResponse {
    pub token: ApiTokenPublic,
    /// Plaintext token — shown once; store it as `VULYK_API_TOKEN`.
    pub secret: String,
}
