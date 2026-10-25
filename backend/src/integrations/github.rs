//! GitHub Actions: `workflow_dispatch` + Actions webhook status sync.

use hmac::{Hmac, Mac};
use serde_json::{json, Value};
use sha2::Sha256;

use super::ci::{optional_str, require_str, CiDispatchHandle};
use crate::error::{AppError, AppResult};

type HmacSha256 = Hmac<Sha256>;

pub async fn dispatch(cfg: &Value, inputs: &Value) -> AppResult<CiDispatchHandle> {
    let token = require_str(cfg, "token")?;
    let owner = require_str(cfg, "owner")?;
    let repo = require_str(cfg, "repo")?;
    let workflow_id = require_str(cfg, "workflow_id")?;
    let git_ref = optional_str(cfg, "ref").unwrap_or_else(|| "main".into());
    let api_base = optional_str(cfg, "api_base").unwrap_or_else(|| "https://api.github.com".into());

    let body = json!({
        "ref": git_ref,
        "inputs": inputs.as_object().cloned().unwrap_or_default(),
    });

    let url = format!("{api_base}/repos/{owner}/{repo}/actions/workflows/{workflow_id}/dispatches");
    let client = reqwest::Client::new();
    let res = client
        .post(&url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .header("User-Agent", "vulyk-ci")
        .bearer_auth(&token)
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("GitHub request failed: {e}")))?;

    if !(res.status().is_success() || res.status().as_u16() == 204) {
        let status = res.status();
        let text = res.text().await.unwrap_or_default();
        return Err(AppError::BadRequest(format!(
            "GitHub workflow_dispatch failed ({status}): {text}"
        )));
    }

    // workflow_dispatch returns 204 with no run id; callers correlate via webhook.
    Ok(CiDispatchHandle {
        external_id: None,
        external_url: Some(format!(
            "https://github.com/{owner}/{repo}/actions/workflows/{workflow_id}"
        )),
        status: "dispatched".into(),
    })
}

pub fn verify_signature(secret: &str, body: &[u8], signature_header: &str) -> bool {
    let Some(sig) = signature_header.strip_prefix("sha256=") else {
        return false;
    };
    let Ok(mut mac) = HmacSha256::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    mac.update(body);
    let expected = hex::encode(mac.finalize().into_bytes());
    // Constant-time-ish compare
    expected.len() == sig.len()
        && expected
            .bytes()
            .zip(sig.bytes())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0
}

/// Map GitHub `workflow_run` / `check_suite` style payloads to a dispatch status.
pub fn status_from_workflow_run(payload: &Value) -> Option<(String, String, Option<String>)> {
    let run = payload.get("workflow_run")?;
    let external_id = run.get("id")?.as_i64()?.to_string();
    let conclusion = run.get("conclusion").and_then(|v| v.as_str()).unwrap_or("");
    let status = run.get("status").and_then(|v| v.as_str()).unwrap_or("");
    let url = run
        .get("html_url")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let mapped = match (status, conclusion) {
        ("queued", _) | ("waiting", _) | ("requested", _) => "dispatched",
        ("in_progress", _) => "running",
        ("completed", "success") => "succeeded",
        ("completed", "cancelled") => "cancelled",
        ("completed", _) => "failed",
        _ => "running",
    };
    Some((external_id, mapped.into(), url))
}
