use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct Tag {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub color: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateTag {
    pub name: String,
    #[serde(default = "default_color")]
    pub color: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateTag {
    pub name: Option<String>,
    pub color: Option<String>,
}

fn default_color() -> String {
    "#0f6b4c".into()
}
