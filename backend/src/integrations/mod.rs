//! External system adapters: ALM (W6) + CI providers (W5).
//!
//! ALM clients talk to OpenProject / Jira over HTTP. This repository never
//! opens PRs, issues, or commits against those upstreams.
//!
//! CI adapters dispatch GitHub Actions / GitLab pipelines and verify webhooks.

pub mod ci;
pub mod github;
pub mod gitlab;
pub mod jira_cloud;
pub mod jira_dc;
pub mod openproject;
pub mod types;

use async_trait::async_trait;

pub use types::*;

use crate::error::{AppError, AppResult};

#[async_trait]
pub trait AlmIntegration: Send + Sync {
    fn id(&self) -> &'static str;
    async fn list_requirements(
        &self,
        cfg: &IntegrationConfig,
        query: &str,
    ) -> AppResult<Vec<RemoteRequirement>>;
    async fn link_requirement(
        &self,
        cfg: &IntegrationConfig,
        link: &RequirementLink,
    ) -> AppResult<()>;
    async fn create_defect(
        &self,
        cfg: &IntegrationConfig,
        defect: DefectDraft,
    ) -> AppResult<RemoteIssue>;
}

pub fn adapter_for(kind: &str) -> AppResult<&'static dyn AlmIntegration> {
    match kind {
        "openproject" => Ok(&openproject::OpenProjectAdapter),
        "jira_cloud" => Ok(&jira_cloud::JiraCloudAdapter),
        "jira_dc" => Ok(&jira_dc::JiraDcAdapter),
        other => Err(AppError::BadRequest(format!(
            "unknown integration kind: {other}"
        ))),
    }
}

pub fn known_kinds() -> &'static [&'static str] {
    &["openproject", "jira_cloud", "jira_dc"]
}
