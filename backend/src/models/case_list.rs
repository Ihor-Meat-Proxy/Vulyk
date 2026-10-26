//! Case list DTOs: enriched list rows, column preferences, sort helpers.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;

use super::CustomFieldValue;

/// Built-in column ids (stable API / preference keys). Custom fields use `cf:<uuid>`.
pub const CASE_LIST_BUILTIN_COLUMNS: &[&str] = &[
    "key",
    "title",
    "priority",
    "status",
    "automated",
    "last_run_status",
    "updated_at",
    "created_at",
    "folder",
    "tags",
];

/// Columns that may never be hidden.
pub const CASE_LIST_REQUIRED_COLUMNS: &[&str] = &["title"];

/// Default visible built-in columns (order).
pub const CASE_LIST_DEFAULT_VISIBLE: &[&str] = &["key", "title", "priority", "status"];

/// Allowed sort fields (plus `cf:<uuid>` and `default`).
pub const CASE_LIST_SORT_FIELDS: &[&str] = &[
    "default",
    "key",
    "title",
    "priority",
    "status",
    "automated",
    "last_run_status",
    "updated_at",
    "created_at",
    "folder",
    "tags",
    "sort_order",
];

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TestCaseListItem {
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
    #[serde(default)]
    pub external_key: Option<String>,
    #[serde(default)]
    pub sort_order: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// True when the case has an `external_key` (automation / import link).
    #[serde(default)]
    pub automated: bool,
    /// Latest `test_results.status` by executed_at (manual + ingest).
    #[serde(default)]
    pub last_run_status: Option<String>,
    /// Folder (suite) display name when filed.
    #[serde(default)]
    pub folder_name: Option<String>,
    /// Tag names sorted alphabetically.
    #[serde(default)]
    pub tag_names: Vec<String>,
    /// Values for requested custom-field columns (`cf_ids` query param).
    #[serde(default)]
    pub custom_fields: Vec<CustomFieldValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct CaseListColumnPref {
    /// Built-in id (`key`, `title`, …) or `cf:<uuid>` for a project custom field.
    pub id: String,
    pub visible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct CaseListColumnsConfig {
    pub columns: Vec<CaseListColumnPref>,
    /// Optional persisted sort field (`key`, `title`, `cf:<uuid>`, …).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort: Option<String>,
    /// `asc` | `desc`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<String>,
}

impl Default for CaseListColumnsConfig {
    fn default() -> Self {
        Self::default_config()
    }
}

impl CaseListColumnsConfig {
    pub fn default_config() -> Self {
        let columns = CASE_LIST_BUILTIN_COLUMNS
            .iter()
            .map(|id| CaseListColumnPref {
                id: (*id).to_string(),
                visible: CASE_LIST_DEFAULT_VISIBLE.contains(id),
            })
            .collect();
        Self {
            columns,
            sort: None,
            order: None,
        }
    }

    pub fn from_value(value: &Value) -> Self {
        if value.is_null() || value.as_object().is_some_and(|o| o.is_empty()) {
            return Self::default_config();
        }
        match serde_json::from_value::<CaseListColumnsConfig>(value.clone()) {
            Ok(cfg) => cfg.sanitized(),
            Err(_) => Self::default_config(),
        }
    }

    pub fn to_value(&self) -> Value {
        serde_json::to_value(self).unwrap_or_else(|_| Value::Object(Default::default()))
    }

    /// Normalize: ensure title visible, drop unknown built-ins (keep `cf:`), clamp order.
    pub fn sanitized(mut self) -> Self {
        if self.columns.is_empty() {
            return Self::default_config();
        }

        let mut seen = std::collections::HashSet::new();
        self.columns.retain(|c| {
            if !seen.insert(c.id.clone()) {
                return false;
            }
            is_allowed_column_id(&c.id)
        });

        // Ensure every required column exists and is visible.
        for req in CASE_LIST_REQUIRED_COLUMNS {
            if let Some(col) = self.columns.iter_mut().find(|c| c.id == *req) {
                col.visible = true;
            } else {
                self.columns.insert(
                    0,
                    CaseListColumnPref {
                        id: (*req).to_string(),
                        visible: true,
                    },
                );
            }
        }

        // Append any missing built-ins (hidden) so the settings menu stays complete.
        for id in CASE_LIST_BUILTIN_COLUMNS {
            if !self.columns.iter().any(|c| c.id == *id) {
                self.columns.push(CaseListColumnPref {
                    id: (*id).to_string(),
                    visible: CASE_LIST_DEFAULT_VISIBLE.contains(id),
                });
            }
        }

        if let Some(order) = self.order.as_mut() {
            let o = order.trim().to_lowercase();
            *order = if o == "desc" {
                "desc".into()
            } else {
                "asc".into()
            };
        }

        if let Some(sort) = &self.sort {
            if !is_allowed_sort_field(sort) {
                self.sort = None;
                self.order = None;
            }
        }

        self
    }

    pub fn validate(cfg: &CaseListColumnsConfig) -> Result<CaseListColumnsConfig, String> {
        if cfg.columns.is_empty() {
            return Err("columns must not be empty".into());
        }
        for col in &cfg.columns {
            if !is_allowed_column_id(&col.id) {
                return Err(format!("unknown column id '{}'", col.id));
            }
            if CASE_LIST_REQUIRED_COLUMNS.contains(&col.id.as_str()) && !col.visible {
                return Err(format!("column '{}' cannot be hidden", col.id));
            }
        }
        if !cfg.columns.iter().any(|c| c.id == "title" && c.visible) {
            return Err("title column must be present and visible".into());
        }
        if let Some(sort) = &cfg.sort {
            if !is_allowed_sort_field(sort) {
                return Err(format!("invalid sort field '{sort}'"));
            }
        }
        if let Some(order) = &cfg.order {
            let o = order.trim().to_lowercase();
            if o != "asc" && o != "desc" {
                return Err("order must be asc or desc".into());
            }
        }
        Ok(cfg.clone().sanitized())
    }
}

pub fn is_custom_field_column_id(id: &str) -> bool {
    id.strip_prefix("cf:")
        .and_then(|s| Uuid::parse_str(s).ok())
        .is_some()
}

pub fn parse_custom_field_column_id(id: &str) -> Option<Uuid> {
    id.strip_prefix("cf:").and_then(|s| Uuid::parse_str(s).ok())
}

pub fn is_allowed_column_id(id: &str) -> bool {
    CASE_LIST_BUILTIN_COLUMNS.contains(&id) || is_custom_field_column_id(id)
}

pub fn is_allowed_sort_field(sort: &str) -> bool {
    CASE_LIST_SORT_FIELDS.contains(&sort) || is_custom_field_column_id(sort)
}

pub fn custom_field_column_id(field_id: Uuid) -> String {
    format!("cf:{field_id}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_keeps_title_visible() {
        let cfg = CaseListColumnsConfig::default_config();
        let title = cfg.columns.iter().find(|c| c.id == "title").unwrap();
        assert!(title.visible);
        assert!(cfg
            .columns
            .iter()
            .any(|c| c.id == "automated" && !c.visible));
    }

    #[test]
    fn sanitize_forces_title_visible() {
        let cfg = CaseListColumnsConfig {
            columns: vec![
                CaseListColumnPref {
                    id: "title".into(),
                    visible: false,
                },
                CaseListColumnPref {
                    id: "key".into(),
                    visible: true,
                },
            ],
            sort: Some("bogus".into()),
            order: Some("DESC".into()),
        }
        .sanitized();
        assert!(
            cfg.columns
                .iter()
                .find(|c| c.id == "title")
                .unwrap()
                .visible
        );
        assert!(cfg.sort.is_none());
        assert_eq!(cfg.order.as_deref(), None);
    }

    #[test]
    fn validate_rejects_hiding_title() {
        let err = CaseListColumnsConfig::validate(&CaseListColumnsConfig {
            columns: vec![CaseListColumnPref {
                id: "title".into(),
                visible: false,
            }],
            sort: None,
            order: None,
        })
        .unwrap_err();
        assert!(err.contains("cannot be hidden"));
    }

    #[test]
    fn cf_column_ids_roundtrip() {
        let id = Uuid::nil();
        let col = custom_field_column_id(id);
        assert_eq!(parse_custom_field_column_id(&col), Some(id));
        assert!(is_allowed_column_id(&col));
        assert!(is_allowed_sort_field(&col));
    }
}
