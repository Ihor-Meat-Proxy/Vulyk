use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct TestPlan {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TestPlanDetail {
    pub plan: TestPlan,
    pub case_ids: Vec<Uuid>,
    pub case_count: i64,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreatePlan {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub case_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdatePlan {
    pub name: Option<String>,
    pub description: Option<String>,
    pub case_ids: Option<Vec<Uuid>>,
}
