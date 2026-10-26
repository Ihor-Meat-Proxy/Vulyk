use async_trait::async_trait;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use serde_json::{json, Value};

use super::{cfg_str, require_str, BackupDestination, RemoteObject};
use crate::error::{AppError, AppResult};

pub struct GitHubDestination {
    owner: String,
    repo: String,
    branch: String,
    path_prefix: String,
    token: String,
    api_base: String,
    client: reqwest::Client,
}

impl GitHubDestination {
    pub fn from_config(config: &Value, secrets: &Value) -> AppResult<Self> {
        let owner = require_str(config, "owner")?;
        let repo = require_str(config, "repo")?;
        let branch = cfg_str(config, "branch").unwrap_or_else(|| "main".into());
        let path_prefix = cfg_str(config, "path").unwrap_or_default();
        let token = cfg_str(secrets, "token")
            .or_else(|| cfg_str(config, "token"))
            .ok_or_else(|| AppError::BadRequest("github token required".into()))?;
        let api_base =
            cfg_str(config, "api_base").unwrap_or_else(|| "https://api.github.com".into());
        crate::services::egress::validate_public_https_url(&api_base)?;
        Ok(Self {
            owner,
            repo,
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

    fn contents_url(&self, path: &str) -> String {
        format!(
            "{}/repos/{}/{}/contents/{}",
            self.api_base.trim_end_matches('/'),
            self.owner,
            self.repo,
            path.trim_start_matches('/')
        )
    }
}

#[async_trait]
impl BackupDestination for GitHubDestination {
    fn kind(&self) -> &'static str {
        "github"
    }

    async fn test_connection(&self) -> AppResult<()> {
        let url = format!(
            "{}/repos/{}/{}",
            self.api_base.trim_end_matches('/'),
            self.owner,
            self.repo
        );
        let res = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("User-Agent", "vulyk-backup")
            .header("Accept", "application/vnd.github+json")
            .send()
            .await
            .map_err(|e| AppError::BadRequest(format!("github: {e}")))?;
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(AppError::BadRequest(format!("github {status}: {body}")));
        }
        Ok(())
    }

    async fn put_file(&self, remote_path: &str, bytes: &[u8]) -> AppResult<()> {
        let path = self.full_path(remote_path);
        let url = self.contents_url(&path);

        // Fetch existing sha for update
        let existing = self
            .client
            .get(&url)
            .query(&[("ref", self.branch.as_str())])
            .header("Authorization", format!("Bearer {}", self.token))
            .header("User-Agent", "vulyk-backup")
            .header("Accept", "application/vnd.github+json")
            .send()
            .await
            .ok();

        let mut body = json!({
            "message": format!("vulyk backup: {path}"),
            "content": B64.encode(bytes),
            "branch": self.branch,
        });
        if let Some(res) = existing {
            if res.status().is_success() {
                if let Ok(v) = res.json::<Value>().await {
                    if let Some(sha) = v.get("sha").and_then(|s| s.as_str()) {
                        body["sha"] = json!(sha);
                    }
                }
            }
        }

        let res = self
            .client
            .put(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("User-Agent", "vulyk-backup")
            .header("Accept", "application/vnd.github+json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("github put: {e}")))?;
        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            return Err(AppError::Internal(format!("github put {status}: {text}")));
        }
        Ok(())
    }

    async fn list(&self, remote_prefix: &str) -> AppResult<Vec<RemoteObject>> {
        let path = self.full_path(remote_prefix);
        let url = self.contents_url(&path);
        let res = self
            .client
            .get(&url)
            .query(&[("ref", self.branch.as_str())])
            .header("Authorization", format!("Bearer {}", self.token))
            .header("User-Agent", "vulyk-backup")
            .header("Accept", "application/vnd.github+json")
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("github list: {e}")))?;
        if res.status().as_u16() == 404 {
            return Ok(vec![]);
        }
        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            return Err(AppError::Internal(format!("github list {status}: {text}")));
        }
        let v: Value = res
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("github list json: {e}")))?;
        let mut out = Vec::new();
        let items = if v.is_array() {
            v.as_array().cloned().unwrap_or_default()
        } else {
            vec![v]
        };
        for item in items {
            if item.get("type").and_then(|t| t.as_str()) == Some("file") {
                out.push(RemoteObject {
                    path: item
                        .get("path")
                        .and_then(|p| p.as_str())
                        .unwrap_or("")
                        .into(),
                    size_bytes: item.get("size").and_then(|s| s.as_u64()),
                });
            }
        }
        Ok(out)
    }

    async fn get_file(&self, remote_path: &str) -> AppResult<Vec<u8>> {
        let path = self.full_path(remote_path);
        let url = self.contents_url(&path);
        let res = self
            .client
            .get(&url)
            .query(&[("ref", self.branch.as_str())])
            .header("Authorization", format!("Bearer {}", self.token))
            .header("User-Agent", "vulyk-backup")
            .header("Accept", "application/vnd.github+json")
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("github get: {e}")))?;
        if !res.status().is_success() {
            return Err(AppError::NotFound("github file".into()));
        }
        let v: Value = res
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("github get json: {e}")))?;
        let content = v
            .get("content")
            .and_then(|c| c.as_str())
            .ok_or_else(|| AppError::Internal("github missing content".into()))?;
        let cleaned: String = content.chars().filter(|c| !c.is_whitespace()).collect();
        B64.decode(cleaned)
            .map_err(|e| AppError::Internal(format!("github b64: {e}")))
    }

    async fn delete(&self, remote_path: &str) -> AppResult<()> {
        let path = self.full_path(remote_path);
        let url = self.contents_url(&path);
        let meta = self
            .client
            .get(&url)
            .query(&[("ref", self.branch.as_str())])
            .header("Authorization", format!("Bearer {}", self.token))
            .header("User-Agent", "vulyk-backup")
            .header("Accept", "application/vnd.github+json")
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("github delete meta: {e}")))?;
        if meta.status().as_u16() == 404 {
            return Ok(());
        }
        let v: Value = meta
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("github delete json: {e}")))?;
        let sha = v
            .get("sha")
            .and_then(|s| s.as_str())
            .ok_or_else(|| AppError::Internal("github delete missing sha".into()))?;
        let res = self
            .client
            .delete(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("User-Agent", "vulyk-backup")
            .header("Accept", "application/vnd.github+json")
            .json(&json!({
                "message": format!("vulyk backup prune: {path}"),
                "sha": sha,
                "branch": self.branch,
            }))
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("github delete: {e}")))?;
        if !res.status().is_success() && res.status().as_u16() != 404 {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            return Err(AppError::Internal(format!(
                "github delete {status}: {text}"
            )));
        }
        Ok(())
    }
}
