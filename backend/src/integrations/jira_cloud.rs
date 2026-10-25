//! Jira Cloud adapter — OAuth / email+API token (Basic).

use async_trait::async_trait;
use base64::Engine;
use serde_json::{json, Value};

use super::{
    AlmIntegration, DefectDraft, IntegrationConfig, RemoteIssue, RemoteRequirement, RequirementLink,
};
use crate::error::{AppError, AppResult};

pub struct JiraCloudAdapter;

#[async_trait]
impl AlmIntegration for JiraCloudAdapter {
    fn id(&self) -> &'static str {
        "jira_cloud"
    }

    async fn list_requirements(
        &self,
        cfg: &IntegrationConfig,
        query: &str,
    ) -> AppResult<Vec<RemoteRequirement>> {
        if let Some(fixture) = cfg.config.get("fixture_requirements") {
            return parse_fixture_requirements(fixture);
        }

        let project = cfg
            .str_config("project_key")
            .ok_or_else(|| AppError::BadRequest("jira_cloud config.project_key required".into()))?;
        let issue_types = cfg
            .str_config("requirement_issue_types")
            .unwrap_or_else(|| "Story,Epic".into());
        let mut jql = format!("project = {project} AND issuetype in ({issue_types})");
        if !query.is_empty() {
            jql.push_str(&format!(" AND text ~ \"{}\"", query.replace('"', "\\\"")));
        }
        let url = format!("{}/rest/api/3/search", cfg.base_url.trim_end_matches('/'));
        let body = authenticated_post(
            cfg,
            &url,
            json!({
                "jql": jql,
                "maxResults": 50,
                "fields": ["summary", "description", "issuetype", "key"]
            }),
        )
        .await?;
        parse_search_results(&cfg.base_url, &body)
    }

    async fn link_requirement(
        &self,
        cfg: &IntegrationConfig,
        link: &RequirementLink,
    ) -> AppResult<()> {
        if cfg.config.get("fixture_requirements").is_some() {
            return Ok(());
        }
        // Remote comment documenting the Vulyk link (local link is authoritative).
        let url = format!(
            "{}/rest/api/3/issue/{}/comment",
            cfg.base_url.trim_end_matches('/'),
            link.external_key
        );
        let _ = authenticated_post(
            cfg,
            &url,
            json!({
                "body": {
                    "type": "doc",
                    "version": 1,
                    "content": [{
                        "type": "paragraph",
                        "content": [{
                            "type": "text",
                            "text": format!("Linked to Vulyk case {}", link.case_id)
                        }]
                    }]
                }
            }),
        )
        .await?;
        Ok(())
    }

    async fn create_defect(
        &self,
        cfg: &IntegrationConfig,
        defect: DefectDraft,
    ) -> AppResult<RemoteIssue> {
        if let Some(fixture) = cfg.config.get("fixture_defect") {
            return parse_fixture_issue(fixture);
        }

        let project = cfg
            .str_config("project_key")
            .ok_or_else(|| AppError::BadRequest("jira_cloud config.project_key required".into()))?;
        let issue_type = cfg
            .str_config("defect_issue_type")
            .unwrap_or_else(|| "Bug".into());
        let description = format!(
            "{}\n\nSteps to reproduce:\n{}\n\nCase: {}\nResult: {}",
            defect.description,
            defect.steps_to_reproduce,
            defect.case_key.as_deref().unwrap_or("-"),
            defect.result_status.as_deref().unwrap_or("failed")
        );
        let url = format!("{}/rest/api/3/issue", cfg.base_url.trim_end_matches('/'));
        let body = authenticated_post(
            cfg,
            &url,
            json!({
                "fields": {
                    "project": { "key": project },
                    "summary": defect.title,
                    "issuetype": { "name": issue_type },
                    "description": {
                        "type": "doc",
                        "version": 1,
                        "content": [{
                            "type": "paragraph",
                            "content": [{ "type": "text", "text": description }]
                        }]
                    }
                }
            }),
        )
        .await?;
        issue_from_create(&cfg.base_url, &body)
    }
}

