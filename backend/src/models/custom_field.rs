use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;

/// Canonical field types exposed by the Admin UI / OpenAPI.
pub const FIELD_TYPES: &[&str] = &[
    "text",
    "long_text",
    "number",
    "checkbox",
    "dropdown",
    "multi_select",
    "date",
    "url",
];

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct CustomFieldDefinition {
    pub id: Uuid,
    pub key: String,
    pub name: String,
    pub field_type: String,
    pub entity: String,
    pub option_list_id: Option<Uuid>,
    pub default_value: Value,
    pub required: bool,
    pub help_text: String,
    pub sort_order: i32,
    pub show_in_list: bool,
    pub show_in_filters: bool,
    pub active: bool,
    pub scope: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CustomFieldDefinitionDetail {
    #[serde(flatten)]
    pub field: CustomFieldDefinition,
    #[serde(default)]
    pub project_ids: Vec<Uuid>,
    #[serde(default)]
    pub project_overrides: Vec<CustomFieldProjectOverride>,
    /// True when stored values exist; hard-delete should be blocked.
    #[serde(default)]
    pub in_use: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct CustomFieldProjectOverride {
    pub field_id: Uuid,
    pub project_id: Uuid,
    pub required_override: Option<bool>,
    pub default_value_override: Option<Value>,
    pub enabled: bool,
}

/// Project-resolved field (effective required/default after overrides).
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CustomField {
    pub id: Uuid,
    pub project_id: Uuid,
    pub key: String,
    pub name: String,
    pub field_type: String,
    pub entity: String,
    pub option_list_id: Option<Uuid>,
    /// Legacy alias: options array derived from option list items when present.
    pub options: Value,
    pub default_value: Value,
    pub required: bool,
    pub help_text: String,
    pub sort_order: i32,
    pub show_in_list: bool,
    pub show_in_filters: bool,
    pub active: bool,
    pub scope: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateCustomFieldDefinition {
    pub key: String,
    pub name: String,
    pub field_type: String,
    #[serde(default = "default_entity")]
    pub entity: String,
    pub option_list_id: Option<Uuid>,
    #[serde(default = "default_null_value")]
    pub default_value: Value,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub help_text: String,
    #[serde(default)]
    pub sort_order: i32,
    #[serde(default)]
    pub show_in_list: bool,
    #[serde(default)]
    pub show_in_filters: bool,
    #[serde(default = "default_true")]
    pub active: bool,
    #[serde(default = "default_scope_all")]
    pub scope: String,
    /// Required when scope = selected; ignored when scope = all.
    #[serde(default)]
    pub project_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateCustomFieldDefinition {
    pub key: Option<String>,
    pub name: Option<String>,
    pub field_type: Option<String>,
    pub entity: Option<String>,
    pub option_list_id: Option<Option<Uuid>>,
    pub default_value: Option<Value>,
    pub required: Option<bool>,
    pub help_text: Option<String>,
    pub sort_order: Option<i32>,
    pub show_in_list: Option<bool>,
    pub show_in_filters: Option<bool>,
    pub active: Option<bool>,
    pub scope: Option<String>,
    pub project_ids: Option<Vec<Uuid>>,
    /// When true, scope/project changes hide fields but keep stored values.
    #[serde(default)]
    pub acknowledge_hide_values: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpsertProjectFieldOverride {
    pub required_override: Option<Option<bool>>,
    pub default_value_override: Option<Option<Value>>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AddFieldToProject {
    #[serde(default)]
    pub required_override: Option<bool>,
    #[serde(default)]
    pub default_value_override: Option<Value>,
}

/// Legacy create body kept for project-scoped API compatibility.
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
    pub key: Option<String>,
    pub entity: Option<String>,
    pub option_list_id: Option<Uuid>,
    pub help_text: Option<String>,
    pub default_value: Option<Value>,
    pub show_in_list: Option<bool>,
    pub show_in_filters: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateCustomField {
    pub name: Option<String>,
    pub field_type: Option<String>,
    pub options: Option<Value>,
    pub required: Option<bool>,
    pub sort_order: Option<i32>,
    pub help_text: Option<String>,
    pub default_value: Option<Value>,
    pub option_list_id: Option<Uuid>,
    pub show_in_list: Option<bool>,
    pub show_in_filters: Option<bool>,
    pub active: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CustomFieldValue {
    pub field_id: Uuid,
    pub name: String,
    pub field_type: String,
    pub key: String,
    pub value: Value,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SetCustomFieldValue {
    pub field_id: Uuid,
    pub value: Value,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ScopeChangeWarning {
    pub warning: String,
    pub hidden_project_ids: Vec<Uuid>,
    pub values_retained: bool,
}

fn default_true() -> bool {
    true
}

fn default_entity() -> String {
    "test_case".into()
}

fn default_scope_all() -> String {
    "all".into()
}

fn default_field_type() -> String {
    "text".into()
}

fn default_options() -> Value {
    Value::Array(vec![])
}

fn default_null_value() -> Value {
    Value::Null
}

/// Normalize legacy aliases to canonical types.
pub fn normalize_field_type(raw: &str) -> String {
    match raw {
        "boolean" => "checkbox".into(),
        "select" => "dropdown".into(),
        other => other.to_string(),
    }
}

pub fn is_allowed_field_type(raw: &str) -> bool {
    let n = normalize_field_type(raw);
    FIELD_TYPES.contains(&n.as_str())
}

pub fn needs_option_list(field_type: &str) -> bool {
    matches!(
        normalize_field_type(field_type).as_str(),
        "dropdown" | "multi_select"
    )
}
