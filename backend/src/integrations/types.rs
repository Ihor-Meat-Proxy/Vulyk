use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;

/// Runtime config assembled from the `integrations` row (base_url + config + secrets).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationConfig {
    pub base_url: String,
    pub config: Value,
    pub secrets: Value,
}

impl IntegrationConfig {
    pub fn str_config(&self, key: &str) -> Option<String> {
        self.config
            .get(key)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty())
    }

    pub fn str_secret(&self, key: &str) -> Option<String> {
        self.secrets
            .get(key)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RemoteRequirement {
    pub external_id: String,
    pub external_key: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub raw: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RequirementLink {
    pub requirement_id: Uuid,
    pub case_id: Uuid,
    pub external_id: String,
    pub external_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct DefectDraft {
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub steps_to_reproduce: String,
    pub case_key: Option<String>,
    pub result_status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RemoteIssue {
    pub external_id: String,
    pub external_key: String,
    pub url: String,
    #[serde(default)]
    pub raw: Value,
}
