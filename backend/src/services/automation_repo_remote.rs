//! Fetch automation-repo trees/files via GitHub / GitLab REST (no local `git`).

use std::collections::HashMap;
use std::time::Duration;

use base64::Engine;
use serde::Deserialize;
use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::services::automation_repo::{
    language_for_path, IndexedFile, CHUNK_LINES, MAX_FILES, MAX_FILE_BYTES,
};
use crate::services::egress;

#[derive(Debug, Clone)]
pub struct RemoteTreeEntry {
    pub path: String,
    pub blob_sha: String,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub struct RemoteIndex {
    pub commit_sha: String,
    pub entries: Vec<RemoteTreeEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostKind {
    GitHub {
        api_base: String,
        owner: String,
        repo: String,
    },
    GitLab {
        api_base: String,
        project_path: String,
    },
}

/// Parse `https://github.com/owner/repo(.git)?` or GitLab HTTPS remotes.
pub fn parse_remote(provider: &str, remote_url: &str) -> AppResult<HostKind> {
    egress::validate_public_https_url(remote_url)?;
    let url = remote_url.trim().trim_end_matches('/');
    let without_git = url.strip_suffix(".git").unwrap_or(url);
    let provider = provider.trim().to_lowercase();

    let parsed = url::Url::parse(without_git)
        .map_err(|e| AppError::BadRequest(format!("invalid remote URL: {e}")))?;
    let host = parsed.host_str().unwrap_or("").to_lowercase();
    let path = parsed.path().trim_start_matches('/');
    if path.is_empty() {
        return Err(AppError::BadRequest(
            "remote URL missing owner/repo path".into(),
        ));
    }
    let scheme = match parsed.scheme() {
        "http" | "https" => parsed.scheme(),
        other => {
            return Err(AppError::BadRequest(format!(
                "unsupported remote URL scheme '{other}'"
            )))
        }
    };
    let authority = match parsed.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.clone(),
    };

    match provider.as_str() {
        "github" => {
            let mut parts = path.splitn(2, '/');
            let owner = parts.next().unwrap_or("").to_string();
            let repo = parts.next().unwrap_or("").trim_end_matches('/').to_string();
            if owner.is_empty() || repo.is_empty() {
                return Err(AppError::BadRequest(
                    "GitHub remote must look like https://github.com/owner/repo".into(),
                ));
            }
            let api_base = if host == "github.com" || host == "www.github.com" {
                "https://api.github.com".into()
            } else {
                // GitHub Enterprise / lab mock: {scheme}://host[:port]/api/v3
                format!("{scheme}://{authority}/api/v3")
            };
            Ok(HostKind::GitHub {
                api_base,
                owner,
                repo,
            })
        }
        "gitlab" => {
            let api_base = format!("{scheme}://{authority}/api/v4");
            Ok(HostKind::GitLab {
                api_base,
                project_path: path.to_string(),
            })
        }
        _ => Err(AppError::BadRequest(format!(
            "provider '{provider}' does not support REST indexing"
        ))),
    }
}

async fn sleep_backoff(attempt: u32) {
    let ms = 250u64.saturating_mul(1u64 << attempt.min(4));
    tokio::time::sleep(Duration::from_millis(ms)).await;
}

async fn api_get(
    client: &reqwest::Client,
    url: &str,
    token: Option<&str>,
    accept: &str,
) -> AppResult<(reqwest::header::HeaderMap, Vec<u8>)> {
    let mut last_err = None;
    for attempt in 0..4u32 {
        let mut req = client
            .get(url)
            .header("User-Agent", "vulyk-automation-repo")
            .header("Accept", accept);
        if let Some(t) = token.filter(|t| !t.is_empty()) {
            req = req.bearer_auth(t);
        }
        let res = req
            .send()
            .await
            .map_err(|e| AppError::BadRequest(format!("remote request failed: {e}")))?;
        let status = res.status();
        let headers = res.headers().clone();
        let bytes = res
            .bytes()
            .await
            .map_err(|e| AppError::BadRequest(format!("remote body: {e}")))?
            .to_vec();
        if status.as_u16() == 429 || status.as_u16() == 403 {
            let body = String::from_utf8_lossy(&bytes);
            // Build a fake Response-like rate limit message from headers
            let remaining = headers
                .get("x-ratelimit-remaining")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("?");
            if remaining == "0" || status.as_u16() == 429 || body.contains("rate limit") {
                if attempt < 3 {
                    sleep_backoff(attempt).await;
                    last_err = Some(body.to_string());
                    continue;
                }
                let reset = headers
                    .get("x-ratelimit-reset")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("?");
                return Err(AppError::BadRequest(format!(
                    "Git host rate limit exceeded (remaining={remaining}; reset={reset}): {}",
                    body.chars().take(200).collect::<String>()
                )));
            }
        }
        if !status.is_success() {
            let body = String::from_utf8_lossy(&bytes);
            return Err(AppError::BadRequest(format!(
                "remote API {status}: {}",
                body.chars().take(300).collect::<String>()
            )));
        }
        return Ok((headers, bytes));
    }
    Err(AppError::BadRequest(format!(
        "Git host rate limit exceeded after retries: {}",
        last_err.unwrap_or_default()
    )))
}

/// List tree entries (blobs only) under optional subpath for the branch tip.
pub async fn fetch_tree(
    provider: &str,
    remote_url: &str,
    branch: &str,
    subpath: &str,
    token: Option<&str>,
) -> AppResult<RemoteIndex> {
    let host = parse_remote(provider, remote_url)?;
    let client = reqwest::Client::new();
    match host {
        HostKind::GitHub {
            api_base,
            owner,
            repo,
        } => fetch_github_tree(&client, &api_base, &owner, &repo, branch, subpath, token).await,
        HostKind::GitLab {
            api_base,
            project_path,
        } => fetch_gitlab_tree(&client, &api_base, &project_path, branch, subpath, token).await,
    }
}

async fn fetch_github_tree(
    client: &reqwest::Client,
    api_base: &str,
    owner: &str,
    repo: &str,
    branch: &str,
    subpath: &str,
    token: Option<&str>,
) -> AppResult<RemoteIndex> {
    let commit_url = format!("{api_base}/repos/{owner}/{repo}/commits/{branch}");
    let (_, commit_bytes) =
        api_get(client, &commit_url, token, "application/vnd.github+json").await?;
    let commit: Value = serde_json::from_slice(&commit_bytes)
        .map_err(|e| AppError::BadRequest(format!("github commit JSON: {e}")))?;
    let commit_sha = commit
        .get("sha")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("github commit missing sha".into()))?
        .to_string();
    let tree_sha = commit
        .pointer("/commit/tree/sha")
        .and_then(|v| v.as_str())
        .unwrap_or(&commit_sha);

