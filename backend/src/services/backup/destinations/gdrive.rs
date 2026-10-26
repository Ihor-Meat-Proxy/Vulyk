//! Google Drive destination (OAuth2 refresh token or service-account-style access token).
//!
//! Config: `{ "folder_id": "..." }`
//! Secrets: `{ "access_token": "..." }` or `{ "refresh_token": "...", "client_id": "...", "client_secret": "..." }`

use async_trait::async_trait;
use serde_json::{json, Value};

use super::{cfg_str, require_str, BackupDestination, RemoteObject};
use crate::error::{AppError, AppResult};

pub struct GoogleDriveDestination {
    folder_id: String,
    access_token: String,
    client: reqwest::Client,
}

impl GoogleDriveDestination {
    pub fn from_config(config: &Value, secrets: &Value) -> AppResult<Self> {
        let folder_id = require_str(config, "folder_id")?;
        // Prefer ready access_token (tests / short-lived); else caller must refresh externally
        // and store access_token. Full refresh-token exchange supported when client_id/secret set.
        let access_token = cfg_str(secrets, "access_token");
        let client = reqwest::Client::new();
        // Note: async refresh happens in test_connection / ensure_token — stored here if present
        let token = access_token
            .or_else(|| cfg_str(secrets, "token"))
            .unwrap_or_default();
        Ok(Self {
            folder_id,
            access_token: token,
            client,
        })
    }

    async fn ensure_token(&self, _secrets: &Value) -> AppResult<String> {
        if self.access_token.is_empty() {
            return Err(AppError::BadRequest(
                "gdrive access_token (or pre-refreshed token) required".into(),
            ));
        }
        Ok(self.access_token.clone())
    }

    /// Build from config+secrets with optional refresh.
    pub async fn connect(config: &Value, secrets: &Value) -> AppResult<Self> {
        let folder_id = require_str(config, "folder_id")?;
        let client = reqwest::Client::new();
        let mut access_token = cfg_str(secrets, "access_token")
            .or_else(|| cfg_str(secrets, "token"))
            .unwrap_or_default();

        if access_token.is_empty() {
            if let Some(refresh) = cfg_str(secrets, "refresh_token") {
                let client_id = cfg_str(secrets, "client_id")
                    .or_else(|| cfg_str(config, "client_id"))
                    .ok_or_else(|| AppError::BadRequest("gdrive client_id required".into()))?;
                let client_secret = cfg_str(secrets, "client_secret")
                    .or_else(|| cfg_str(config, "client_secret"))
                    .ok_or_else(|| AppError::BadRequest("gdrive client_secret required".into()))?;
                let res = client
                    .post("https://oauth2.googleapis.com/token")
                    .form(&[
                        ("client_id", client_id.as_str()),
                        ("client_secret", client_secret.as_str()),
                        ("refresh_token", refresh.as_str()),
                        ("grant_type", "refresh_token"),
                    ])
                    .send()
                    .await
                    .map_err(|e| AppError::BadRequest(format!("gdrive token: {e}")))?;
                if !res.status().is_success() {
                    let status = res.status();
                    let body = res.text().await.unwrap_or_default();
                    return Err(AppError::BadRequest(format!(
                        "gdrive token {status}: {body}"
                    )));
                }
                let v: Value = res
                    .json()
                    .await
                    .map_err(|e| AppError::BadRequest(format!("gdrive token json: {e}")))?;
                access_token = v
                    .get("access_token")
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .into();
            }
        }

        if access_token.is_empty() {
            return Err(AppError::BadRequest(
                "gdrive access_token or refresh_token required".into(),
            ));
        }

        Ok(Self {
            folder_id,
            access_token,
            client,
        })
    }
}

