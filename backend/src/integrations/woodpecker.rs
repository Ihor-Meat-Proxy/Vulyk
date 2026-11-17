//! Woodpecker CI: manual pipeline trigger + short status poll.
//!
//! Auth: `Authorization: Bearer <PAT>`.
//! Trigger: `POST {base}/api/repos/{repo_id}/pipelines` with `{ branch, variables }`.
//! Lookup: `GET {base}/api/repos/lookup/{owner}%2F{name}` when `repo_id` is omitted.

use std::time::Duration;

use serde_json::{json, Map, Value};
use tokio::time::sleep;

use super::ci::{optional_str, require_str, CiDispatchHandle};
use crate::error::{AppError, AppResult};
use crate::services::egress::{self, CiUrlPolicy};

#[derive(Debug, Clone)]
struct WoodpeckerTarget {
    base_url: String,
    public_base: String,
    token: String,
    repo: String,
    repo_id: Option<i64>,
    branch: String,
}

fn client() -> AppResult<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| AppError::BadRequest(format!("HTTP client error: {e}")))
}

fn parse_target(cfg: &Value, policy: &CiUrlPolicy) -> AppResult<WoodpeckerTarget> {
    let base_url = require_str(cfg, "base_url")?
        .trim_end_matches('/')
        .to_string();
    let token = require_str(cfg, "token")?;
    let repo = require_str(cfg, "repo")?.trim_matches('/').to_string();
    if !repo.contains('/') {
        return Err(AppError::BadRequest(
            "woodpecker config 'repo' must be 'owner/name'".into(),
        ));
    }
    let branch = optional_str(cfg, "branch").unwrap_or_else(|| "main".into());
    let repo_id = cfg
        .get("repo_id")
        .and_then(|v| v.as_i64().or_else(|| v.as_str()?.parse().ok()));
    let public_base = optional_str(cfg, "public_base_url")
        .map(|s| s.trim_end_matches('/').to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| base_url.clone());

    egress::validate_ci_url(&base_url, policy)?;
    if public_base != base_url {
        egress::validate_ci_url(&public_base, policy)?;
    }

    Ok(WoodpeckerTarget {
        base_url,
        public_base,
        token,
        repo,
        repo_id,
        branch,
    })
}

fn auth(http: reqwest::RequestBuilder, token: &str) -> reqwest::RequestBuilder {
    http.header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/json")
        .header("User-Agent", "vulyk-ci")
}

fn encode_repo_path(repo: &str) -> String {
    repo.split('/')
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

async fn resolve_repo_id(
    http: &reqwest::Client,
    target: &WoodpeckerTarget,
) -> AppResult<(i64, String)> {
    if let Some(id) = target.repo_id {
        return Ok((id, target.repo.clone()));
    }
    let url = format!(
        "{}/api/repos/lookup/{}",
        target.base_url,
        encode_repo_path(&target.repo)
    );
    let res = auth(http.get(&url), &target.token)
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("Woodpecker lookup failed: {e}")))?;
    if !res.status().is_success() {
        let status = res.status();
        let text = res.text().await.unwrap_or_default();
        return Err(AppError::BadRequest(format!(
            "Woodpecker repo lookup failed ({status}): {text}"
        )));
    }
    let body: Value = res
        .json()
        .await
        .map_err(|e| AppError::BadRequest(format!("Woodpecker lookup decode failed: {e}")))?;
    let id = body
        .get("id")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| AppError::BadRequest("Woodpecker lookup missing repo id".into()))?;
    let full_name = body
        .get("full_name")
        .and_then(|v| v.as_str())
        .unwrap_or(&target.repo)
        .to_string();
    Ok((id, full_name))
}

fn pipeline_ui_url(public_base: &str, full_name: &str, number: i64) -> String {
    format!(
        "{}/repos/{}/pipeline/{}",
        public_base.trim_end_matches('/'),
        full_name.trim_matches('/'),
        number
    )
}

