use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::SetCustomFieldValue;

#[derive(Debug, Deserialize, ToSchema)]
pub struct BulkCasesRequest {
    pub action: String,
    pub case_ids: Vec<Uuid>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub suite_id: Option<Uuid>,
    #[serde(default)]
    pub clear_suite: bool,
    #[serde(default)]
    pub tag_ids: Vec<Uuid>,
    #[serde(default)]
    pub tag_names: Vec<String>,
    /// Used with action `set_custom_fields`.
    #[serde(default)]
    pub custom_field_values: Vec<SetCustomFieldValue>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BulkCasesResponse {
    pub affected: i64,
    pub action: String,
}
