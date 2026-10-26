//! Pluggable backup destinations.

mod gdrive;
mod github;
mod gitlab;
mod local;
mod nextcloud;

use async_trait::async_trait;
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::crypto::SecretsKey;
use crate::error::{AppError, AppResult};

pub use gdrive::GoogleDriveDestination;
pub use github::GitHubDestination;
pub use gitlab::GitLabDestination;
pub use local::LocalDestination;
pub use nextcloud::NextcloudDestination;

#[derive(Debug, Clone)]
pub struct RemoteObject {
    pub path: String,
    pub size_bytes: Option<u64>,
}

#[async_trait]
pub trait BackupDestination: Send + Sync {
    fn kind(&self) -> &'static str;

    async fn test_connection(&self) -> AppResult<()>;

    async fn put_file(&self, remote_path: &str, bytes: &[u8]) -> AppResult<()>;

    async fn put_tree(&self, remote_root: &str, files: &[(String, Vec<u8>)]) -> AppResult<()> {
        for (rel, bytes) in files {
            let path = format!(
                "{}/{}",
                remote_root.trim_end_matches('/'),
                rel.trim_start_matches('/')
            );
            self.put_file(&path, bytes).await?;
        }
        Ok(())
    }

    async fn list(&self, remote_prefix: &str) -> AppResult<Vec<RemoteObject>>;

    async fn get_file(&self, remote_path: &str) -> AppResult<Vec<u8>>;

    async fn delete(&self, remote_path: &str) -> AppResult<()>;
}

pub async fn build_destination(
    pool: &PgPool,
    secrets: &SecretsKey,
    destination_id: Uuid,
) -> AppResult<Box<dyn BackupDestination>> {
    let row: Option<(String, Value, Option<Vec<u8>>, Option<Vec<u8>>)> = sqlx::query_as(
        r#"SELECT kind, config, secrets_ciphertext, secrets_nonce
           FROM backup_destinations WHERE id = $1 AND enabled = TRUE"#,
    )
    .bind(destination_id)
    .fetch_optional(pool)
    .await?;

    let Some((kind, config, ct, nonce)) = row else {
        return Err(AppError::NotFound("backup destination".into()));
    };

    let secrets_json = match (ct, nonce) {
        (Some(ct), Some(nonce)) if !ct.is_empty() => {
            let plain = secrets.decrypt(&ct, &nonce)?;
            serde_json::from_str(&plain).unwrap_or_else(|_| Value::Object(Default::default()))
        }
        _ => Value::Object(Default::default()),
    };

    from_config(&kind, &config, &secrets_json)
}

pub fn from_config(
    kind: &str,
    config: &Value,
    secrets: &Value,
) -> AppResult<Box<dyn BackupDestination>> {
    match kind {
        "local" => Ok(Box::new(LocalDestination::from_config(config)?)),
        "github" => Ok(Box::new(GitHubDestination::from_config(config, secrets)?)),
        "gitlab" => Ok(Box::new(GitLabDestination::from_config(config, secrets)?)),
        "nextcloud" => Ok(Box::new(NextcloudDestination::from_config(
            config, secrets,
        )?)),
        "gdrive" => Ok(Box::new(GoogleDriveDestination::from_config(
            config, secrets,
        )?)),
        other => Err(AppError::BadRequest(format!(
            "unsupported destination kind '{other}'"
        ))),
    }
}

pub(crate) fn cfg_str(config: &Value, key: &str) -> Option<String> {
    config
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

pub(crate) fn require_str(config: &Value, key: &str) -> AppResult<String> {
    cfg_str(config, key).ok_or_else(|| AppError::BadRequest(format!("missing config.{key}")))
}
