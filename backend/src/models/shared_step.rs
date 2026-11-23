use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct SharedStep {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct SharedStepLine {
    pub id: Uuid,
    pub shared_step_id: Uuid,
    pub position: i32,
    pub action: String,
    pub expected_result: String,
    pub data: String,
}

/// List row: shared step plus the number of distinct cases that reference it.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct SharedStepSummary {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub line_count: i64,
    pub usage_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SharedStepDetail {
    #[serde(flatten)]
    pub shared_step: SharedStep,
    pub lines: Vec<SharedStepLine>,
    pub usage_count: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
pub struct CreateSharedStepLine {
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub expected_result: String,
    #[serde(default)]
    pub data: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateSharedStep {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub lines: Vec<CreateSharedStepLine>,
}

/// Partial update. When `lines` is present, all existing lines are replaced.
#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateSharedStep {
    pub name: Option<String>,
    pub description: Option<String>,
    pub lines: Option<Vec<CreateSharedStepLine>>,
}
