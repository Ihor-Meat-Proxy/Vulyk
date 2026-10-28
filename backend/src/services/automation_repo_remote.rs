//! Fetch automation-repo trees/files via GitHub / GitLab REST (no local `git`).
//!
//! Indexing downloads a **single archive** (GitHub zipball / GitLab archive.zip)
//! and extracts only the configured subpath — one or two API calls instead of
//! per-file blob fetches (which burn the unauthenticated 60 req/h quota).

use std::collections::HashMap;
use std::io::{Cursor, Read};
use std::time::Duration;

use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use zip::ZipArchive;

use crate::error::{AppError, AppResult};
use crate::services::automation_repo::{
    language_for_path, IndexedFile, CHUNK_LINES, MAX_FILES, MAX_FILE_BYTES,
};
use crate::services::egress;

/// Soft cap on downloaded archive size (bytes).
pub const MAX_ARCHIVE_BYTES: u64 = 50 * 1024 * 1024;

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

#[derive(Debug, Clone, Serialize)]
pub struct RemoteBranch {
    pub name: String,
    pub commit_sha: Option<String>,
    /// True when this name was not in the previously seen set for the repo.
    pub is_new: bool,
}

/// Result of a cheap branch-head probe (ETag / If-None-Match aware).
#[derive(Debug, Clone)]
pub enum HeadCheck {
    /// Unchanged (HTTP 304); does not consume GitHub rate limit when ETag matched.
    NotModified { etag: Option<String> },
    /// Tip SHA available (and optional new ETag to store).
    Modified {
        commit_sha: String,
        etag: Option<String>,
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

fn rate_limit_reset(headers: &reqwest::header::HeaderMap) -> Option<DateTime<Utc>> {
    headers
        .get("x-ratelimit-reset")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<i64>().ok())
        .and_then(|unix| Utc.timestamp_opt(unix, 0).single())
}

fn rate_limit_error(headers: &reqwest::header::HeaderMap, body: &str) -> AppError {
    let remaining = headers
        .get("x-ratelimit-remaining")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("0");
    let reset_at = rate_limit_reset(headers);
    let reset_hint = reset_at
        .map(|t| t.to_rfc3339())
        .unwrap_or_else(|| "?".into());
    AppError::GitHostRateLimit {
        message: format!(
            "Git host rate limit exceeded (remaining={remaining}; reset={reset_hint}). \
             Add a read-only access token to raise the quota. {}",
            body.chars().take(160).collect::<String>()
        ),
        reset_at,
    }
}

struct ApiGetOpts<'a> {
    token: Option<&'a str>,
    accept: &'a str,
    etag: Option<&'a str>,
}

