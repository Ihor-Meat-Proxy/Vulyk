use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct OptionList {
    pub id: Uuid,
    pub key: String,
    pub name: String,
    pub description: String,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct OptionListItem {
    pub id: Uuid,
    pub list_id: Uuid,
    pub key: Option<String>,
    pub label: String,
    pub value: Option<String>,
    pub colour: Option<String>,
    pub description: String,
    pub sort_order: i32,
    pub active: bool,
    pub deprecated: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct OptionListDetail {
    #[serde(flatten)]
    pub list: OptionList,
    pub items: Vec<OptionListItem>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateOptionList {
    pub key: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_true")]
    pub active: bool,
    #[serde(default)]
    pub items: Vec<CreateOptionListItem>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateOptionList {
    pub key: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub active: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema, Clone)]
pub struct CreateOptionListItem {
    pub label: String,
    pub key: Option<String>,
    pub value: Option<String>,
    pub colour: Option<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub sort_order: i32,
    #[serde(default = "default_true")]
    pub active: bool,
    #[serde(default)]
    pub deprecated: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateOptionListItem {
    pub label: Option<String>,
    pub key: Option<String>,
    pub value: Option<String>,
    pub colour: Option<String>,
    pub description: Option<String>,
    pub sort_order: Option<i32>,
    pub active: Option<bool>,
    pub deprecated: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ReorderOptionListItems {
    pub item_ids: Vec<Uuid>,
}

fn default_true() -> bool {
    true
}
