use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct Suite {
    pub id: Uuid,
    pub project_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub name: String,
    pub sort_order: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Direct (non-recursive) case count in this folder.
    #[serde(default)]
    pub case_count: i64,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateSuite {
    pub name: String,
    pub parent_id: Option<Uuid>,
    #[serde(default)]
    pub sort_order: i32,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateSuite {
    pub name: Option<String>,
    pub parent_id: Option<Uuid>,
    /// When true, move folder to project root (parent_id = NULL).
    #[serde(default)]
    pub clear_parent: bool,
    pub sort_order: Option<i32>,
}

/// Options when deleting a non-empty folder.
#[derive(Debug, Clone, Deserialize, ToSchema, Default)]
pub struct DeleteSuiteQuery {
    /// `move_to_parent` (default) reparents cases/children to the deleted folder's parent.
    /// `delete_cases` deletes cases in this folder (and descendants) and retires their keys.
    #[serde(default = "default_on_nonempty")]
    pub on_nonempty: String,
}

fn default_on_nonempty() -> String {
    "move_to_parent".into()
}
