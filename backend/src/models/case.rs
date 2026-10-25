use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct TestCase {
    pub id: Uuid,
    pub project_id: Uuid,
    pub suite_id: Option<Uuid>,
    pub key: String,
    pub title: String,
    pub description: String,
    pub precondition: String,
    pub priority: String,
    pub status: String,
    pub version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct TestStep {
    pub id: Uuid,
    pub case_id: Uuid,
    pub position: i32,
    pub action: String,
    pub expected_result: String,
    pub data: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TestCaseDetail {
    pub case: TestCase,
    pub steps: Vec<TestStep>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateStep {
    pub action: String,
    #[serde(default)]
    pub expected_result: String,
    #[serde(default)]
    pub data: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateCase {
    pub key: String,
    pub title: String,
    pub suite_id: Option<Uuid>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub precondition: String,
    #[serde(default = "default_priority")]
    pub priority: String,
    #[serde(default = "default_status")]
    pub status: String,
    #[serde(default)]
    pub steps: Vec<CreateStep>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateCase {
    pub title: Option<String>,
    pub suite_id: Option<Uuid>,
    pub description: Option<String>,
    pub precondition: Option<String>,
    pub priority: Option<String>,
    pub status: Option<String>,
    pub steps: Option<Vec<CreateStep>>,
}

fn default_priority() -> String {
    "medium".into()
}

fn default_status() -> String {
    "draft".into()
}