fn map_status(raw: &str) -> &'static str {
    match raw.trim().to_ascii_lowercase().as_str() {
        "pending" | "created" | "blocked" => "dispatched",
        "running" | "started" => "running",
        "success" | "successed" => "succeeded",
        "failure" | "error" | "killed" | "declined" | "skipped" => "failed",
        _ => "dispatched",
    }
}

fn pipeline_number(body: &Value) -> Option<i64> {
    body.get("number").and_then(|v| {
        v.as_i64()
            .or_else(|| v.as_u64().map(|n| n as i64))
            .or_else(|| v.as_str()?.parse().ok())
    })
}

/// Probe credentials + repository lookup, and warn when the token cannot push/trigger.
///
/// Woodpecker rejects pipeline triggers with a browser session cookie/Bearer that
/// only passes `/api/user`. Operators must use a **personal access token** with
/// repo push / pipeline rights.
pub async fn test_connection(cfg: &Value, policy: &CiUrlPolicy) -> AppResult<(bool, String)> {
    let target = parse_target(cfg, policy)?;
    let http = client()?;

    let user_url = format!("{}/api/user", target.base_url);
    let user_res = auth(http.get(&user_url), &target.token)
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("Woodpecker /api/user failed: {e}")))?;
    if !user_res.status().is_success() {
        let status = user_res.status();
        let text = user_res.text().await.unwrap_or_default();
        return Err(AppError::BadRequest(format!(
            "Woodpecker /api/user failed ({status}): {text}. Use a personal access token (PAT), not a browser session."
        )));
    }
    let user_body: Value = user_res.json().await.unwrap_or_else(|_| json!({}));
    let login = user_body
        .get("login")
        .or_else(|| user_body.get("username"))
        .and_then(|v| v.as_str())
        .unwrap_or("token");

    let (repo_id, full_name) = resolve_repo_id(&http, &target).await?;

    // Prefer explicit permissions on the repo object when the API provides them.
    let repo_url = format!("{}/api/repos/{repo_id}", target.base_url);
    let repo_res = auth(http.get(&repo_url), &target.token).send().await;
    if let Ok(repo_res) = repo_res {
        if repo_res.status().is_success() {
            if let Ok(repo_body) = repo_res.json::<Value>().await {
                let push = repo_body
                    .pointer("/permissions/push")
                    .and_then(|v| v.as_bool())
                    .or_else(|| {
                        repo_body
                            .get("permissions")
                            .and_then(|p| p.get("push"))
                            .and_then(|v| v.as_bool())
                    });
                if push == Some(false) {
                    return Ok((
                        false,
                        format!(
                            "Connected as {login}; repo '{full_name}' is readable but this token cannot push/trigger pipelines. Create a Woodpecker personal access token with repo push rights (session cookies are not enough)."
                        ),
                    ));
                }
            }
        } else if matches!(repo_res.status().as_u16(), 401 | 403) {
            return Ok((
                false,
                format!(
                    "Connected as {login}; repo lookup succeeded but GET /api/repos/{repo_id} returned {}. Use a personal access token with pipeline trigger permission.",
                    repo_res.status()
                ),
            ));
        }
    }

    Ok((
        true,
        format!(
            "Connected as {login}; repo '{full_name}' (id={repo_id}) reachable. Prefer a Woodpecker PAT for dispatch."
        ),
    ))
}

/// Poll one pipeline status snapshot (for background follow-up until terminal).
pub async fn poll_pipeline_once(
    cfg: &Value,
    pipeline_number: &str,
    policy: &CiUrlPolicy,
) -> AppResult<(String, String)> {
    let target = parse_target(cfg, policy)?;
    let http = client()?;
    let (repo_id, full_name) = resolve_repo_id(&http, &target).await?;
    let number: i64 = pipeline_number.parse().map_err(|_| {
        AppError::BadRequest(format!(
            "invalid Woodpecker pipeline number '{pipeline_number}'"
        ))
    })?;
    let poll_url = format!("{}/api/repos/{repo_id}/pipelines/{number}", target.base_url);
    let poll = auth(http.get(&poll_url), &target.token)
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("Woodpecker poll failed: {e}")))?;
    if !poll.status().is_success() {
        let status = poll.status();
        let text = poll.text().await.unwrap_or_default();
        return Err(AppError::BadRequest(format!(
            "Woodpecker poll failed ({status}): {text}"
        )));
    }
    let pipeline: Value = poll
        .json()
        .await
        .map_err(|e| AppError::BadRequest(format!("Woodpecker poll decode failed: {e}")))?;
    let raw = pipeline
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("pending");
    Ok((
        map_status(raw).to_string(),
        pipeline_ui_url(&target.public_base, &full_name, number),
    ))
}

