use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde_json::Value;
use tokio::fs;

use super::{cfg_str, BackupDestination, RemoteObject};
use crate::error::{AppError, AppResult};

pub struct LocalDestination {
    root: PathBuf,
}

impl LocalDestination {
    pub fn from_config(config: &Value) -> AppResult<Self> {
        let path = cfg_str(config, "path").unwrap_or_else(|| {
            std::env::var("VULYK_CONTENT_BACKUP_DIR")
                .unwrap_or_else(|_| "./data/content-backups".into())
        });
        Ok(Self {
            root: PathBuf::from(path),
        })
    }

    fn resolve(&self, remote_path: &str) -> AppResult<PathBuf> {
        let cleaned = remote_path.trim_start_matches('/');
        if cleaned.contains("..") {
            return Err(AppError::BadRequest("path traversal rejected".into()));
        }
        Ok(self.root.join(cleaned))
    }
}

#[async_trait]
impl BackupDestination for LocalDestination {
    fn kind(&self) -> &'static str {
        "local"
    }

    async fn test_connection(&self) -> AppResult<()> {
        fs::create_dir_all(&self.root)
            .await
            .map_err(|e| AppError::BadRequest(format!("local path: {e}")))?;
        let probe = self.root.join(".vulyk-backup-probe");
        fs::write(&probe, b"ok")
            .await
            .map_err(|e| AppError::BadRequest(format!("local write: {e}")))?;
        let _ = fs::remove_file(&probe).await;
        Ok(())
    }

    async fn put_file(&self, remote_path: &str, bytes: &[u8]) -> AppResult<()> {
        let path = self.resolve(remote_path)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| AppError::Internal(format!("mkdir: {e}")))?;
        }
        fs::write(&path, bytes)
            .await
            .map_err(|e| AppError::Internal(format!("write: {e}")))?;
        Ok(())
    }

    async fn list(&self, remote_prefix: &str) -> AppResult<Vec<RemoteObject>> {
        let dir = self.resolve(remote_prefix)?;
        let mut out = Vec::new();
        if !dir.exists() {
            return Ok(out);
        }
        list_recursive(&self.root, &dir, &mut out).await?;
        Ok(out)
    }

    async fn get_file(&self, remote_path: &str) -> AppResult<Vec<u8>> {
        let path = self.resolve(remote_path)?;
        fs::read(&path)
            .await
            .map_err(|e| AppError::NotFound(format!("file: {e}")))
    }

    async fn delete(&self, remote_path: &str) -> AppResult<()> {
        let path = self.resolve(remote_path)?;
        if path.is_dir() {
            fs::remove_dir_all(&path)
                .await
                .map_err(|e| AppError::Internal(format!("rmdir: {e}")))?;
        } else if path.exists() {
            fs::remove_file(&path)
                .await
                .map_err(|e| AppError::Internal(format!("unlink: {e}")))?;
        }
        Ok(())
    }
}

async fn list_recursive(root: &Path, dir: &Path, out: &mut Vec<RemoteObject>) -> AppResult<()> {
    let mut rd = fs::read_dir(dir)
        .await
        .map_err(|e| AppError::Internal(format!("readdir: {e}")))?;
    while let Some(entry) = rd
        .next_entry()
        .await
        .map_err(|e| AppError::Internal(format!("readdir: {e}")))?
    {
        let path = entry.path();
        if path.is_dir() {
            Box::pin(list_recursive(root, &path, out)).await?;
        } else {
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            let meta = entry
                .metadata()
                .await
                .map_err(|e| AppError::Internal(format!("meta: {e}")))?;
            out.push(RemoteObject {
                path: rel,
                size_bytes: Some(meta.len()),
            });
        }
    }
    Ok(())
}
