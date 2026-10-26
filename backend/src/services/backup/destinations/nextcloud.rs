use async_trait::async_trait;
use serde_json::Value;

use super::{cfg_str, require_str, BackupDestination, RemoteObject};
use crate::error::{AppError, AppResult};

pub struct NextcloudDestination {
    base_url: String,
    username: String,
    app_password: String,
    root_path: String,
    client: reqwest::Client,
}

impl NextcloudDestination {
    pub fn from_config(config: &Value, secrets: &Value) -> AppResult<Self> {
        let base_url = require_str(config, "base_url")?;
        let username = require_str(config, "username")?;
        let app_password = cfg_str(secrets, "app_password")
            .or_else(|| cfg_str(secrets, "password"))
            .or_else(|| cfg_str(config, "app_password"))
            .ok_or_else(|| AppError::BadRequest("nextcloud app_password required".into()))?;
        let root_path = cfg_str(config, "path").unwrap_or_else(|| "VulykBackups".into());
        Ok(Self {
            base_url: base_url.trim_end_matches('/').into(),
            username,
            app_password,
            root_path,
            client: reqwest::Client::new(),
        })
    }

    fn dav_url(&self, remote_path: &str) -> String {
        let remote = remote_path.trim_start_matches('/');
        let root = self.root_path.trim_matches('/');
        let joined = if remote.is_empty() {
            root.to_string()
        } else if root.is_empty() {
            remote.to_string()
        } else {
            format!("{root}/{remote}")
        };
        format!(
            "{}/remote.php/dav/files/{}/{}",
            self.base_url,
            urlencoding::encode(&self.username),
            joined
        )
    }
}

#[async_trait]
impl BackupDestination for NextcloudDestination {
    fn kind(&self) -> &'static str {
        "nextcloud"
    }

    async fn test_connection(&self) -> AppResult<()> {
        let url = self.dav_url("");
        let res = self
            .client
            .request(
                reqwest::Method::from_bytes(b"PROPFIND").unwrap_or(reqwest::Method::GET),
                &url,
            )
            .basic_auth(&self.username, Some(&self.app_password))
            .header("Depth", "0")
            .send()
            .await
            .map_err(|e| AppError::BadRequest(format!("nextcloud: {e}")))?;
        let status = res.status().as_u16();
        if status == 404 {
            // Root may not exist yet — try create via MKCOL on parent pieces later; accept auth OK on 401 check
            // Fall through: if unauthorized, fail; if not found, still ok for test if credentials accepted
        }
        if status == 401 || status == 403 {
            let body = res.text().await.unwrap_or_default();
            return Err(AppError::BadRequest(format!(
                "nextcloud auth failed ({status}): {body}"
            )));
        }
        Ok(())
    }

    async fn put_file(&self, remote_path: &str, bytes: &[u8]) -> AppResult<()> {
        // Ensure parent collections exist
        let parts: Vec<&str> = remote_path
            .trim_matches('/')
            .split('/')
            .filter(|p| !p.is_empty())
            .collect();
        if parts.len() > 1 {
            let mut acc = String::new();
            for part in &parts[..parts.len() - 1] {
                if !acc.is_empty() {
                    acc.push('/');
                }
                acc.push_str(part);
                let mk = self.dav_url(&acc);
                let _ = self
                    .client
                    .request(
                        reqwest::Method::from_bytes(b"MKCOL").unwrap_or(reqwest::Method::PUT),
                        &mk,
                    )
                    .basic_auth(&self.username, Some(&self.app_password))
                    .send()
                    .await;
            }
        }

        let url = self.dav_url(remote_path);
        let res = self
            .client
            .put(&url)
            .basic_auth(&self.username, Some(&self.app_password))
            .body(bytes.to_vec())
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("nextcloud put: {e}")))?;
        if !res.status().is_success()
            && res.status().as_u16() != 201
            && res.status().as_u16() != 204
        {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            return Err(AppError::Internal(format!(
                "nextcloud put {status}: {text}"
            )));
        }
        Ok(())
    }

    async fn list(&self, remote_prefix: &str) -> AppResult<Vec<RemoteObject>> {
        let url = self.dav_url(remote_prefix);
        let res = self
            .client
            .request(
                reqwest::Method::from_bytes(b"PROPFIND").unwrap_or(reqwest::Method::GET),
                &url,
            )
            .basic_auth(&self.username, Some(&self.app_password))
            .header("Depth", "1")
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("nextcloud list: {e}")))?;
        if res.status().as_u16() == 404 {
            return Ok(vec![]);
        }
        let text = res.text().await.unwrap_or_default();
        // Minimal href scrape — sufficient for retention listing in tests/mocks
        let mut out = Vec::new();
        for line in text.lines() {
            let trimmed = line.trim();
            if let Some(start) = trimmed.find("<d:href>") {
                if let Some(end) = trimmed.find("</d:href>") {
                    let href = &trimmed[start + 8..end];
                    if let Some(idx) = href.find("/dav/files/") {
                        let rest = &href[idx + 11..];
                        if let Some(slash) = rest.find('/') {
                            let path = rest[slash + 1..].trim_end_matches('/');
                            if !path.is_empty() && path != remote_prefix.trim_matches('/') {
                                out.push(RemoteObject {
                                    path: path.into(),
                                    size_bytes: None,
                                });
                            }
                        }
                    }
                }
            }
        }
        Ok(out)
    }

    async fn get_file(&self, remote_path: &str) -> AppResult<Vec<u8>> {
        let url = self.dav_url(remote_path);
        let res = self
            .client
            .get(&url)
            .basic_auth(&self.username, Some(&self.app_password))
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("nextcloud get: {e}")))?;
        if !res.status().is_success() {
            return Err(AppError::NotFound("nextcloud file".into()));
        }
        res.bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| AppError::Internal(format!("nextcloud body: {e}")))
    }

    async fn delete(&self, remote_path: &str) -> AppResult<()> {
        let url = self.dav_url(remote_path);
        let res = self
            .client
            .delete(&url)
            .basic_auth(&self.username, Some(&self.app_password))
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("nextcloud delete: {e}")))?;
        if !res.status().is_success() && res.status().as_u16() != 404 {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            return Err(AppError::Internal(format!(
                "nextcloud delete {status}: {text}"
            )));
        }
        Ok(())
    }
}