    let tree_url = format!("{api_base}/repos/{owner}/{repo}/git/trees/{tree_sha}?recursive=1");
    let (_, tree_bytes) = api_get(client, &tree_url, token, "application/vnd.github+json").await?;
    let tree: GhTree = serde_json::from_slice(&tree_bytes)
        .map_err(|e| AppError::BadRequest(format!("github tree JSON: {e}")))?;
    if tree.truncated {
        return Err(AppError::BadRequest(
            "GitHub tree is truncated; narrow the subpath or reduce repository size".into(),
        ));
    }

    let prefix = normalize_subpath(subpath);
    let mut entries = Vec::new();
    for e in tree.tree {
        if e.type_ != "blob" {
            continue;
        }
        if !path_in_subpath(&e.path, &prefix) {
            continue;
        }
        let rel = strip_subpath(&e.path, &prefix);
        if language_for_path(&rel).is_none() {
            continue;
        }
        let size = e.size.unwrap_or(0);
        if size == 0 || size > MAX_FILE_BYTES {
            continue;
        }
        entries.push(RemoteTreeEntry {
            path: rel,
            blob_sha: e.sha,
            size,
        });
        if entries.len() >= MAX_FILES {
            break;
        }
    }
    Ok(RemoteIndex {
        commit_sha,
        entries,
    })
}

