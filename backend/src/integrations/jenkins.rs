//! Jenkins: crumb-aware `build` / `buildWithParameters` + queue follow.

use std::time::Duration;

use serde_json::{json, Value};

use super::ci::{optional_str, require_str, CiDispatchHandle};
use crate::error::{AppError, AppResult};
use crate::services::egress;

#[derive(Debug, Clone)]
pub struct JenkinsTarget {
    pub base_url: String,
    pub job_path: String,
    pub username: String,
    pub token: String,
    pub use_crumb: bool,
}

#[derive(Debug, Clone)]
struct Crumb {
    field: String,
    value: String,
}

/// Normalize operator input into a Jenkins job API path segment list.
///
/// Accepts:
/// - `folder/job-name`
/// - `/job/folder/job/job-name`
/// - full job URL `https://jenkins.example/job/folder/job/job-name/`
pub fn normalize_job_path(raw: &str) -> AppResult<String> {
    let trimmed = raw.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err(AppError::BadRequest(
            "jenkins config requires 'job_path' or 'job_url'".into(),
        ));
    }

    // Full URL → extract path after host.
    let path = if trimmed.contains("://") {
        let url = trimmed
            .parse::<url::Url>()
            .map_err(|e| AppError::BadRequest(format!("invalid jenkins job_url: {e}")))?;
        url.path().trim_end_matches('/').to_string()
    } else if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        // Plain "folder/name" → "/job/folder/job/name"
        let segments: Vec<&str> = trimmed
            .split('/')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        if segments.is_empty() {
            return Err(AppError::BadRequest(
                "jenkins job_path has no segments".into(),
            ));
        }
        let mut out = String::new();
        for seg in segments {
            out.push_str("/job/");
            out.push_str(seg);
        }
        return Ok(out);
    };

    // Ensure path uses /job/ segments (operators sometimes paste /view/.../job/...).
    // Keep as-is when already /job/...; otherwise wrap remaining segments.
    let path = path.trim_end_matches('/');
    if path.is_empty() || path == "/" {
        return Err(AppError::BadRequest(
            "jenkins job_path has no segments".into(),
        ));
    }
    if path.contains("/job/") || path.starts_with("/job/") {
        return Ok(path.to_string());
    }
    let segments: Vec<&str> = path
        .split('/')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    let mut out = String::new();
    for seg in segments {
        out.push_str("/job/");
        out.push_str(seg);
    }
    Ok(out)
}

pub fn parse_target(cfg: &Value) -> AppResult<JenkinsTarget> {
    let base_url = require_str(cfg, "base_url")?
        .trim_end_matches('/')
        .to_string();
    let job_raw = optional_str(cfg, "job_path")
        .or_else(|| optional_str(cfg, "job_url"))
        .ok_or_else(|| {
            AppError::BadRequest("jenkins config requires 'job_path' or 'job_url'".into())
        })?;
    let job_path = normalize_job_path(&job_raw)?;
    let username = require_str(cfg, "username")?;
    let token = require_str(cfg, "token")?;
    let use_crumb = cfg
        .get("use_crumb")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    // SSRF: only after field shape is valid so unit tests can assert missing keys.
    egress::validate_public_https_url(&base_url)?;

    Ok(JenkinsTarget {
        base_url,
        job_path,
        username,
        token,
        use_crumb,
    })
}

fn client() -> AppResult<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| AppError::BadRequest(format!("HTTP client error: {e}")))
}

async fn fetch_crumb(http: &reqwest::Client, target: &JenkinsTarget) -> AppResult<Option<Crumb>> {
    if !target.use_crumb {
        return Ok(None);
    }
    let url = format!("{}/crumbIssuer/api/json", target.base_url);
    let res = http
        .get(&url)
        .basic_auth(&target.username, Some(&target.token))
        .header("User-Agent", "vulyk-ci")
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("Jenkins crumb request failed: {e}")))?;

    let status = res.status();
    if status.as_u16() == 404 {
        // CSRF protection disabled on this controller.
        return Ok(None);
    }
    if !status.is_success() {
        let text = res.text().await.unwrap_or_default();
        return Err(AppError::BadRequest(format!(
            "Jenkins crumb issuer failed ({status}): {text}"
        )));
    }
    let body: Value = res
        .json()
        .await
        .map_err(|e| AppError::BadRequest(format!("Jenkins crumb decode failed: {e}")))?;
    let value = body
        .get("crumb")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::BadRequest("Jenkins crumb response missing 'crumb'".into()))?
        .to_string();
    let field = body
        .get("crumbRequestField")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("Jenkins-Crumb")
        .to_string();
    Ok(Some(Crumb { field, value }))
}

