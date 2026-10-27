use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct CiConfig {
    pub id: Uuid,
    pub project_id: Uuid,
    pub provider: String,
    pub name: String,
    #[schema(value_type = Object)]
    pub config: Value,
    /// Never serialize to API clients — use [`CiConfigPublic`].
    #[serde(skip_serializing)]
    pub webhook_secret: Option<String>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// API-safe CI config: provider tokens and webhook secrets are redacted.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CiConfigPublic {
    pub id: Uuid,
    pub project_id: Uuid,
    pub provider: String,
    pub name: String,
    #[schema(value_type = Object)]
    pub config: Value,
    pub has_token: bool,
    pub has_webhook_secret: bool,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

const SECRET_CONFIG_KEYS: &[&str] = &["token", "trigger_token", "private_token", "access_token"];

impl From<CiConfig> for CiConfigPublic {
    fn from(value: CiConfig) -> Self {
        let (config, has_token) = redact_ci_config(value.config);
        let has_webhook_secret = value.webhook_secret.as_ref().is_some_and(|s| !s.is_empty());
        Self {
            id: value.id,
            project_id: value.project_id,
            provider: value.provider,
            name: value.name,
            config,
            has_token,
            has_webhook_secret,
            enabled: value.enabled,
            created_at: value.created_at,
            updated_at: value.updated_at,
        }
    }
}

/// Strip secret keys from CI provider config JSON; return (redacted, had_any_secret).
pub fn redact_ci_config(mut config: Value) -> (Value, bool) {
    let mut has_token = false;
    if let Some(obj) = config.as_object_mut() {
        for key in SECRET_CONFIG_KEYS {
            if let Some(v) = obj.remove(*key) {
                let present = match &v {
                    Value::Null => false,
                    Value::String(s) => !s.is_empty(),
                    _ => true,
                };
                if present {
                    has_token = true;
                }
            }
        }
    }
    (config, has_token)
}

/// Merge update config into existing, preserving secret keys when omitted.
/// Empty string / null for a secret key clears it.
pub fn merge_ci_config(existing: &Value, incoming: Value) -> Value {
    let Some(inc) = incoming.as_object() else {
        return incoming;
    };
    let mut out = if existing.is_object() {
        existing.clone()
    } else {
        json!({})
    };
    {
        let out_obj = out.as_object_mut().expect("object");
        for (k, v) in inc {
            if SECRET_CONFIG_KEYS.contains(&k.as_str()) {
                match v {
                    Value::Null => {
                        out_obj.remove(k);
                    }
                    Value::String(s) if s.is_empty() => {
                        out_obj.remove(k);
                    }
                    other => {
                        out_obj.insert(k.clone(), other.clone());
                    }
                }
            } else {
                out_obj.insert(k.clone(), v.clone());
            }
        }
        // Preserve existing secrets when the update omitted those keys entirely.
        if let Some(existing_obj) = existing.as_object() {
            for key in SECRET_CONFIG_KEYS {
                if !inc.contains_key(*key) {
                    if let Some(prev) = existing_obj.get(*key) {
                        out_obj.insert((*key).to_string(), prev.clone());
                    }
                }
            }
        }
    }
    out
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateCiConfig {
    /// `github` or `gitlab`
    pub provider: String,
    pub name: String,
    #[schema(value_type = Object)]
    pub config: Value,
    pub webhook_secret: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateCiConfig {
    pub name: Option<String>,
    #[schema(value_type = Object)]
    pub config: Option<Value>,
    pub webhook_secret: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow, ToSchema)]
pub struct CiDispatch {
    pub id: Uuid,
    pub project_id: Uuid,
    pub ci_config_id: Uuid,
    pub run_id: Option<Uuid>,
    pub status: String,
    pub external_id: Option<String>,
    pub external_url: Option<String>,
    #[schema(value_type = Object)]
    pub inputs: Value,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct DispatchCiRequest {
    #[serde(default)]
    #[schema(value_type = Object)]
    pub inputs: Value,
    /// Optional Vulyk run to attach; created automatically when omitted.
    pub run_id: Option<Uuid>,
    pub run_name: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct WebhookAck {
    pub received: bool,
    pub dispatch_id: Option<Uuid>,
    pub status: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn redacts_tokens_and_flags_presence() {
        let (redacted, has) = redact_ci_config(json!({
            "token": "ghp_secret",
            "owner": "org",
            "repo": "repo"
        }));
        assert!(has);
        assert!(redacted.get("token").is_none());
        assert_eq!(redacted["owner"], "org");
    }

    #[test]
    fn merge_preserves_omitted_token() {
        let existing = json!({"token": "keep", "owner": "a"});
        let merged = merge_ci_config(&existing, json!({"owner": "b", "ref": "main"}));
        assert_eq!(merged["token"], "keep");
        assert_eq!(merged["owner"], "b");
        assert_eq!(merged["ref"], "main");
    }

    #[test]
    fn merge_empty_token_clears() {
        let existing = json!({"token": "keep", "owner": "a"});
        let merged = merge_ci_config(&existing, json!({"token": ""}));
        assert!(merged.get("token").is_none());
    }
}