async fn fetch_gitlab_tree(
    client: &reqwest::Client,
    api_base: &str,
    project_path: &str,
    branch: &str,
    subpath: &str,
    token: Option<&str>,
) -> AppResult<RemoteIndex> {
    let encoded = urlencoding::encode(project_path);
    let commit_url = format!("{api_base}/projects/{encoded}/repository/commits/{branch}");
    let (_, commit_bytes) = api_get(client, &commit_url, token, "application/json").await?;
    let commit: Value = serde_json::from_slice(&commit_bytes)
        .map_err(|e| AppError::BadRequest(format!("gitlab commit JSON: {e}")))?;
    let commit_sha = commit
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("gitlab commit missing id".into()))?
        .to_string();

    let prefix = normalize_subpath(subpath);
    let mut entries = Vec::new();
    let mut page = 1u32;
    loop {
        let tree_url = format!(
            "{api_base}/projects/{encoded}/repository/tree?ref={}&recursive=true&per_page=100&page={page}",
            urlencoding::encode(branch)
        );
        let (headers, tree_bytes) = api_get(client, &tree_url, token, "application/json").await?;
        let page_entries: Vec<GlTreeEntry> = serde_json::from_slice(&tree_bytes)
            .map_err(|e| AppError::BadRequest(format!("gitlab tree JSON: {e}")))?;
        if page_entries.is_empty() {
            break;
        }
        for e in page_entries {
            if e.type_ != "blob" {
                continue;
            }
            if !path_in_subpath(&e.path, &prefix) {
                continue;
            }
            let rel = strip_subpath(&e.path, &prefix);
            if language_for_path(&rel).is_none() {
                continue;
            }
            entries.push(RemoteTreeEntry {
                path: rel,
                blob_sha: e.id,
                size: 0, // filled when fetching
            });
            if entries.len() >= MAX_FILES {
                break;
            }
        }
        if entries.len() >= MAX_FILES {
            break;
        }
        let next = headers
            .get("x-next-page")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if next.is_empty() {
            break;
        }
        page += 1;
        if page > 50 {
            break;
        }
    }
    Ok(RemoteIndex {
        commit_sha,
        entries,
    })
}

/// Fetch blob contents for entries whose `blob_sha` is not in `known_hashes` (path → hash).
pub async fn fetch_changed_files(
    provider: &str,
    remote_url: &str,
    _branch: &str,
    token: Option<&str>,
    index: &RemoteIndex,
    known_hashes: &HashMap<String, String>,
) -> AppResult<Vec<IndexedFile>> {
    let host = parse_remote(provider, remote_url)?;
    let client = reqwest::Client::new();
    let mut out = Vec::new();
    for entry in &index.entries {
        if known_hashes
            .get(&entry.path)
            .is_some_and(|h| h == &entry.blob_sha)
        {
            continue;
        }
        let content = match &host {
            HostKind::GitHub {
                api_base,
                owner,
                repo,
            } => fetch_github_blob(&client, api_base, owner, repo, &entry.blob_sha, token).await?,
            HostKind::GitLab {
                api_base,
                project_path,
            } => fetch_gitlab_blob(&client, api_base, project_path, &entry.blob_sha, token).await?,
        };
        if content.len() as u64 > MAX_FILE_BYTES || content.is_empty() {
            continue;
        }
        let Some(lang) = language_for_path(&entry.path) else {
            continue;
        };
        let chunks = chunk_text(&content);
        out.push(IndexedFile {
            path: entry.path.clone(),
            language: lang.to_string(),
            size_bytes: content.len() as i32,
            content_hash: entry.blob_sha.clone(),
            chunks,
        });
    }
    Ok(out)
}

async fn fetch_github_blob(
    client: &reqwest::Client,
    api_base: &str,
    owner: &str,
    repo: &str,
    sha: &str,
    token: Option<&str>,
) -> AppResult<String> {
    let url = format!("{api_base}/repos/{owner}/{repo}/git/blobs/{sha}");
    let (_, bytes) = api_get(client, &url, token, "application/vnd.github+json").await?;
    let blob: GhBlob = serde_json::from_slice(&bytes)
        .map_err(|e| AppError::BadRequest(format!("github blob JSON: {e}")))?;
    if blob.encoding == "base64" {
        let cleaned: String = blob
            .content
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(cleaned)
            .map_err(|e| AppError::BadRequest(format!("github blob base64: {e}")))?;
        String::from_utf8(decoded)
            .map_err(|_| AppError::BadRequest("github blob is not utf-8".into()))
    } else {
        Ok(blob.content)
    }
}