fn apply_crumb(mut req: reqwest::RequestBuilder, crumb: &Option<Crumb>) -> reqwest::RequestBuilder {
    if let Some(c) = crumb {
        req = req.header(&c.field, &c.value);
    }
    req
}

/// Probe Jenkins credentials + job existence (crumb → whoAmI → job API).
pub async fn test_connection(cfg: &Value) -> AppResult<(bool, String)> {
    let target = parse_target(cfg)?;
    let http = client()?;
    let crumb = fetch_crumb(&http, &target).await?;

    let who_url = format!("{}/whoAmI/api/json", target.base_url);
    let who_req = apply_crumb(
        http.get(&who_url)
            .basic_auth(&target.username, Some(&target.token))
            .header("User-Agent", "vulyk-ci"),
        &crumb,
    );
    let who = who_req
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("Jenkins whoAmI failed: {e}")))?;
    if !who.status().is_success() {
        let status = who.status();
        let text = who.text().await.unwrap_or_default();
        return Err(AppError::BadRequest(format!(
            "Jenkins whoAmI failed ({status}): {text}"
        )));
    }
    let who_body: Value = who.json().await.unwrap_or_else(|_| json!({}));
    let who_name = who_body
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or(&target.username);

    let job_url = format!(
        "{}{}/api/json?tree=name,url,buildable",
        target.base_url, target.job_path
    );
    let job_req = apply_crumb(
        http.get(&job_url)
            .basic_auth(&target.username, Some(&target.token))
            .header("User-Agent", "vulyk-ci"),
        &crumb,
    );
    let job = job_req
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("Jenkins job API failed: {e}")))?;
    if !job.status().is_success() {
        let status = job.status();
        let text = job.text().await.unwrap_or_default();
        return Err(AppError::BadRequest(format!(
            "Jenkins job not reachable ({status}): {text}"
        )));
    }
    let job_body: Value = job.json().await.unwrap_or_else(|_| json!({}));
    let job_name = job_body
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("job");
    let crumb_note = if crumb.is_some() {
        "crumb ok"
    } else if target.use_crumb {
        "crumb unused (CSRF off)"
    } else {
        "crumb disabled"
    };
    Ok((
        true,
        format!("Connected as {who_name}; job '{job_name}' reachable ({crumb_note})"),
    ))
}

/// Trigger a Jenkins build and follow the queue item to a concrete build number when possible.
pub async fn dispatch(cfg: &Value, inputs: &Value) -> AppResult<CiDispatchHandle> {
    let target = parse_target(cfg)?;
    let http = client()?;
    let crumb = fetch_crumb(&http, &target).await?;

    let mut form: Vec<(String, String)> = Vec::new();
    if let Some(obj) = inputs.as_object() {
        for (k, v) in obj {
            let val = match v {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            form.push((k.clone(), val));
        }
    }
    // Config-level default parameters (same idea as GitLab default_variables).
    if let Some(defaults) = cfg.get("default_parameters").and_then(|v| v.as_object()) {
        for (k, v) in defaults {
            if inputs.get(k).is_none() {
                let val = match v {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                form.push((k.clone(), val));
            }
        }
    }

    let trigger_path = if form.is_empty() {
        "build"
    } else {
        "buildWithParameters"
    };
    let trigger_url = format!("{}{}/{trigger_path}", target.base_url, target.job_path);

    let mut req = apply_crumb(
        http.post(&trigger_url)
            .basic_auth(&target.username, Some(&target.token))
            .header("User-Agent", "vulyk-ci"),
        &crumb,
    );
    if !form.is_empty() {
        req = req.form(&form);
    }

    let res = req
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("Jenkins trigger failed: {e}")))?;

    let status = res.status();
    // Jenkins returns 201 Created with Location: queue item, or 302 redirect.
    if !(status.is_success() || status.as_u16() == 201 || status.as_u16() == 302) {
        let text = res.text().await.unwrap_or_default();
        return Err(AppError::BadRequest(format!(
            "Jenkins {trigger_path} failed ({status}): {text}"
        )));
    }

    let location = res
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let job_html = format!("{}{}/", target.base_url, target.job_path);

    let Some(loc) = location else {
        return Ok(CiDispatchHandle {
            external_id: None,
            external_url: Some(job_html),
            status: "dispatched".into(),
        });
    };

    let queue_url = absolute_url(&target.base_url, &loc);
    match follow_queue(&http, &target, &crumb, &queue_url).await {
        Ok(Some((build_number, build_url))) => Ok(CiDispatchHandle {
            external_id: Some(build_number.to_string()),
            external_url: Some(build_url),
            status: "running".into(),
        }),
        Ok(None) => Ok(CiDispatchHandle {
            external_id: queue_item_id(&queue_url),
            external_url: Some(job_html),
            status: "dispatched".into(),
        }),
        Err(_) => Ok(CiDispatchHandle {
            external_id: queue_item_id(&queue_url),
            external_url: Some(job_html),
            status: "dispatched".into(),
        }),
    }
}