/// Trigger a manual pipeline and briefly poll until it leaves `pending` when possible.
pub async fn dispatch(
    cfg: &Value,
    inputs: &Value,
    policy: &CiUrlPolicy,
) -> AppResult<CiDispatchHandle> {
    let target = parse_target(cfg, policy)?;
    let http = client()?;
    let (repo_id, full_name) = resolve_repo_id(&http, &target).await?;

    let mut variables = Map::new();
    if let Some(obj) = inputs.as_object() {
        for (k, v) in obj {
            let val = match v {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            variables.insert(k.clone(), Value::String(val));
        }
    }
    if let Some(defaults) = cfg.get("default_variables").and_then(|v| v.as_object()) {
        for (k, v) in defaults {
            if !variables.contains_key(k) {
                let val = match v {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                variables.insert(k.clone(), Value::String(val));
            }
        }
    }

    let body = json!({
        "branch": target.branch,
        "variables": variables,
    });

    let url = format!("{}/api/repos/{repo_id}/pipelines", target.base_url);
    let res = auth(http.post(&url), &target.token)
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("Woodpecker trigger failed: {e}")))?;
    if !res.status().is_success() {
        let status = res.status();
        let text = res.text().await.unwrap_or_default();
        let hint = if status.as_u16() == 401 || status.as_u16() == 403 {
            " Use a Woodpecker personal access token (PAT) with repo push / pipeline trigger rights — browser session tokens often pass test-connection but fail here."
        } else {
            ""
        };
        return Err(AppError::BadRequest(format!(
            "Woodpecker pipeline trigger failed ({status}): {text}.{hint}"
        )));
    }
    let mut pipeline: Value = res
        .json()
        .await
        .map_err(|e| AppError::BadRequest(format!("Woodpecker trigger decode failed: {e}")))?;

    let number = pipeline_number(&pipeline).ok_or_else(|| {
        AppError::BadRequest("Woodpecker trigger response missing pipeline number".into())
    })?;

    // Follow status briefly (Jenkins-style) so we can store a running link.
    for _ in 0..8 {
        let status = pipeline
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("pending");
        if !matches!(status, "pending" | "created" | "blocked") {
            break;
        }
        sleep(Duration::from_millis(400)).await;
        let poll_url = format!("{}/api/repos/{repo_id}/pipelines/{number}", target.base_url);
        let poll = auth(http.get(&poll_url), &target.token)
            .send()
            .await
            .map_err(|e| AppError::BadRequest(format!("Woodpecker poll failed: {e}")))?;
        if !poll.status().is_success() {
            break;
        }
        if let Ok(next) = poll.json::<Value>().await {
            pipeline = next;
        } else {
            break;
        }
    }

    let raw_status = pipeline
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("pending");
    let mapped = map_status(raw_status);

    Ok(CiDispatchHandle {
        external_id: Some(number.to_string()),
        external_url: Some(pipeline_ui_url(&target.public_base, &full_name, number)),
        status: mapped.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_repo_path_joins_owner_name() {
        assert_eq!(encode_repo_path("acme/demo"), "acme%2Fdemo");
    }

    #[test]
    fn pipeline_ui_url_shape() {
        assert_eq!(
            pipeline_ui_url("https://ci.example.com", "acme/demo", 9),
            "https://ci.example.com/repos/acme/demo/pipeline/9"
        );
    }

    #[test]
    fn map_status_running() {
        assert_eq!(map_status("running"), "running");
        assert_eq!(map_status("success"), "succeeded");
        assert_eq!(map_status("failure"), "failed");
    }
}
