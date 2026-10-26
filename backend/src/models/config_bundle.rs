use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;

pub const CONFIG_BUNDLE_VERSION: &str = "1.0";

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ConfigBundle {
    pub version: String,
    pub exported_at: String,
    pub instance: ConfigInstanceSection,
    #[serde(default)]
    pub option_lists: Vec<ConfigOptionList>,
    #[serde(default)]
    pub custom_fields: Vec<ConfigCustomField>,
    #[serde(default)]
    pub projects: Vec<ConfigProject>,
    #[serde(default)]
    pub roles_memberships: Option<ConfigRolesSection>,
    #[serde(default)]
    pub llm_providers: Vec<ConfigLlmProvider>,
    #[serde(default)]
    pub integrations: Vec<ConfigIntegration>,
    #[serde(default)]
    pub automation_repos: Vec<ConfigAutomationRepo>,
    #[serde(default)]
    pub data: Option<ConfigDataSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ConfigInstanceSection {
    pub name: Option<String>,
    pub default_locale: Option<String>,
    pub default_timezone: Option<String>,
    pub registration_mode: Option<String>,
    pub appearance_defaults: Option<Value>,
    pub sso_providers: Vec<ConfigSsoProvider>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ConfigSsoProvider {
    pub key: String,
    pub name: String,
    pub provider_type: String,
    pub enabled: bool,
    pub config: Value,
    /// Secrets are omitted unless encrypted_secrets is set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encrypted_secrets: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ConfigOptionList {
    pub key: String,
    pub name: String,
    pub description: String,
    pub active: bool,
    pub items: Vec<ConfigOptionListItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ConfigOptionListItem {
    pub key: Option<String>,
    pub label: String,
    pub value: Option<String>,
    pub colour: Option<String>,
    pub description: String,
    pub sort_order: i32,
    pub active: bool,
    pub deprecated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ConfigCustomField {
    pub key: String,
    pub name: String,
    pub field_type: String,
    pub entity: String,
    pub option_list_key: Option<String>,
    pub default_value: Value,
    pub required: bool,
    pub help_text: String,
    pub sort_order: i32,
    pub show_in_list: bool,
    pub show_in_filters: bool,
    pub active: bool,
    pub scope: String,
    #[serde(default)]
    pub project_keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ConfigProject {
    pub key: String,
    pub name: String,
    pub description: String,
    pub case_key_prefix: Option<String>,
    #[serde(default)]
    pub folders: Vec<ConfigFolder>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ConfigFolder {
    pub name: String,
    #[serde(default)]
    pub children: Vec<ConfigFolder>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ConfigRolesSection {
    #[serde(default)]
    pub memberships: Vec<ConfigMembership>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ConfigMembership {
    pub project_key: String,
    pub email: String,
    pub role: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ConfigLlmProvider {
    pub project_key: Option<String>,
    pub provider: String,
    pub name: String,
    pub model: Option<String>,
    pub enabled: bool,
    pub config: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ConfigIntegration {
    pub project_key: Option<String>,
    pub kind: String,
    pub name: String,
    pub enabled: bool,
    pub config: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ConfigAutomationRepo {
    pub project_key: String,
    pub provider: String,
    pub remote_url: String,
    pub branch: Option<String>,
    pub subpath: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ConfigDataSection {
    #[serde(default)]
    pub cases: Vec<Value>,
    #[serde(default)]
    pub requirements: Vec<Value>,
    #[serde(default)]
    pub runs: Vec<Value>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ConfigExportRequest {
    /// all | selected
    #[serde(default = "default_scope_all")]
    pub project_scope: String,
    #[serde(default)]
    pub project_ids: Vec<Uuid>,
    #[serde(default)]
    pub project_keys: Vec<String>,
    #[serde(default)]
    pub include_memberships: bool,
    #[serde(default)]
    pub include_data: bool,
    #[serde(default)]
    pub include_folder_structure: bool,
    /// Optional passphrase to encrypt SSO/integration secrets into the bundle.
    #[serde(default)]
    pub secrets_passphrase: Option<String>,
    /// json | yaml
    #[serde(default = "default_format_json")]
    pub format: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ConfigImportRequest {
    pub bundle: ConfigBundle,
    /// merge | replace
    #[serde(default = "default_strategy_merge")]
    pub strategy: String,
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default)]
    pub secrets_passphrase: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ConfigImportPreview {
    pub dry_run: bool,
    pub strategy: String,
    pub creates: Vec<ConfigChangeItem>,
    pub updates: Vec<ConfigChangeItem>,
    pub conflicts: Vec<ConfigChangeItem>,
    pub skipped: Vec<ConfigChangeItem>,
    pub applied: bool,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ConfigChangeItem {
    pub kind: String,
    pub key: String,
    pub action: String,
    pub detail: String,
}

fn default_scope_all() -> String {
    "all".into()
}

fn default_format_json() -> String {
    "json".into()
}

fn default_strategy_merge() -> String {
    "merge".into()
}