async fn fetch_gitlab_blob(
    client: &reqwest::Client,
    api_base: &str,
    project_path: &str,
    blob_sha: &str,
    token: Option<&str>,
) -> AppResult<String> {
    let encoded = urlencoding::encode(project_path);
    let url = format!("{api_base}/projects/{encoded}/repository/blobs/{blob_sha}/raw");
    let (_, bytes) = api_get(client, &url, token, "application/octet-stream").await?;
    String::from_utf8(bytes)
        .map_err(|_| AppError::BadRequest(format!("gitlab blob {blob_sha} is not utf-8 text")))
}

/// Test connectivity without indexing (branch tip reachable).
pub async fn test_remote(
    provider: &str,
    remote_url: &str,
    branch: &str,
    token: Option<&str>,
) -> AppResult<(bool, String, Option<String>)> {
    match fetch_tree(provider, remote_url, branch, "", token).await {
        Ok(idx) => Ok((
            true,
            format!(
                "Reachable via {} API; branch '{branch}' @ {}",
                provider,
                &idx.commit_sha[..idx.commit_sha.len().min(8)]
            ),
            Some(idx.commit_sha),
        )),
        Err(e) => Ok((false, egress::redact_url_credentials(&e.to_string()), None)),
    }
}

fn normalize_subpath(subpath: &str) -> String {
    subpath
        .trim()
        .trim_start_matches('/')
        .trim_end_matches('/')
        .to_string()
}

fn path_in_subpath(path: &str, prefix: &str) -> bool {
    if prefix.is_empty() {
        return true;
    }
    path == prefix || path.starts_with(&format!("{prefix}/"))
}

fn strip_subpath(path: &str, prefix: &str) -> String {
    if prefix.is_empty() {
        return path.to_string();
    }
    path.strip_prefix(&format!("{prefix}/"))
        .unwrap_or(path)
        .to_string()
}

fn chunk_text(content: &str) -> Vec<String> {
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return vec![];
    }
    let mut chunks = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let end = (i + CHUNK_LINES).min(lines.len());
        chunks.push(lines[i..end].join("\n"));
        i = end;
    }
    chunks
}

#[derive(Debug, Deserialize)]
struct GhTree {
    #[allow(dead_code)]
    sha: String,
    tree: Vec<GhTreeEntry>,
    #[serde(default)]
    truncated: bool,
}

#[derive(Debug, Deserialize)]
struct GhTreeEntry {
    path: String,
    #[serde(rename = "type")]
    type_: String,
    sha: String,
    size: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct GhBlob {
    content: String,
    encoding: String,
}

#[derive(Debug, Deserialize)]
struct GlTreeEntry {
    id: String,
    path: String,
    #[serde(rename = "type")]
    type_: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_github_url() {
        let h = parse_remote("github", "https://github.com/acme/demo.git").unwrap();
        match h {
            HostKind::GitHub {
                owner,
                repo,
                api_base,
                ..
            } => {
                assert_eq!(owner, "acme");
                assert_eq!(repo, "demo");
                assert_eq!(api_base, "https://api.github.com");
            }
            _ => panic!("expected github"),
        }
    }

    #[test]
    fn parses_gitlab_url() {
        let h = parse_remote("gitlab", "https://gitlab.com/group/sub/repo").unwrap();
        match h {
            HostKind::GitLab { project_path, .. } => {
                assert_eq!(project_path, "group/sub/repo");
            }
            _ => panic!("expected gitlab"),
        }
    }

    #[test]
    fn subpath_filter() {
        assert!(path_in_subpath("e2e/a.ts", "e2e"));
        assert!(!path_in_subpath("src/a.ts", "e2e"));
        assert_eq!(strip_subpath("e2e/a.ts", "e2e"), "a.ts");
    }
}
