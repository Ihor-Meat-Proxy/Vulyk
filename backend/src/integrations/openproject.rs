//! OpenProject APIv3 client (work packages as requirements / defects).
//! Documentation only lives in this repo — we do not contribute upstream.

use async_trait::async_trait;
use base64::Engine;
use serde_json::{json, Value};

use super::{
    AlmIntegration, DefectDraft, IntegrationConfig, RemoteIssue, RemoteRequirement, RequirementLink,
};
use crate::error::{AppError, AppResult};

pub struct OpenProjectAdapter;

#[async_trait]
impl AlmIntegration for OpenProjectAdapter {
    fn id(&self) -> &'static str {
        "openproject"
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
            .str_config("project_id")
            .ok_or_else(|| AppError::BadRequest("openproject config.project_id required".into()))?;
        let type_id = cfg.str_config("requirement_type_id");
        let mut filters = vec![json!({"project":{"operator":"=","values":[project]}})];
        if let Some(tid) = type_id {
            filters.push(json!({"type":{"operator":"=","values":[tid]}}));
        }
        if !query.is_empty() {
            filters.push(json!({"subject":{"operator":"**","values":[query]}}));
        }
        let filters_param = serde_json::to_string(&filters)
            .map_err(|e| AppError::BadRequest(format!("filter encode: {e}")))?;
        let url = format!(
            "{}/api/v3/work_packages?filters={}",
            cfg.base_url.trim_end_matches('/'),
            urlencoding_minimal(&filters_param)
        );
        let body = authenticated_get(cfg, &url).await?;
        parse_work_packages(&cfg.base_url, &body)
    }

    async fn link_requirement(
        &self,
        cfg: &IntegrationConfig,
        link: &RequirementLink,
    ) -> AppResult<()> {
        // Persist is owned by Vulyk services; optional remote custom field write.
        if cfg.config.get("fixture_requirements").is_some() {
            return Ok(());
        }
        let field = match cfg.str_config("link_custom_field") {
            Some(f) => f,
            None => return Ok(()),
        };
        let url = format!(
            "{}/api/v3/work_packages/{}",
            cfg.base_url.trim_end_matches('/'),
            link.external_id
        );
        let payload = json!({
            field: format!("vulyk:case:{}", link.case_id),
        });
        let _ = authenticated_patch(cfg, &url, payload).await?;
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
            .str_config("project_id")
            .ok_or_else(|| AppError::BadRequest("openproject config.project_id required".into()))?;
        let type_id = cfg
            .str_config("defect_type_id")
            .unwrap_or_else(|| "1".into());
        let description = format!(
            "{}\n\nSteps to reproduce:\n{}\n\nCase: {}\nResult: {}",
            defect.description,
            defect.steps_to_reproduce,
            defect.case_key.as_deref().unwrap_or("-"),
            defect.result_status.as_deref().unwrap_or("failed")
        );
        let url = format!(
            "{}/api/v3/work_packages",
            cfg.base_url.trim_end_matches('/')
        );
        let payload = json!({
            "subject": defect.title,
            "description": { "format": "plain", "raw": description },
            "_links": {
                "type": { "href": format!("/api/v3/types/{type_id}") },
                "project": { "href": format!("/api/v3/projects/{project}") }
            }
        });
        let body = authenticated_post(cfg, &url, payload).await?;
        work_package_to_issue(&cfg.base_url, &body)
    }
}

fn urlencoding_minimal(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            _ => format!("%{:02X}", c as u8),
        })
        .collect()
}

fn api_token(cfg: &IntegrationConfig) -> AppResult<String> {
    cfg.str_secret("api_token")
        .ok_or_else(|| AppError::BadRequest("openproject secrets.api_token required".into()))
}

fn auth_header(cfg: &IntegrationConfig) -> AppResult<String> {
    let token = api_token(cfg)?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(format!("apikey:{token}"));
    Ok(format!("Basic {encoded}"))
}

async fn authenticated_get(cfg: &IntegrationConfig, url: &str) -> AppResult<Value> {
    let client = reqwest::Client::new();
    let res = client
        .get(url)
        .header("Authorization", auth_header(cfg)?)
        .header("Accept", "application/hal+json")
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("openproject request failed: {e}")))?;
    if !res.status().is_success() {
        let status = res.status();
        let text = res.text().await.unwrap_or_default();
        return Err(AppError::BadRequest(format!(
            "openproject HTTP {status}: {text}"
        )));
    }
    res.json()
        .await
        .map_err(|e| AppError::BadRequest(format!("openproject json: {e}")))
}

