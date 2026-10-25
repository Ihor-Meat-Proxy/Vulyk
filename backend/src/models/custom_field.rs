use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct CustomField {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub field_type: String,
    pub options: Value,
    pub required: bool,
    pub sort_order: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateCustomField {
    pub name: String,
    #[serde(default = "default_field_type")]
    pub field_type: String,
    #[serde(default = "default_options")]
    pub options: Value,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub sort_order: i32,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateCustomField {
    pub name: Option<String>,
    pub field_type: Option<String>,
    pub options: Option<Value>,
    pub required: Option<bool>,
    pub sort_order: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CustomFieldValue {
    pub field_id: Uuid,
    pub name: String,
    pub field_type: String,
    pub value: Value,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SetCustomFieldValue {
    pub field_id: Uuid,
    pub value: Value,
}

fn default_field_type() -> String {
    "text".into()
}

fn default_options() -> Value {
    Value::Array(vec![])
}