fn absolute_url(base: &str, loc: &str) -> String {
    if loc.starts_with("http://") || loc.starts_with("https://") {
        loc.trim_end_matches('/').to_string()
    } else if loc.starts_with('/') {
        format!(
            "{}{}",
            base.trim_end_matches('/'),
            loc.trim_end_matches('/')
        )
    } else {
        format!("{}/{loc}", base.trim_end_matches('/'))
    }
}

fn queue_item_id(queue_url: &str) -> Option<String> {
    // .../queue/item/123 or .../queue/item/123/
    let marker = "/queue/item/";
    let idx = queue_url.find(marker)?;
    let rest = &queue_url[idx + marker.len()..];
    let id = rest.split('/').next().unwrap_or("").trim();
    if id.is_empty() {
        None
    } else {
        Some(format!("queue:{id}"))
    }
}

async fn follow_queue(
    http: &reqwest::Client,
    target: &JenkinsTarget,
    crumb: &Option<Crumb>,
    queue_url: &str,
) -> AppResult<Option<(i64, String)>> {
    let api = if queue_url.contains("/api/json") {
        queue_url.to_string()
    } else {
        format!("{}/api/json", queue_url.trim_end_matches('/'))
    };

    // Short poll: queue items usually resolve quickly; give up after a few tries.
    for _ in 0..8 {
        let req = apply_crumb(
            http.get(&api)
                .basic_auth(&target.username, Some(&target.token))
                .header("User-Agent", "vulyk-ci"),
            crumb,
        );
        let res = req
            .send()
            .await
            .map_err(|e| AppError::BadRequest(format!("Jenkins queue poll failed: {e}")))?;
        if !res.status().is_success() {
            return Ok(None);
        }
        let body: Value = res
            .json()
            .await
            .map_err(|e| AppError::BadRequest(format!("Jenkins queue decode failed: {e}")))?;

        if let Some(exec) = body.get("executable") {
            let number = exec.get("number").and_then(|v| v.as_i64()).or_else(|| {
                exec.get("number")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as i64)
            });
            let url = exec
                .get("url")
                .and_then(|v| v.as_str())
                .map(|s| s.trim_end_matches('/').to_string())
                .unwrap_or_else(|| {
                    format!(
                        "{}{}/{}/",
                        target.base_url,
                        target.job_path,
                        number.unwrap_or(0)
                    )
                });
            if let Some(n) = number {
                return Ok(Some((n, format!("{}/", url.trim_end_matches('/')))));
            }
        }
        if body.get("cancelled").and_then(|v| v.as_bool()) == Some(true) {
            return Err(AppError::BadRequest(
                "Jenkins queue item was cancelled before a build started".into(),
            ));
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    Ok(None)
}

/// Build the trigger URL used for a given config + whether parameters are present.
pub fn trigger_url(base_url: &str, job_path: &str, with_params: bool) -> String {
    let path = if with_params {
        "buildWithParameters"
    } else {
        "build"
    };
    format!("{}{}/{path}", base_url.trim_end_matches('/'), job_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn normalize_plain_path() {
        assert_eq!(
            normalize_job_path("folder/my-job").unwrap(),
            "/job/folder/job/my-job"
        );
    }

    #[test]
    fn normalize_already_job_path() {
        assert_eq!(
            normalize_job_path("/job/folder/job/my-job/").unwrap(),
            "/job/folder/job/my-job"
        );
    }

    #[test]
    fn normalize_full_url() {
        assert_eq!(
            normalize_job_path("https://jenkins.example/job/ci/job/e2e/").unwrap(),
            "/job/ci/job/e2e"
        );
    }

    #[test]
    fn trigger_url_with_and_without_params() {
        assert_eq!(
            trigger_url("https://jenkins.example", "/job/demo", true),
            "https://jenkins.example/job/demo/buildWithParameters"
        );
        assert_eq!(
            trigger_url("https://jenkins.example/", "/job/demo", false),
            "https://jenkins.example/job/demo/build"
        );
    }

    #[test]
    fn parse_target_requires_fields() {
        let err = parse_target(&json!({
            "base_url": "https://jenkins.example",
            "job_path": "demo"
        }))
        .unwrap_err();
        assert!(err.to_string().contains("username"));
    }

    #[test]
    fn queue_item_id_extracts() {
        assert_eq!(
            queue_item_id("https://jenkins.example/queue/item/42/"),
            Some("queue:42".into())
        );
    }
}
