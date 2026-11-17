use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// How ingest handles results that do not match key / automation_id / external_key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum CiUnmatchedPolicy {
    /// List unmatched keys and skip (historical default).
    #[default]
    Off,
    /// Auto-create a draft case for unmatched **Passed** results, then attach.
    CreatePassed,
    /// Auto-create a draft case for any unmatched status, then attach.
    CreateAll,
    /// Fail the ingest request (4xx) when any unmatched remain.
    FailUnmatched,
}

impl CiUnmatchedPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::CreatePassed => "create_passed",
            Self::CreateAll => "create_all",
            Self::FailUnmatched => "fail_unmatched",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "off" => Some(Self::Off),
            "create_passed" => Some(Self::CreatePassed),
            "create_all" => Some(Self::CreateAll),
            "fail_unmatched" => Some(Self::FailUnmatched),
            _ => None,
        }
    }
}

impl std::fmt::Display for CiUnmatchedPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

pub fn default_ci_unmatched_policy() -> String {
    CiUnmatchedPolicy::Off.as_str().into()
}

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
    /// Unmatched CI result policy: `off`, `create_passed`, `create_all`, `fail_unmatched`.
    #[serde(default = "default_ci_unmatched_policy")]
    pub ci_unmatched_policy: String,
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
    /// Optional unmatched CI policy (defaults to `off`).
    #[serde(default)]
    pub ci_unmatched_policy: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateProject {
    pub name: Option<String>,
    pub description: Option<String>,
    /// Editable only before any cases exist (next_case_number == 0 and no cases/tombstones).
    pub case_key_prefix: Option<String>,
    /// Unmatched CI result policy: `off`, `create_passed`, `create_all`, `fail_unmatched`.
    pub ci_unmatched_policy: Option<String>,
}