#[async_trait]
impl BackupDestination for GoogleDriveDestination {
    fn kind(&self) -> &'static str {
        "gdrive"
    }

    async fn test_connection(&self) -> AppResult<()> {
        let token = self.ensure_token(&Value::Null).await?;
        let url = format!(
            "https://www.googleapis.com/drive/v3/files/{}?fields=id,name",
            self.folder_id
        );
        let res = self
            .client
            .get(&url)
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| AppError::BadRequest(format!("gdrive: {e}")))?;
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(AppError::BadRequest(format!("gdrive {status}: {body}")));
        }
        Ok(())
    }

    async fn put_file(&self, remote_path: &str, bytes: &[u8]) -> AppResult<()> {
        let token = self.ensure_token(&Value::Null).await?;
        let name = remote_path.rsplit('/').next().unwrap_or(remote_path);
        let metadata = json!({
            "name": name,
            "parents": [self.folder_id],
        });
        let meta_str = serde_json::to_string(&metadata)
            .map_err(|e| AppError::Internal(format!("gdrive meta: {e}")))?;

        // multipart related upload
        let boundary = "vulyk_backup_boundary";
        let mut body = Vec::new();
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(b"Content-Type: application/json; charset=UTF-8\r\n\r\n");
        body.extend_from_slice(meta_str.as_bytes());
        body.extend_from_slice(b"\r\n");
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(b"Content-Type: application/octet-stream\r\n\r\n");
        body.extend_from_slice(bytes);
        body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());

        let res = self
            .client
            .post("https://www.googleapis.com/upload/drive/v3/files?uploadType=multipart")
            .bearer_auth(&token)
            .header(
                "Content-Type",
                format!("multipart/related; boundary={boundary}"),
            )
            .body(body)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("gdrive put: {e}")))?;
        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            return Err(AppError::Internal(format!("gdrive put {status}: {text}")));
        }
        Ok(())
    }

    async fn list(&self, _remote_prefix: &str) -> AppResult<Vec<RemoteObject>> {
        let token = self.ensure_token(&Value::Null).await?;
        let q = format!("'{}' in parents and trashed=false", self.folder_id);
        let res = self
            .client
            .get("https://www.googleapis.com/drive/v3/files")
            .query(&[
                ("q", q.as_str()),
                ("fields", "files(id,name,size)"),
                ("pageSize", "100"),
            ])
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("gdrive list: {e}")))?;
        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            return Err(AppError::Internal(format!("gdrive list {status}: {text}")));
        }
        let v: Value = res
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("gdrive list json: {e}")))?;
        let files = v
            .get("files")
            .and_then(|f| f.as_array())
            .cloned()
            .unwrap_or_default();
        Ok(files
            .into_iter()
            .map(|f| RemoteObject {
                path: f.get("name").and_then(|n| n.as_str()).unwrap_or("").into(),
                size_bytes: f
                    .get("size")
                    .and_then(|s| s.as_str())
                    .and_then(|s| s.parse().ok()),
            })
            .collect())
    }

    async fn get_file(&self, remote_path: &str) -> AppResult<Vec<u8>> {
        let token = self.ensure_token(&Value::Null).await?;
        // Resolve by name in folder
        let name = remote_path.rsplit('/').next().unwrap_or(remote_path);
        let q = format!(
            "name = '{}' and '{}' in parents and trashed=false",
            name.replace('\'', "\\'"),
            self.folder_id
        );
        let res = self
            .client
            .get("https://www.googleapis.com/drive/v3/files")
            .query(&[
                ("q", q.as_str()),
                ("fields", "files(id)"),
                ("pageSize", "1"),
            ])
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("gdrive find: {e}")))?;
        let v: Value = res
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("gdrive find json: {e}")))?;
        let id = v
            .get("files")
            .and_then(|f| f.as_array())
            .and_then(|a| a.first())
            .and_then(|f| f.get("id"))
            .and_then(|i| i.as_str())
            .ok_or_else(|| AppError::NotFound("gdrive file".into()))?;
        let dl = self
            .client
            .get(format!(
                "https://www.googleapis.com/drive/v3/files/{id}?alt=media"
            ))
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("gdrive get: {e}")))?;
        if !dl.status().is_success() {
            return Err(AppError::NotFound("gdrive file content".into()));
        }
        dl.bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| AppError::Internal(format!("gdrive body: {e}")))
    }

    async fn delete(&self, remote_path: &str) -> AppResult<()> {
        let token = self.ensure_token(&Value::Null).await?;
        let name = remote_path.rsplit('/').next().unwrap_or(remote_path);
        let q = format!(
            "name = '{}' and '{}' in parents and trashed=false",
            name.replace('\'', "\\'"),
            self.folder_id
        );
        let res = self
            .client
            .get("https://www.googleapis.com/drive/v3/files")
            .query(&[
                ("q", q.as_str()),
                ("fields", "files(id)"),
                ("pageSize", "1"),
            ])
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("gdrive find: {e}")))?;
        let v: Value = res
            .json()
            .await
            .map_err(|e| AppError::Internal(format!("gdrive find json: {e}")))?;
        let Some(id) = v
            .get("files")
            .and_then(|f| f.as_array())
            .and_then(|a| a.first())
            .and_then(|f| f.get("id"))
            .and_then(|i| i.as_str())
        else {
            return Ok(());
        };
        let del = self
            .client
            .delete(format!("https://www.googleapis.com/drive/v3/files/{id}"))
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| AppError::Internal(format!("gdrive delete: {e}")))?;
        if !del.status().is_success() && del.status().as_u16() != 404 {
            let status = del.status();
            let text = del.text().await.unwrap_or_default();
            return Err(AppError::Internal(format!(
                "gdrive delete {status}: {text}"
            )));
        }
        Ok(())
    }
}
