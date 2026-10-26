use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct Project {
    pub id: Uuid,
    pub key: String,
    pub name: String,
    pub description: String,
    /// Auto-derived uppercase initials used for case keys (e.g. KOC).
    pub case_key_prefix: String,
    /// Monotonic counter; next allocated case uses this+1. Never decreases.
    pub next_case_number: i32,
    /// Seeded demo project (admin can remove/re-seed).
    #[serde(default)]
    pub is_sample: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateProject {
    /// Uppercase project key, e.g. DEMO. Derived from `name` (same initials rules as
    /// `case_key_prefix`) when omitted.
    #[serde(default)]
    pub key: Option<String>,
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Optional override for case key prefix (must be unique). Derived from name when omitted.
    #[serde(default)]
    pub case_key_prefix: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateProject {
    pub name: Option<String>,
    pub description: Option<String>,
    /// Editable only before any cases exist (next_case_number == 0 and no cases/tombstones).
    pub case_key_prefix: Option<String>,
}
