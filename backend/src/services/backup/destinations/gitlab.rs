use async_trait::async_trait;
use serde_json::{json, Value};

use super::{cfg_str, BackupDestination, RemoteObject};
use crate::error::{AppError, AppResult};

pub struct GitLabDestination {
    project_id: String,
    branch: String,
    path_prefix: String,
    token: String,
    api_base: String,
    client: reqwest::Client,
}

impl GitLabDestination {
    pub fn from_config(config: &Value, secrets: &Value) -> AppResult<Self> {
        let project_id = cfg_str(config, "project_id")
            .or_else(|| cfg_str(config, "project"))
            .ok_or_else(|| AppError::BadRequest("missing config.project_id".into()))?;
        let branch = cfg_str(config, "branch").unwrap_or_else(|| "main".into());
        let path_prefix = cfg_str(config, "path").unwrap_or_default();
        let token = cfg_str(secrets, "token")
            .or_else(|| cfg_str(config, "token"))
            .ok_or_else(|| AppError::BadRequest("gitlab token required".into()))?;
        let api_base =
            cfg_str(config, "api_base").unwrap_or_else(|| "https://gitlab.com/api/v4".into());
        crate::services::egress::validate_public_https_url(&api_base)?;
        Ok(Self {
            project_id,
            branch,
            path_prefix,
            token,
            api_base,
            client: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(|e| AppError::Internal(format!("http client: {e}")))?,
        })
    }

    fn full_path(&self, remote_path: &str) -> String {
        let remote = remote_path.trim_start_matches('/');
        if self.path_prefix.is_empty() {
            remote.into()
        } else {
            format!("{}/{}", self.path_prefix.trim_end_matches('/'), remote)
        }
    }

    fn file_url(&self, path: &str) -> String {
        let encoded = urlencoding::encode(path);
        format!(
            "{}/projects/{}/repository/files/{}",
            self.api_base.trim_end_matches('/'),
            urlencoding::encode(&self.project_id),
            encoded
        )
    }
}

#[async_trait]
impl BackupDestination for GitLabDestination {
    fn kind(&self) -> &'static str {
        "gitlab"
    }

    async fn test_connection(&self) -> AppResult<()> {
        let url = format!(
            "{}/projects/{}",
            self.api_base.trim_end_matches('/'),
            urlencoding::encode(&self.project_id)
        );
        let res = self
            .client
            .get(&url)
            .header("PRIVATE-TOKEN", &self.token)
            .send()
            .await
            .map_err(|e| AppError::BadRequest(format!("gitlab: {e}")))?;
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(AppError::BadRequest(format!("gitlab {status}: {body}")));
        }
        Ok(())
    }

    async fn put_file(&self, remote_path: &str, bytes: &[u8]) -> AppResult<()> {
        let path = self.full_path(remote_path);
        let url = self.file_url(&path);
        let content = String::from_utf8_lossy(bytes);
        // Try update first, then create
        let update = self
            .client
            .put(&url)
            .header("PRIVATE-TOKEN", &self.token)
            .json(&json!({
                "branch": self.branch,
                "content": content.as_ref(),
                "commit_message": format!("vulyk backup: {path}"),
            }))
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("gitlab put: {e}")))?;
        if update.status().is_success() {
            return Ok(());
        }
        let create = self
            .client
            .post(&url)
            .header("PRIVATE-TOKEN", &self.token)
            .json(&json!({
                "branch": self.branch,
                "content": content.as_ref(),
                "commit_message": format!("vulyk backup: {path}"),
            }))
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("gitlab create: {e}")))?;
        if !create.status().is_success() {
            let status = create.status();
            let text = create.text().await.unwrap_or_default();
            return Err(AppError::Internal(format!("gitlab put {status}: {text}")));
        }
        Ok(())
    }

    async fn list(&self, remote_prefix: &str) -> AppResult<Vec<RemoteObject>> {
        let path = self.full_path(remote_prefix);
        let url = format!(
            "{}/projects/{}/repository/tree",
            self.api_base.trim_end_matches('/'),
            urlencoding::encode(&self.project_id)
        );
        let res = self
            .client
            .get(&url)
            .query(&[
                ("ref", self.branch.as_str()),
                ("path", path.as_str()),
                ("recursive", "true"),
                ("per_page", "100"),
            ])
            .header("PRIVATE-TOKEN", &self.token)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("gitlab list: {e}")))?;
        if res.status().as_u16() == 404 {
            return Ok(vec![]);
        }
        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            return Err(AppError::Internal(format!("gitlab list {status}: {text}")));
        }
        let items: Vec<Value> = res
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("gitlab list json: {e}")))?;
        Ok(items
            .into_iter()
            .filter(|i| i.get("type").and_then(|t| t.as_str()) == Some("blob"))
            .map(|i| RemoteObject {
                path: i.get("path").and_then(|p| p.as_str()).unwrap_or("").into(),
                size_bytes: None,
            })
            .collect())
    }

    async fn get_file(&self, remote_path: &str) -> AppResult<Vec<u8>> {
        let path = self.full_path(remote_path);
        let url = self.file_url(&path);
        let res = self
            .client
            .get(&url)
            .query(&[("ref", self.branch.as_str())])
            .header("PRIVATE-TOKEN", &self.token)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("gitlab get: {e}")))?;
        if !res.status().is_success() {
            return Err(AppError::NotFound("gitlab file".into()));
        }
        let v: Value = res
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("gitlab get json: {e}")))?;
        let content = v
            .get("content")
            .and_then(|c| c.as_str())
            .ok_or_else(|| AppError::Internal("gitlab missing content".into()))?;
        let encoding = v
            .get("encoding")
            .and_then(|e| e.as_str())
            .unwrap_or("base64");
        if encoding == "base64" {
            use base64::engine::general_purpose::STANDARD as B64;
            use base64::Engine;
            let cleaned: String = content.chars().filter(|c| !c.is_whitespace()).collect();
            B64.decode(cleaned)
                .map_err(|e| AppError::Internal(format!("gitlab b64: {e}")))
        } else {
            Ok(content.as_bytes().to_vec())
        }
    }

    async fn delete(&self, remote_path: &str) -> AppResult<()> {
        let path = self.full_path(remote_path);
        let url = self.file_url(&path);
        let res = self
            .client
            .delete(&url)
            .header("PRIVATE-TOKEN", &self.token)
            .json(&json!({
                "branch": self.branch,
                "commit_message": format!("vulyk backup prune: {path}"),
            }))
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("gitlab delete: {e}")))?;
        if !res.status().is_success() && res.status().as_u16() != 404 {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            return Err(AppError::Internal(format!(
                "gitlab delete {status}: {text}"
            )));
        }
        Ok(())
    }
}