async fn authenticated_post(cfg: &IntegrationConfig, url: &str, body: Value) -> AppResult<Value> {
    let client = reqwest::Client::new();
    let res = client
        .post(url)
        .header("Authorization", auth_header(cfg)?)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("openproject request failed: {e}")))?;
    if !res.status().is_success() {
        let status = res.status();
        let text = res.text().await.unwrap_or_default();
        return Err(AppError::BadRequest(format!(
            "openproject HTTP {status}: {text}"
        )));
    }
    res.json()
        .await
        .map_err(|e| AppError::BadRequest(format!("openproject json: {e}")))
}

async fn authenticated_patch(cfg: &IntegrationConfig, url: &str, body: Value) -> AppResult<Value> {
    let client = reqwest::Client::new();
    let res = client
        .patch(url)
        .header("Authorization", auth_header(cfg)?)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("openproject request failed: {e}")))?;
    if !res.status().is_success() {
        let status = res.status();
        let text = res.text().await.unwrap_or_default();
        return Err(AppError::BadRequest(format!(
            "openproject HTTP {status}: {text}"
        )));
    }
    res.json()
        .await
        .map_err(|e| AppError::BadRequest(format!("openproject json: {e}")))
}

fn parse_work_packages(base_url: &str, body: &Value) -> AppResult<Vec<RemoteRequirement>> {
    let elements = body
        .pointer("/_embedded/elements")
        .and_then(|v| v.as_array())
        .ok_or_else(|| AppError::BadRequest("openproject: missing _embedded.elements".into()))?;
    elements
        .iter()
        .map(|wp| work_package_to_requirement(base_url, wp))
        .collect()
}

fn work_package_to_requirement(base_url: &str, wp: &Value) -> AppResult<RemoteRequirement> {
    let id = wp
        .get("id")
        .map(|v| v.to_string().trim_matches('"').to_string())
        .ok_or_else(|| AppError::BadRequest("work package missing id".into()))?;
    let title = wp
        .get("subject")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let description = wp
        .pointer("/description/raw")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    Ok(RemoteRequirement {
        external_key: format!("WP-{id}"),
        external_id: id.clone(),
        title,
        description,
        url: format!("{}/work_packages/{id}", base_url.trim_end_matches('/')),
        raw: wp.clone(),
    })
}

fn work_package_to_issue(base_url: &str, wp: &Value) -> AppResult<RemoteIssue> {
    let id = wp
        .get("id")
        .map(|v| v.to_string().trim_matches('"').to_string())
        .ok_or_else(|| AppError::BadRequest("work package missing id".into()))?;
    Ok(RemoteIssue {
        external_key: format!("WP-{id}"),
        external_id: id.clone(),
        url: format!("{}/work_packages/{id}", base_url.trim_end_matches('/')),
        raw: wp.clone(),
    })
}

fn parse_fixture_requirements(fixture: &Value) -> AppResult<Vec<RemoteRequirement>> {
    if let Some(arr) = fixture.as_array() {
        return arr
            .iter()
            .map(|v| {
                serde_json::from_value(v.clone())
                    .map_err(|e| AppError::BadRequest(format!("fixture requirement: {e}")))
            })
            .collect();
    }
    parse_work_packages("https://openproject.example.test", fixture)
}

fn parse_fixture_issue(fixture: &Value) -> AppResult<RemoteIssue> {
    if fixture.get("external_key").is_some() {
        return serde_json::from_value(fixture.clone())
            .map_err(|e| AppError::BadRequest(format!("fixture defect: {e}")));
    }
    work_package_to_issue("https://openproject.example.test", fixture)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn lists_requirements_from_fixture() {
        let cfg = IntegrationConfig {
            base_url: "https://openproject.example.test".into(),
            config: json!({
                "fixture_requirements": [
                    {
                        "external_id": "42",
                        "external_key": "WP-42",
                        "title": "Login required",
                        "description": "Users must authenticate",
                        "url": "https://openproject.example.test/work_packages/42",
                        "raw": {}
                    }
                ]
            }),
            secrets: json!({}),
        };
        let list = OpenProjectAdapter
            .list_requirements(&cfg, "")
            .await
            .expect("list");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].external_key, "WP-42");
    }

    #[tokio::test]
    async fn creates_defect_from_fixture() {
        let cfg = IntegrationConfig {
            base_url: "https://openproject.example.test".into(),
            config: json!({
                "fixture_defect": {
                    "id": 99,
                    "subject": "Bug"
                }
            }),
            secrets: json!({}),
        };
        let issue = OpenProjectAdapter
            .create_defect(
                &cfg,
                DefectDraft {
                    title: "Fail".into(),
                    description: "x".into(),
                    steps_to_reproduce: "1".into(),
                    case_key: Some("C-1".into()),
                    result_status: Some("failed".into()),
                },
            )
            .await
            .expect("defect");
        assert_eq!(issue.external_key, "WP-99");
    }
}
