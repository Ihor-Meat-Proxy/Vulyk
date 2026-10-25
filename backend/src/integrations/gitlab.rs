//! GitLab: pipeline trigger API + pipeline webhook status sync.

use serde_json::{json, Value};

use super::ci::{optional_str, require_str, CiDispatchHandle};
use crate::error::{AppError, AppResult};

pub async fn dispatch(cfg: &Value, inputs: &Value) -> AppResult<CiDispatchHandle> {
    let base = optional_str(cfg, "base_url").unwrap_or_else(|| "https://gitlab.com".into());
    let project_id = require_str(cfg, "project_id")?;
    let trigger_token = require_str(cfg, "trigger_token")?;
    let git_ref = optional_str(cfg, "ref").unwrap_or_else(|| "main".into());

    let mut form = vec![
        ("token".to_string(), trigger_token),
        ("ref".to_string(), git_ref),
    ];

    if let Some(obj) = inputs.as_object() {
        for (k, v) in obj {
            let val = match v {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            form.push((format!("variables[{k}]"), val));
        }
    }

    // Also allow variables nested under config.default_variables
    if let Some(defaults) = cfg.get("default_variables").and_then(|v| v.as_object()) {
        for (k, v) in defaults {
            if inputs.get(k).is_none() {
                let val = match v {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                form.push((format!("variables[{k}]"), val));
            }
        }
    }

    let url = format!(
        "{}/api/v4/projects/{}/trigger/pipeline",
        base.trim_end_matches('/'),
        urlencoding_path(&project_id)
    );

    let client = reqwest::Client::new();
    let res = client
        .post(&url)
        .form(&form)
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("GitLab request failed: {e}")))?;

    if !res.status().is_success() {
        let status = res.status();
        let text = res.text().await.unwrap_or_default();
        return Err(AppError::BadRequest(format!(
            "GitLab pipeline trigger failed ({status}): {text}"
        )));
    }

    let body: Value = res
        .json()
        .await
        .map_err(|e| AppError::BadRequest(format!("GitLab response decode failed: {e}")))?;

    let external_id = body.get("id").and_then(|v| {
        v.as_i64()
            .map(|n| n.to_string())
            .or_else(|| v.as_str().map(|s| s.to_string()))
    });
    let external_url = body
        .get("web_url")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    Ok(CiDispatchHandle {
        external_id,
        external_url,
        status: "dispatched".into(),
    })
}

fn urlencoding_path(s: &str) -> String {
    // Encode path segments like `group/project` for GitLab API.
    s.split('/')
        .map(|part| {
            let mut out = String::new();
            for b in part.bytes() {
                match b {
                    b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' => {
                        out.push(b as char);
                    }
                    _ => out.push_str(&format!("%{b:02X}")),
                }
            }
            out
        })
        .collect::<Vec<_>>()
        .join("%2F")
}

pub fn verify_token(secret: &str, header_token: Option<&str>) -> bool {
    match header_token {
        Some(t) => {
            t.len() == secret.len()
                && t.bytes()
                    .zip(secret.bytes())
                    .fold(0u8, |acc, (a, b)| acc | (a ^ b))
                    == 0
        }
        None => false,
    }
}

pub fn status_from_pipeline(payload: &Value) -> Option<(String, String, Option<String>)> {
    // Pipeline hook: object_attributes
    let attrs = payload
        .get("object_attributes")
        .cloned()
        .or_else(|| payload.get("pipeline").cloned())
        .unwrap_or(json!({}));

    let external_id = attrs.get("id").and_then(|v| {
        v.as_i64()
            .map(|n| n.to_string())
            .or_else(|| v.as_str().map(|s| s.to_string()))
    })?;
    let status = attrs.get("status").and_then(|v| v.as_str()).unwrap_or("");
    let url = attrs
        .get("url")
        .or_else(|| payload.get("project").and_then(|p| p.get("web_url")))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let mapped = match status {
        "created" | "pending" | "waiting_for_resource" | "preparing" => "dispatched",
        "running" => "running",
        "success" => "succeeded",
        "canceled" | "cancelled" => "cancelled",
        "failed" | "skipped" => "failed",
        _ => "running",
    };
    Some((external_id, mapped.into(), url))
}
