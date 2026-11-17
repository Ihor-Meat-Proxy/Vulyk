//! Forgejo / Gitea Actions: `workflow_dispatch` via `/api/v1` + public URL links.
//!
//! JSON body matches GitHub Actions (`ref` + `inputs`). Auth and API root differ:
//! `Authorization: token …` and `{base}/api/v1/repos/.../actions/workflows/.../dispatches`.

use serde_json::{json, Value};

use super::ci::{optional_str, require_str, CiDispatchHandle};
use crate::error::{AppError, AppResult};
use crate::services::egress::{self, CiUrlPolicy};

/// Shared dispatch for `forgejo` and `gitea` CI configs.
pub async fn dispatch(
    cfg: &Value,
    inputs: &Value,
    policy: &CiUrlPolicy,
) -> AppResult<CiDispatchHandle> {
    let token = require_str(cfg, "token")?;
    let owner = require_str(cfg, "owner")?;
    let repo = require_str(cfg, "repo")?;
    let workflow_id = require_str(cfg, "workflow_id")?;
    let git_ref = optional_str(cfg, "ref").unwrap_or_else(|| "main".into());
    let base_url = require_str(cfg, "base_url")?
        .trim_end_matches('/')
        .to_string();
    egress::validate_ci_url(&base_url, policy)?;

    let public_base = optional_str(cfg, "public_base_url")
        .map(|s| s.trim_end_matches('/').to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| base_url.clone());
    if public_base != base_url {
        egress::validate_ci_url(&public_base, policy)?;
    }

    let body = json!({
        "ref": git_ref,
        "inputs": inputs.as_object().cloned().unwrap_or_default(),
    });

    let url = format!(
        "{base_url}/api/v1/repos/{owner}/{repo}/actions/workflows/{workflow_id}/dispatches"
    );
    let client = reqwest::Client::new();
    let res = client
        .post(&url)
        .header("Accept", "application/json")
        .header("User-Agent", "vulyk-ci")
        // Forgejo/Gitea PATs use `token`; Bearer is accepted by some builds.
        .header("Authorization", format!("token {token}"))
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("Forgejo/Gitea request failed: {e}")))?;

    if !(res.status().is_success() || res.status().as_u16() == 204) {
        let status = res.status();
        let text = res.text().await.unwrap_or_default();
        return Err(AppError::BadRequest(format!(
            "Forgejo/Gitea workflow_dispatch failed ({status}): {text}"
        )));
    }

    // No run id in the 204 response; UI links to the workflow Actions page.
    // Callers correlate concrete runs via webhook (GitHub-compatible workflow_run).
    Ok(CiDispatchHandle {
        external_id: None,
        external_url: Some(format!(
            "{public_base}/{owner}/{repo}/actions?workflow={workflow_id}&actor=0&status=0"
        )),
        status: "dispatched".into(),
    })
}

/// Env / config public base for rewriting inbound pipeline URLs.
pub fn public_base_env_for_provider(provider: &str) -> Option<String> {
    let key = match provider.trim().to_ascii_lowercase().as_str() {
        "forgejo" => "VULYK_FORGEJO_PUBLIC_BASE_URL",
        "gitea" => "VULYK_GITEA_PUBLIC_BASE_URL",
        _ => return None,
    };
    std::env::var(key)
        .ok()
        .map(|s| s.trim().trim_end_matches('/').to_string())
        .filter(|s| !s.is_empty())
}

/// Rewrite `pipeline_url` when a public base is configured for the provider.
pub fn maybe_rewrite_pipeline_url(
    provider: Option<&str>,
    pipeline_url: Option<String>,
) -> Option<String> {
    let url = pipeline_url?;
    let provider = provider?;
    let Some(base) = public_base_env_for_provider(provider) else {
        return Some(url);
    };
    Some(rewrite_public_url(&url, &base))
}

/// Replace the origin of `url` with `public_base` when both parse as http(s) URLs.
///
/// Used so inbound reporters that see an internal Docker host
/// (`http://vulyk-forgejo:3000/...`) store a clickable public pipeline link.
pub fn rewrite_public_url(url: &str, public_base: &str) -> String {
    let url = url.trim();
    let public_base = public_base.trim().trim_end_matches('/');
    if url.is_empty() || public_base.is_empty() {
        return url.to_string();
    }
    let Some(rest) = strip_http_origin(url) else {
        return url.to_string();
    };
    format!("{public_base}{rest}")
}

fn strip_http_origin(url: &str) -> Option<&str> {
    let lower = url.to_ascii_lowercase();
    let scheme_len = if lower.starts_with("https://") {
        8
    } else if lower.starts_with("http://") {
        7
    } else {
        return None;
    };
    let after = &url[scheme_len..];
    let path_start = after.find('/').unwrap_or(after.len());
    Some(&after[path_start..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrite_swaps_internal_origin() {
        let out = rewrite_public_url(
            "http://vulyk-forgejo:3000/acme/demo/actions/runs/9",
            "https://git.example.com",
        );
        assert_eq!(out, "https://git.example.com/acme/demo/actions/runs/9");
    }

    #[test]
    fn rewrite_keeps_non_http() {
        assert_eq!(rewrite_public_url("not-a-url", "https://x"), "not-a-url");
    }

    #[test]
    fn maybe_rewrite_uses_forgejo_env() {
        const KEY: &str = "VULYK_FORGEJO_PUBLIC_BASE_URL";
        let prev = std::env::var(KEY).ok();
        std::env::set_var(KEY, "https://git.example.com");
        let out = maybe_rewrite_pipeline_url(
            Some("forgejo"),
            Some("http://vulyk-forgejo:3000/acme/demo/actions/runs/9".into()),
        );
        assert_eq!(
            out.as_deref(),
            Some("https://git.example.com/acme/demo/actions/runs/9")
        );
        match prev {
            Some(v) => std::env::set_var(KEY, v),
            None => std::env::remove_var(KEY),
        }
    }
}
