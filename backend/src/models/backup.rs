//! Content backup DTOs, archive manifest, and admin API request/response types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;

pub const CONTENT_ARCHIVE_SCHEMA_VERSION: &str = "content-1.0";

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct BackupDestination {
    pub id: Uuid,
    pub name: String,
    pub kind: String,
    #[schema(value_type = Object)]
    pub config: Value,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// True when encrypted secrets are stored (never expose ciphertext).
    #[sqlx(default)]
    #[serde(default)]
    pub has_secrets: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateBackupDestination {
    pub name: String,
    pub kind: String,
    #[serde(default)]
    #[schema(value_type = Object)]
    pub config: Value,
    /// Plaintext secrets object (token, app_password, service_account_json, …). Encrypted at rest.
    #[serde(default)]
    #[schema(value_type = Option<Object>)]
    pub secrets: Option<Value>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateBackupDestination {
    pub name: Option<String>,
    #[serde(default)]
    #[schema(value_type = Option<Object>)]
    pub config: Option<Value>,
    /// When set, replaces stored secrets. Omit to keep existing. Empty object clears.
    #[serde(default)]
    #[schema(value_type = Option<Object>)]
    pub secrets: Option<Value>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct BackupPolicy {
    pub id: Uuid,
    pub project_id: Option<Uuid>,
    pub destination_id: Uuid,
    pub schedule: String,
    pub debounce_minutes: i32,
    pub retain_count: Option<i32>,
    pub retain_days: Option<i32>,
    pub pack_mode: String,
    pub include_attachments: bool,
    pub enabled: bool,
    pub last_scheduled_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateBackupPolicy {
    pub project_id: Option<Uuid>,
    pub destination_id: Uuid,
    #[serde(default = "default_schedule")]
    pub schedule: String,
    #[serde(default = "default_debounce")]
    pub debounce_minutes: i32,
    pub retain_count: Option<i32>,
    pub retain_days: Option<i32>,
    #[serde(default = "default_pack_zip")]
    pub pack_mode: String,
    #[serde(default = "default_true")]
    pub include_attachments: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateBackupPolicy {
    pub project_id: Option<Uuid>,
    pub destination_id: Option<Uuid>,
    pub schedule: Option<String>,
    pub debounce_minutes: Option<i32>,
    pub retain_count: Option<i32>,
    pub retain_days: Option<i32>,
    pub pack_mode: Option<String>,
    pub include_attachments: Option<bool>,
    pub enabled: Option<bool>,
    /// When true, clears project_id (all projects).
    #[serde(default)]
    pub clear_project: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct BackupRun {
    pub id: Uuid,
    pub policy_id: Option<Uuid>,
    pub destination_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub status: String,
    pub trigger: String,
    pub remote_path: Option<String>,
    pub local_path: Option<String>,
    pub size_bytes: i64,
    pub duration_ms: Option<i32>,
    pub error: Option<String>,
    #[schema(value_type = Object)]
    pub summary: Value,
    pub checksum_sha256: Option<String>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ManualBackupRequest {
    pub destination_id: Uuid,
    pub project_id: Option<Uuid>,
    #[serde(default = "default_pack_zip")]
    pub pack_mode: String,
    #[serde(default = "default_true")]
    pub include_attachments: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BackupTestResult {
    pub ok: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ContentArchiveManifest {
    pub schema_version: String,
    pub created_at: String,
    pub producer: String,
    pub scope: String,
    #[serde(default)]
    pub project_keys: Vec<String>,
    pub include_attachments: bool,
    pub pack_mode: String,
    #[serde(default)]
    pub file_checksums: Vec<FileChecksum>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct FileChecksum {
    pub path: String,
    pub sha256: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ContentArchiveProject {
    pub key: String,
    pub name: String,
    pub description: String,
    pub case_key_prefix: String,
    pub next_case_number: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ContentFolderNode {
    pub name: String,
    #[serde(default)]
    pub sort_order: i32,
    /// Nested folders (Object in OpenAPI to avoid recursion blow-up).
    #[serde(default)]
    #[schema(value_type = Vec<Object>)]
    pub children: Vec<ContentFolderNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ContentTag {
    pub name: String,
    #[serde(default = "default_tag_color")]
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ContentCustomFieldDef {
    pub key: String,
    pub name: String,
    pub field_type: String,
    #[serde(default)]
    pub entity: String,
    #[serde(default)]
    pub options: Value,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub sort_order: i32,
    #[serde(default)]
    pub option_list_key: Option<String>,
    #[serde(default)]
    pub help_text: String,
    #[serde(default)]
    pub active: bool,
    #[serde(default)]
    pub scope: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ContentOptionList {
    pub key: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_true")]
    pub active: bool,
    #[serde(default)]
    pub items: Vec<ContentOptionListItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ContentOptionListItem {
    pub label: String,
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub colour: Option<String>,
    #[serde(default)]
    pub sort_order: i32,
    #[serde(default = "default_true")]
    pub active: bool,
    #[serde(default)]
    pub deprecated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ContentCase {
    pub key: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub precondition: String,
    #[serde(default = "default_priority")]
    pub priority: String,
    #[serde(default = "default_status")]
    pub status: String,
    #[serde(default)]
    pub external_key: Option<String>,
    #[serde(default)]
    pub folder_path: Vec<String>,
    #[serde(default)]
    pub sort_order: i32,
    #[serde(default)]
    pub version: i32,
    #[serde(default)]
    pub steps: Vec<ContentStep>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub custom_fields: Vec<ContentFieldValue>,
    #[serde(default)]
    pub versions: Vec<ContentCaseVersion>,
    #[serde(default)]
    pub attachment_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ContentStep {
    pub position: i32,
    pub action: String,
    #[serde(default)]
    pub expected_result: String,
    #[serde(default)]
    pub data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ContentFieldValue {
    pub field_key: String,
    pub field_name: String,
    pub value: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ContentCaseVersion {
    pub version: i32,
    pub snapshot: Value,
    #[serde(default)]
    pub change_summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ContentRequirement {
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub acceptance_criteria: String,
    #[serde(default)]
    pub external_key: Option<String>,
    #[serde(default)]
    pub external_id: Option<String>,
    #[serde(default = "default_source")]
    pub source: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub folder_path: Vec<String>,
    #[serde(default)]
    pub case_keys: Vec<String>,
    #[serde(default)]
    pub custom_fields: Vec<ContentFieldValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ContentTombstone {
    pub key: String,
    pub case_number: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ContentAttachmentMeta {
    pub id: String,
    pub filename: String,
    pub content_type: String,
    pub size_bytes: i64,
    pub entity_type: String,
    /// Case key when entity_type == case
    #[serde(default)]
    pub entity_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ContentProjectBundle {
    pub project: ContentArchiveProject,
    #[serde(default)]
    pub case_folders: Vec<ContentFolderNode>,
    #[serde(default)]
    pub requirement_folders: Vec<ContentFolderNode>,
    #[serde(default)]
    pub tags: Vec<ContentTag>,
    #[serde(default)]
    pub custom_fields: Vec<ContentCustomFieldDef>,
    #[serde(default)]
    pub option_lists: Vec<ContentOptionList>,
    #[serde(default)]
    pub cases: Vec<ContentCase>,
    #[serde(default)]
    pub requirements: Vec<ContentRequirement>,
    #[serde(default)]
    pub tombstones: Vec<ContentTombstone>,
    #[serde(default)]
    pub attachments: Vec<ContentAttachmentMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ContentArchive {
    pub manifest: ContentArchiveManifest,
    #[serde(default)]
    pub option_lists: Vec<ContentOptionList>,
    #[serde(default)]
    pub custom_fields: Vec<ContentCustomFieldDef>,
    pub projects: Vec<ContentProjectBundle>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RestoreRequest {
    /// When set, load archive bytes from this completed backup run.
    pub run_id: Option<Uuid>,
    /// Inline archive (JSON ContentArchive). For ZIP uploads use multipart endpoint.
    pub archive: Option<ContentArchive>,
    /// new_project | merge | replace
    #[serde(default = "default_mode_merge")]
    pub mode: String,
    /// skip | overwrite — used with merge
    #[serde(default = "default_conflict_skip")]
    pub conflict: String,
    /// Target project for merge/replace. Ignored for new_project (uses archive key, may rename).
    pub target_project_id: Option<Uuid>,
    /// Optional new key when mode=new_project and key would clash.
    pub new_project_key: Option<String>,
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RestorePreview {
    pub dry_run: bool,
    pub mode: String,
    pub schema_version: String,
    pub project_key: String,
    pub counts: RestoreCounts,
    pub conflicts: Vec<RestoreConflict>,
    pub custom_field_mappings: Vec<BackupFieldMapping>,
    pub warnings: Vec<String>,
    pub applied: bool,
}

#[derive(Debug, Clone, Serialize, ToSchema, Default)]
pub struct RestoreCounts {
    pub cases_create: i32,
    pub cases_update: i32,
    pub cases_skip: i32,
    pub requirements_create: i32,
    pub requirements_update: i32,
    pub requirements_skip: i32,
    pub tags_create: i32,
    pub custom_fields_create: i32,
    pub folders_create: i32,
    pub tombstones_create: i32,
    pub attachments_create: i32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RestoreConflict {
    pub entity: String,
    pub key: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct BackupFieldMapping {
    pub archive_key: String,
    pub archive_name: String,
    pub action: String,
    pub target_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow, ToSchema)]
pub struct Notification {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub kind: String,
    pub title: String,
    pub body: String,
    #[schema(value_type = Object)]
    pub payload: Value,
    pub read_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

fn default_true() -> bool {
    true
}
fn default_schedule() -> String {
    "daily".into()
}
fn default_debounce() -> i32 {
    30
}
fn default_pack_zip() -> String {
    "zip".into()
}
fn default_tag_color() -> String {
    "#0f6b4c".into()
}
fn default_priority() -> String {
    "medium".into()
}
fn default_status() -> String {
    "draft".into()
}
fn default_source() -> String {
    "manual".into()
}
fn default_mode_merge() -> String {
    "merge".into()
}
fn default_conflict_skip() -> String {
    "skip".into()
}