fn auth_header(cfg: &IntegrationConfig) -> AppResult<String> {
    if let Some(token) = cfg.str_secret("oauth_access_token") {
        return Ok(format!("Bearer {token}"));
    }
    let email = cfg.str_secret("email").ok_or_else(|| {
        AppError::BadRequest("jira_cloud secrets.email or oauth_access_token required".into())
    })?;
    let api_token = cfg.str_secret("api_token").ok_or_else(|| {
        AppError::BadRequest("jira_cloud secrets.api_token required with email".into())
    })?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(format!("{email}:{api_token}"));
    Ok(format!("Basic {encoded}"))
}

async fn authenticated_post(cfg: &IntegrationConfig, url: &str, body: Value) -> AppResult<Value> {
    let client = reqwest::Client::new();
    let res = client
        .post(url)
        .header("Authorization", auth_header(cfg)?)
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("jira_cloud request failed: {e}")))?;
    if !res.status().is_success() {
        let status = res.status();
        let text = res.text().await.unwrap_or_default();
        return Err(AppError::BadRequest(format!(
            "jira_cloud HTTP {status}: {text}"
        )));
    }
    res.json()
        .await
        .map_err(|e| AppError::BadRequest(format!("jira_cloud json: {e}")))
}

fn parse_search_results(base_url: &str, body: &Value) -> AppResult<Vec<RemoteRequirement>> {
    let issues = body
        .get("issues")
        .and_then(|v| v.as_array())
        .ok_or_else(|| AppError::BadRequest("jira: missing issues".into()))?;
    Ok(issues
        .iter()
        .filter_map(|issue| issue_to_requirement(base_url, issue).ok())
        .collect())
}

fn issue_to_requirement(base_url: &str, issue: &Value) -> AppResult<RemoteRequirement> {
    let key = issue
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("jira issue missing key".into()))?
        .to_string();
    let id = issue
        .get("id")
        .map(|v| v.to_string().trim_matches('"').to_string())
        .unwrap_or_else(|| key.clone());
    let title = issue
        .pointer("/fields/summary")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    Ok(RemoteRequirement {
        external_id: id,
        external_key: key.clone(),
        title,
        description: String::new(),
        url: format!("{}/browse/{key}", base_url.trim_end_matches('/')),
        raw: issue.clone(),
    })
}

fn issue_from_create(base_url: &str, body: &Value) -> AppResult<RemoteIssue> {
    let key = body
        .get("key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("jira create missing key".into()))?
        .to_string();
    let id = body
        .get("id")
        .map(|v| v.to_string().trim_matches('"').to_string())
        .unwrap_or_else(|| key.clone());
    Ok(RemoteIssue {
        external_id: id,
        external_key: key.clone(),
        url: format!("{}/browse/{key}", base_url.trim_end_matches('/')),
        raw: body.clone(),
    })
}

fn parse_fixture_requirements(fixture: &Value) -> AppResult<Vec<RemoteRequirement>> {
    if let Some(arr) = fixture.as_array() {
        return arr
            .iter()
            .map(|v| {
                serde_json::from_value(v.clone())
                    .map_err(|e| AppError::BadRequest(format!("fixture: {e}")))
            })
            .collect();
    }
    parse_search_results("https://example.atlassian.net", fixture)
}

fn parse_fixture_issue(fixture: &Value) -> AppResult<RemoteIssue> {
    if fixture.get("external_key").is_some() {
        return serde_json::from_value(fixture.clone())
            .map_err(|e| AppError::BadRequest(format!("fixture: {e}")));
    }
    issue_from_create("https://example.atlassian.net", fixture)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn jira_cloud_fixture_search() {
        let cfg = IntegrationConfig {
            base_url: "https://example.atlassian.net".into(),
            config: json!({
                "fixture_requirements": {
                    "issues": [
                        {
                            "id": "10001",
                            "key": "DEMO-1",
                            "fields": { "summary": "As a user I can log in" }
                        }
                    ]
                }
            }),
            secrets: json!({}),
        };
        let list = JiraCloudAdapter
            .list_requirements(&cfg, "")
            .await
            .expect("list");
        assert_eq!(list[0].external_key, "DEMO-1");
    }
}
