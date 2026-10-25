use serde_json::Value;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone)]
pub struct CiDispatchHandle {
    pub external_id: Option<String>,
    pub external_url: Option<String>,
    pub status: String,
}

pub fn require_str(cfg: &Value, key: &str) -> AppResult<String> {
    cfg.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::BadRequest(format!("ci config missing '{key}'")))
}

pub fn optional_str(cfg: &Value, key: &str) -> Option<String> {
    cfg.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
}
