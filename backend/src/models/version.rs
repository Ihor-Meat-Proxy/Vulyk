use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct CaseVersion {
    pub id: Uuid,
    pub case_id: Uuid,
    pub version: i32,
    pub snapshot: Value,
    pub change_summary: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CaseVersionSummary {
    pub id: Uuid,
    pub case_id: Uuid,
    pub version: i32,
    pub change_summary: String,
    pub created_at: DateTime<Utc>,
}