async fn api_get(
    client: &reqwest::Client,
    url: &str,
    opts: ApiGetOpts<'_>,
) -> AppResult<(reqwest::StatusCode, reqwest::header::HeaderMap, Vec<u8>)> {
    let mut last_err = None;
    for attempt in 0..4u32 {
        let mut req = client
            .get(url)
            .header("User-Agent", "vulyk-automation-repo")
            .header("Accept", opts.accept);
        if let Some(t) = opts.token.filter(|t| !t.is_empty()) {
            req = req.bearer_auth(t);
        }
        if let Some(etag) = opts.etag.filter(|e| !e.is_empty()) {
            req = req.header("If-None-Match", etag);
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

        if status.as_u16() == 304 {
            return Ok((status, headers, bytes));
        }

        if status.as_u16() == 429 || status.as_u16() == 403 {
            let body = String::from_utf8_lossy(&bytes);
            let remaining = headers
                .get("x-ratelimit-remaining")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("?");
            if remaining == "0" || status.as_u16() == 429 || body.contains("rate limit") {
                if attempt < 3 {
                    sleep_backoff(attempt).await;
                    last_err = Some(rate_limit_error(&headers, &body));
                    continue;
                }
                return Err(rate_limit_error(&headers, &body));
            }
        }
        if !status.is_success() {
            let body = String::from_utf8_lossy(&bytes);
            return Err(AppError::BadRequest(format!(
                "remote API {status}: {}",
                body.chars().take(300).collect::<String>()
            )));
        }
        return Ok((status, headers, bytes));
    }
    Err(last_err.unwrap_or_else(|| AppError::BadRequest("Git host rate limit exceeded".into())))
}

fn header_etag(headers: &reqwest::header::HeaderMap) -> Option<String> {
    headers
        .get("etag")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

/// Cheap branch-tip probe with optional ETag (304 → NotModified).
pub async fn check_branch_head(
    provider: &str,
    remote_url: &str,
    branch: &str,
    token: Option<&str>,
    etag: Option<&str>,
) -> AppResult<HeadCheck> {
    let host = parse_remote(provider, remote_url)?;
    let client = reqwest::Client::new();
    match host {
        HostKind::GitHub {
            api_base,
            owner,
            repo,
        } => {
            let url = format!("{api_base}/repos/{owner}/{repo}/commits/{branch}");
            let (status, headers, bytes) = api_get(
                &client,
                &url,
                ApiGetOpts {
                    token,
                    accept: "application/vnd.github+json",
                    etag,
                },
            )
            .await?;
            let new_etag = header_etag(&headers);
            if status.as_u16() == 304 {
                return Ok(HeadCheck::NotModified {
                    etag: new_etag.or_else(|| etag.map(|s| s.to_string())),
                });
            }
            let commit: Value = serde_json::from_slice(&bytes)
                .map_err(|e| AppError::BadRequest(format!("github commit JSON: {e}")))?;
            let commit_sha = commit
                .get("sha")
                .and_then(|v| v.as_str())
                .ok_or_else(|| AppError::BadRequest("github commit missing sha".into()))?
                .to_string();
            Ok(HeadCheck::Modified {
                commit_sha,
                etag: new_etag,
            })
        }
        HostKind::GitLab {
            api_base,
            project_path,
        } => {
            let encoded = urlencoding::encode(&project_path);
            let url = format!("{api_base}/projects/{encoded}/repository/commits/{branch}");
            let (status, headers, bytes) = api_get(
                &client,
                &url,
                ApiGetOpts {
                    token,
                    accept: "application/json",
                    etag,
                },
            )
            .await?;
            let new_etag = header_etag(&headers);
            if status.as_u16() == 304 {
                return Ok(HeadCheck::NotModified {
                    etag: new_etag.or_else(|| etag.map(|s| s.to_string())),
                });
            }
            let commit: Value = serde_json::from_slice(&bytes)
                .map_err(|e| AppError::BadRequest(format!("gitlab commit JSON: {e}")))?;
            let commit_sha = commit
                .get("id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| AppError::BadRequest("gitlab commit missing id".into()))?
                .to_string();
            Ok(HeadCheck::Modified {
                commit_sha,
                etag: new_etag,
            })
        }
    }
}

/// List remote branches; `seen` marks previously known names (`is_new`).
pub async fn list_branches(
    provider: &str,
    remote_url: &str,
    token: Option<&str>,
    seen: &[String],
) -> AppResult<Vec<RemoteBranch>> {
    let host = parse_remote(provider, remote_url)?;
    let client = reqwest::Client::new();
    let seen_set: std::collections::HashSet<&str> = seen.iter().map(String::as_str).collect();
    let mut out = Vec::new();
    match host {
        HostKind::GitHub {
            api_base,
            owner,
            repo,
        } => {
            let mut page = 1u32;
            loop {
                let url =
                    format!("{api_base}/repos/{owner}/{repo}/branches?per_page=100&page={page}");
                let (_, _, bytes) = api_get(
                    &client,
                    &url,
                    ApiGetOpts {
                        token,
                        accept: "application/vnd.github+json",
                        etag: None,
                    },
                )
                .await?;
                let page_entries: Vec<GhBranch> = serde_json::from_slice(&bytes)
                    .map_err(|e| AppError::BadRequest(format!("github branches JSON: {e}")))?;
                if page_entries.is_empty() {
                    break;
                }
                for b in page_entries {
                    let is_new = !seen_set.is_empty() && !seen_set.contains(b.name.as_str());
                    out.push(RemoteBranch {
                        name: b.name,
                        commit_sha: b.commit.and_then(|c| c.sha),
                        is_new,
                    });
                }
                page += 1;
                if page > 20 {
                    break;
                }
            }
        }
        HostKind::GitLab {
            api_base,
            project_path,
        } => {
            let encoded = urlencoding::encode(&project_path);
            let mut page = 1u32;
            loop {
                let url = format!(
                    "{api_base}/projects/{encoded}/repository/branches?per_page=100&page={page}"
                );
                let (headers, bytes) = {
                    let (status, headers, bytes) = api_get(
                        &client,
                        &url,
                        ApiGetOpts {
                            token,
                            accept: "application/json",
                            etag: None,
                        },
                    )
                    .await?;
                    let _ = status;
                    (headers, bytes)
                };
                let page_entries: Vec<GlBranch> = serde_json::from_slice(&bytes)
                    .map_err(|e| AppError::BadRequest(format!("gitlab branches JSON: {e}")))?;
                if page_entries.is_empty() {
                    break;
                }
                for b in page_entries {
                    let is_new = !seen_set.is_empty() && !seen_set.contains(b.name.as_str());
                    out.push(RemoteBranch {
                        name: b.name,
                        commit_sha: b.commit.and_then(|c| c.id),
                        is_new,
                    });
                }
                let next = headers
                    .get("x-next-page")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("");
                if next.is_empty() {
                    break;
                }
                page += 1;
                if page > 20 {
                    break;
                }
            }
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// Download a single archive and extract indexable files under `subpath`.
pub async fn index_from_archive(
    provider: &str,
    remote_url: &str,
    branch: &str,
    subpath: &str,
    token: Option<&str>,
    known_hashes: &HashMap<String, String>,
) -> AppResult<(String, Vec<IndexedFile>, HashMap<String, String>)> {
    let host = parse_remote(provider, remote_url)?;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .map_err(|e| AppError::Internal(format!("http client: {e}")))?;

    // Resolve tip SHA first (also used when archive filename lacks it).
    let head = check_branch_head(provider, remote_url, branch, token, None).await?;
    let commit_sha = match head {
        HeadCheck::Modified { commit_sha, .. } => commit_sha,
        HeadCheck::NotModified { .. } => {
            return Err(AppError::Internal(
                "unexpected 304 without etag on archive index".into(),
            ));
        }
    };

    let (archive_url, accept) = match &host {
        HostKind::GitHub {
            api_base,
            owner,
            repo,
        } => (
            format!("{api_base}/repos/{owner}/{repo}/zipball/{branch}"),
            "application/vnd.github+json",
        ),
        HostKind::GitLab {
            api_base,
            project_path,
        } => {
            let encoded = urlencoding::encode(project_path);
            (
                format!(
                    "{api_base}/projects/{encoded}/repository/archive.zip?sha={}",
                    urlencoding::encode(branch)
                ),
                "application/octet-stream",
            )
        }
    };

    let mut last_err = None;
    let mut archive_bytes = None;
    for attempt in 0..4u32 {
        let mut req = client
            .get(&archive_url)
            .header("User-Agent", "vulyk-automation-repo")
            .header("Accept", accept);
        if let Some(t) = token.filter(|t| !t.is_empty()) {
            req = req.bearer_auth(t);
        }
        let res = req
            .send()
            .await
            .map_err(|e| AppError::BadRequest(format!("archive download failed: {e}")))?;
        let status = res.status();
        let headers = res.headers().clone();
        if status.as_u16() == 429 || status.as_u16() == 403 {
            let body = res.text().await.unwrap_or_default();
            let remaining = headers
                .get("x-ratelimit-remaining")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("?");
            if remaining == "0" || status.as_u16() == 429 || body.contains("rate limit") {
                if attempt < 3 {
                    sleep_backoff(attempt).await;
                    last_err = Some(rate_limit_error(&headers, &body));
                    continue;
                }
                return Err(rate_limit_error(&headers, &body));
            }
            return Err(AppError::BadRequest(format!(
                "archive download {status}: {}",
                body.chars().take(300).collect::<String>()
            )));
        }
        if !status.is_success() {
            let body = res.text().await.unwrap_or_default();
            return Err(AppError::BadRequest(format!(
                "archive download {status}: {}",
                body.chars().take(300).collect::<String>()
            )));
        }
        let len = res.content_length().unwrap_or(0);
        if len > MAX_ARCHIVE_BYTES {
            return Err(AppError::BadRequest(format!(
                "repository archive too large ({len} bytes; max {MAX_ARCHIVE_BYTES})"
            )));
        }
        let bytes = res
            .bytes()
            .await
            .map_err(|e| AppError::BadRequest(format!("archive body: {e}")))?;
        if bytes.len() as u64 > MAX_ARCHIVE_BYTES {
            return Err(AppError::BadRequest(format!(
                "repository archive too large ({} bytes; max {MAX_ARCHIVE_BYTES})",
                bytes.len()
            )));
        }
        archive_bytes = Some(bytes.to_vec());
        break;
    }
    let archive_bytes = archive_bytes.ok_or_else(|| {
        last_err.unwrap_or_else(|| AppError::BadRequest("archive download failed".into()))
    })?;

    let prefix = normalize_subpath(subpath);
    let extracted = extract_zip_files(&archive_bytes, &prefix)?;

    let mut tree_paths = HashMap::new();
    let mut changed = Vec::new();
    for (rel, content) in extracted {
        if content.is_empty() || content.len() as u64 > MAX_FILE_BYTES {
            continue;
        }
        let Some(lang) = language_for_path(&rel) else {
            continue;
        };
        let hash = content_hash(&content);
        tree_paths.insert(rel.clone(), hash.clone());
        if known_hashes.get(&rel).is_some_and(|h| h == &hash) {
            continue;
        }
        let chunks = chunk_text(&content);
        changed.push(IndexedFile {
            path: rel,
            language: lang.to_string(),
            size_bytes: content.len() as i32,
            content_hash: hash,
            chunks,
        });
        if tree_paths.len() >= MAX_FILES {
            break;
        }
    }

    // Cap tree_paths if we hit MAX_FILES mid-way (already capped in loop via break on tree size).
    if tree_paths.len() > MAX_FILES {
        let mut keys: Vec<_> = tree_paths.keys().cloned().collect();
        keys.sort();
        for k in keys.into_iter().skip(MAX_FILES) {
            tree_paths.remove(&k);
        }
        changed.retain(|f| tree_paths.contains_key(&f.path));
    }

    Ok((commit_sha, changed, tree_paths))
}

fn content_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    hex::encode(hasher.finalize())
}

/// Extract text files under subpath from a zip archive; rejects path traversal.
fn extract_zip_files(bytes: &[u8], subpath_prefix: &str) -> AppResult<Vec<(String, String)>> {
    let cursor = Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor)
        .map_err(|e| AppError::BadRequest(format!("invalid zip archive: {e}")))?;

    // Zipball entries look like `{repo}-{sha}/path/to/file`.
    let mut out = Vec::new();
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| AppError::BadRequest(format!("zip entry: {e}")))?;
        if file.is_dir() {
            continue;
        }
        let Ok(raw_name) = file.name().map(|s| s.to_string()) else {
            continue;
        };
        let Some(rel_in_repo) = strip_archive_root(&raw_name) else {
            continue;
        };
        if !is_safe_relative_path(&rel_in_repo) {
            continue;
        }
        if !path_in_subpath(&rel_in_repo, subpath_prefix) {
            continue;
        }
        let rel = strip_subpath(&rel_in_repo, subpath_prefix);
        if !is_safe_relative_path(&rel) {
            continue;
        }
        if language_for_path(&rel).is_none() {
            continue;
        }
        if file.size() > MAX_FILE_BYTES {
            continue;
        }
        let mut buf = Vec::new();
        if file
            .read_to_end(&mut buf)
            .map_err(|e| AppError::BadRequest(format!("zip read: {e}")))?
            == 0
        {
            continue;
        }
        if buf.len() as u64 > MAX_FILE_BYTES {
            continue;
        }
        let Ok(text) = String::from_utf8(buf) else {
            continue;
        };
        out.push((rel, text));
        if out.len() >= MAX_FILES {
            break;
        }
    }
    Ok(out)
}

fn strip_archive_root(name: &str) -> Option<String> {
    let name = name.trim_start_matches('/');
    if name.is_empty() {
        return None;
    }
    // First path component is the synthetic root folder from zipball/archive.
    let mut parts = name.splitn(2, '/');
    let _root = parts.next()?;
    let rest = parts.next().unwrap_or("");
    if rest.is_empty() {
        return None;
    }
    Some(rest.to_string())
}

fn is_safe_relative_path(path: &str) -> bool {
    if path.is_empty() || path.starts_with('/') || path.contains('\0') {
        return false;
    }
    for comp in path.split('/') {
        if comp.is_empty() || comp == "." || comp == ".." {
            return false;
        }
    }
    true
}

/// Legacy tree listing (kept for unit tests / diagnostics). Prefer `index_from_archive`.
pub async fn fetch_tree(
    provider: &str,
    remote_url: &str,
    branch: &str,
    subpath: &str,
    token: Option<&str>,
) -> AppResult<RemoteIndex> {
    let head = check_branch_head(provider, remote_url, branch, token, None).await?;
    let commit_sha = match head {
        HeadCheck::Modified { commit_sha, .. } => commit_sha,
        HeadCheck::NotModified { .. } => {
            return Err(AppError::Internal("unexpected 304 on fetch_tree".into()));
        }
    };
    // For diagnostics only — archive path does not need the recursive tree.
    let _ = subpath;
    Ok(RemoteIndex {
        commit_sha,
        entries: vec![],
    })
}

/// Test connectivity without indexing (branch tip reachable).
pub async fn test_remote(
    provider: &str,
    remote_url: &str,
    branch: &str,
    token: Option<&str>,
) -> AppResult<(bool, String, Option<String>)> {
    match check_branch_head(provider, remote_url, branch, token, None).await {
        Ok(HeadCheck::Modified { commit_sha, .. }) => Ok((
            true,
            format!(
                "Reachable via {} API; branch '{branch}' @ {}",
                provider,
                &commit_sha[..commit_sha.len().min(8)]
            ),
            Some(commit_sha),
        )),
        Ok(HeadCheck::NotModified { .. }) => Ok((
            true,
            format!("Reachable via {provider} API; branch '{branch}' unchanged"),
            None,
        )),
        Err(AppError::GitHostRateLimit { message, .. }) => Ok((false, message, None)),
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
struct GhBranch {
    name: String,
    commit: Option<GhBranchCommit>,
}

#[derive(Debug, Deserialize)]
struct GhBranchCommit {
    sha: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GlBranch {
    name: String,
    commit: Option<GlBranchCommit>,
}

#[derive(Debug, Deserialize)]
struct GlBranchCommit {
    id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

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

    #[test]
    fn rejects_path_traversal() {
        assert!(!is_safe_relative_path("../etc/passwd"));
        assert!(!is_safe_relative_path("a/../../b"));
        assert!(!is_safe_relative_path("/abs"));
        assert!(is_safe_relative_path("e2e/login.spec.ts"));
    }

    #[test]
    fn extract_zip_respects_subpath_and_safety() {
        let mut buf = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut buf);
            let opts = zip::write::SimpleFileOptions::default();
            zip.start_file("acme-demo-aaa/e2e/login.spec.ts", opts)
                .unwrap();
            zip.write_all(b"export const ok = true;\n").unwrap();
            zip.start_file("acme-demo-aaa/src/other.ts", opts).unwrap();
            zip.write_all(b"export const no = true;\n").unwrap();
            zip.start_file("acme-demo-aaa/e2e/../evil.ts", opts)
                .unwrap();
            zip.write_all(b"bad\n").unwrap();
            zip.finish().unwrap();
        }
        let bytes = buf.into_inner();
        let files = extract_zip_files(&bytes, "e2e").unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, "login.spec.ts");
        assert!(files[0].1.contains("ok"));
    }
}
