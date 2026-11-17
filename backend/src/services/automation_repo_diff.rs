//! List pull/merge requests and fetch compare diffs for AI case generation (#256).

use serde_json::Value;

use crate::error::{AppError, AppResult};
use crate::services::automation_repo_remote::{api_get, parse_remote, ApiGetOpts, HostKind};
use crate::services::egress;

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct RemotePullRequest {
    pub number: i64,
    pub title: String,
    pub state: String,
    pub source_branch: String,
    pub target_branch: String,
    pub head_sha: Option<String>,
    pub html_url: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct DiffFileEntry {
    pub path: String,
    pub status: String,
    pub additions: i64,
    pub deletions: i64,
    /// Approximate whitespace-split token count of the patch (0 when patch omitted).
    pub approx_tokens: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patch: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct DiffCompareResult {
    pub base_ref: String,
    pub head_ref: String,
    pub base_sha: Option<String>,
    pub head_sha: Option<String>,
    pub files: Vec<DiffFileEntry>,
    pub total_approx_tokens: i64,
}

pub fn approx_tokens(text: &str) -> i64 {
    text.split_whitespace().count() as i64
}

fn rest_providers(provider: &str) -> bool {
    matches!(provider, "github" | "gitlab" | "forgejo" | "gitea")
}

/// List open pull/merge requests for a linked automation repo.
pub async fn list_pull_requests(
    provider: &str,
    remote_url: &str,
    token: Option<&str>,
    policy: &egress::CiUrlPolicy,
) -> AppResult<Vec<RemotePullRequest>> {
    let provider = provider.trim().to_lowercase();
    if !rest_providers(&provider) {
        return Err(AppError::BadRequest(
            "listing pull requests requires github, gitlab, forgejo, or gitea".into(),
        ));
    }
    let host = parse_remote(&provider, remote_url, policy)?;
    let client = reqwest::Client::new();
    match host {
        HostKind::GitHub {
            api_base,
            owner,
            repo,
        }
        | HostKind::Gitea {
            api_base,
            owner,
            repo,
        } => {
            let accept = if matches!(
                provider.as_str(),
                "forgejo" | "gitea"
            ) {
                "application/json"
            } else {
                "application/vnd.github+json"
            };
            let url = format!("{api_base}/repos/{owner}/{repo}/pulls?state=open&per_page=50");
            let (_, _, bytes) = api_get(
                &client,
                &url,
                ApiGetOpts {
                    token,
                    accept,
                    etag: None,
                },
            )
            .await?;
            let arr: Vec<Value> = serde_json::from_slice(&bytes)
                .map_err(|e| AppError::BadRequest(format!("pulls JSON: {e}")))?;
            Ok(arr
                .into_iter()
                .filter_map(|p| {
                    Some(RemotePullRequest {
                        number: p.get("number")?.as_i64()?,
                        title: p.get("title")?.as_str()?.to_string(),
                        state: p
                            .get("state")
                            .and_then(|v| v.as_str())
                            .unwrap_or("open")
                            .to_string(),
                        source_branch: p
                            .pointer("/head/ref")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        target_branch: p
                            .pointer("/base/ref")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        head_sha: p
                            .pointer("/head/sha")
                            .and_then(|v| v.as_str())
                            .map(str::to_string),
                        html_url: p
                            .get("html_url")
                            .and_then(|v| v.as_str())
                            .map(str::to_string)
                            .or_else(|| {
                                p.get("url")
                                    .and_then(|v| v.as_str())
                                    .map(str::to_string)
                            }),
                        updated_at: p
                            .get("updated_at")
                            .and_then(|v| v.as_str())
                            .map(str::to_string),
                    })
                })
                .collect())
        }
        HostKind::GitLab {
            api_base,
            project_path,
        } => {
            let encoded = urlencoding::encode(&project_path);
            let url = format!(
                "{api_base}/projects/{encoded}/merge_requests?state=opened&per_page=50"
            );
            let (_, _, bytes) = api_get(
                &client,
                &url,
                ApiGetOpts {
                    token,
                    accept: "application/json",
                    etag: None,
                },
            )
            .await?;
            let arr: Vec<Value> = serde_json::from_slice(&bytes)
                .map_err(|e| AppError::BadRequest(format!("merge_requests JSON: {e}")))?;
            Ok(arr
                .into_iter()
                .filter_map(|p| {
                    Some(RemotePullRequest {
                        number: p.get("iid")?.as_i64()?,
                        title: p.get("title")?.as_str()?.to_string(),
                        state: p
                            .get("state")
                            .and_then(|v| v.as_str())
                            .unwrap_or("opened")
                            .to_string(),
                        source_branch: p
                            .get("source_branch")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        target_branch: p
                            .get("target_branch")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        head_sha: p
                            .get("sha")
                            .and_then(|v| v.as_str())
                            .map(str::to_string),
                        html_url: p
                            .get("web_url")
                            .and_then(|v| v.as_str())
                            .map(str::to_string),
                        updated_at: p
                            .get("updated_at")
                            .and_then(|v| v.as_str())
                            .map(str::to_string),
                    })
                })
                .collect())
        }
        HostKind::Bitbucket { .. } => Err(AppError::BadRequest(
            "Bitbucket pull listing is not supported for diff generation; use github, gitlab, forgejo, or gitea"
                .into(),
        )),
    }
}

/// Compare `base_ref`…`head_ref` and return changed files (optionally with patches).
pub async fn compare_refs(
    provider: &str,
    remote_url: &str,
    base_ref: &str,
    head_ref: &str,
    token: Option<&str>,
    include_patches: bool,
    policy: &egress::CiUrlPolicy,
) -> AppResult<DiffCompareResult> {
    let provider = provider.trim().to_lowercase();
    if !rest_providers(&provider) {
        return Err(AppError::BadRequest(
            "diff compare requires github, gitlab, forgejo, or gitea".into(),
        ));
    }
    let base_ref = base_ref.trim();
    let head_ref = head_ref.trim();
    if base_ref.is_empty() || head_ref.is_empty() {
        return Err(AppError::BadRequest(
            "base_ref and head_ref are required".into(),
        ));
    }
    let host = parse_remote(&provider, remote_url, policy)?;
    let client = reqwest::Client::new();
    match host {
        HostKind::GitHub {
            api_base,
            owner,
            repo,
        } => {
            let url = format!(
                "{api_base}/repos/{owner}/{repo}/compare/{}...{}",
                urlencoding::encode(base_ref),
                urlencoding::encode(head_ref)
            );
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
            parse_github_compare(&bytes, base_ref, head_ref, include_patches)
        }
        HostKind::Gitea {
            api_base,
            owner,
            repo,
        } => {
            let url = format!(
                "{api_base}/repos/{owner}/{repo}/compare/{}...{}",
                urlencoding::encode(base_ref),
                urlencoding::encode(head_ref)
            );
            let (_, _, bytes) = api_get(
                &client,
                &url,
                ApiGetOpts {
                    token,
                    accept: "application/json",
                    etag: None,
                },
            )
            .await?;
            parse_gitea_compare(&bytes, base_ref, head_ref, include_patches)
        }
        HostKind::GitLab {
            api_base,
            project_path,
        } => {
            let encoded = urlencoding::encode(&project_path);
            let url = format!(
                "{api_base}/projects/{encoded}/repository/compare?from={}&to={}",
                urlencoding::encode(base_ref),
                urlencoding::encode(head_ref)
            );
            let (_, _, bytes) = api_get(
                &client,
                &url,
                ApiGetOpts {
                    token,
                    accept: "application/json",
                    etag: None,
                },
            )
            .await?;
            parse_gitlab_compare(&bytes, base_ref, head_ref, include_patches)
        }
        HostKind::Bitbucket { .. } => Err(AppError::BadRequest(
            "Bitbucket compare is not supported for diff generation".into(),
        )),
    }
}

fn parse_github_compare(
    bytes: &[u8],
    base_ref: &str,
    head_ref: &str,
    include_patches: bool,
) -> AppResult<DiffCompareResult> {
    let body: Value = serde_json::from_slice(bytes)
        .map_err(|e| AppError::BadRequest(format!("compare JSON: {e}")))?;
    let files_raw = body
        .get("files")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut files = Vec::new();
    let mut total = 0i64;
    for f in files_raw {
        let path = f
            .get("filename")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if path.is_empty() {
            continue;
        }
        let patch = f.get("patch").and_then(|v| v.as_str()).map(str::to_string);
        let tokens = patch.as_deref().map(approx_tokens).unwrap_or(0);
        total += tokens;
        files.push(DiffFileEntry {
            path,
            status: f
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("modified")
                .to_string(),
            additions: f.get("additions").and_then(|v| v.as_i64()).unwrap_or(0),
            deletions: f.get("deletions").and_then(|v| v.as_i64()).unwrap_or(0),
            approx_tokens: tokens,
            patch: if include_patches { patch } else { None },
        });
    }
    Ok(DiffCompareResult {
        base_ref: base_ref.into(),
        head_ref: head_ref.into(),
        base_sha: body
            .pointer("/base_commit/sha")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        head_sha: body
            .pointer("/merge_base_commit/sha")
            .and_then(|v| v.as_str())
            .or_else(|| body.get("sha").and_then(|v| v.as_str()))
            .or_else(|| {
                body.pointer("/commits")
                    .and_then(|v| v.as_array())
                    .and_then(|a| a.last())
                    .and_then(|c| c.get("sha"))
                    .and_then(|v| v.as_str())
            })
            .map(str::to_string),
        files,
        total_approx_tokens: total,
    })
}

fn parse_gitea_compare(
    bytes: &[u8],
    base_ref: &str,
    head_ref: &str,
    include_patches: bool,
) -> AppResult<DiffCompareResult> {
    // Forgejo/Gitea compare payloads closely match GitHub's files[] shape.
    parse_github_compare(bytes, base_ref, head_ref, include_patches)
}

fn parse_gitlab_compare(
    bytes: &[u8],
    base_ref: &str,
    head_ref: &str,
    include_patches: bool,
) -> AppResult<DiffCompareResult> {
    let body: Value = serde_json::from_slice(bytes)
        .map_err(|e| AppError::BadRequest(format!("gitlab compare JSON: {e}")))?;
    let diffs = body
        .get("diffs")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut files = Vec::new();
    let mut total = 0i64;
    for d in diffs {
        let path = d
            .get("new_path")
            .or_else(|| d.get("old_path"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if path.is_empty() {
            continue;
        }
        let patch = d.get("diff").and_then(|v| v.as_str()).map(str::to_string);
        let tokens = patch.as_deref().map(approx_tokens).unwrap_or(0);
        total += tokens;
        let status = if d.get("new_file").and_then(|v| v.as_bool()).unwrap_or(false) {
            "added"
        } else if d
            .get("deleted_file")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            "removed"
        } else if d
            .get("renamed_file")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            "renamed"
        } else {
            "modified"
        };
        files.push(DiffFileEntry {
            path,
            status: status.into(),
            additions: 0,
            deletions: 0,
            approx_tokens: tokens,
            patch: if include_patches { patch } else { None },
        });
    }
    Ok(DiffCompareResult {
        base_ref: base_ref.into(),
        head_ref: head_ref.into(),
        base_sha: body
            .pointer("/commit/id")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        head_sha: body
            .pointer("/commit/id")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        files,
        total_approx_tokens: total,
    })
}

/// Select files within a token budget (largest patches first when over budget).
pub fn select_files_for_budget(
    files: &[DiffFileEntry],
    include_paths: Option<&[String]>,
    exclude_paths: Option<&[String]>,
    token_budget: i64,
) -> (Vec<DiffFileEntry>, i64) {
    let exclude: std::collections::HashSet<&str> = exclude_paths
        .unwrap_or(&[])
        .iter()
        .map(String::as_str)
        .collect();
    let include: Option<std::collections::HashSet<&str>> =
        include_paths.map(|p| p.iter().map(String::as_str).collect());

    let mut chosen: Vec<DiffFileEntry> = files
        .iter()
        .filter(|f| !exclude.contains(f.path.as_str()))
        .filter(|f| {
            include
                .as_ref()
                .map(|set| set.contains(f.path.as_str()))
                .unwrap_or(true)
        })
        .cloned()
        .collect();

    let budget = token_budget.max(0);
    let mut used = 0i64;
    let mut out = Vec::new();
    // Prefer smaller files first so more paths fit the budget preview.
    chosen.sort_by_key(|f| f.approx_tokens);
    for f in chosen {
        let cost = f.approx_tokens.max(1);
        if used + cost > budget && !out.is_empty() {
            continue;
        }
        if used + cost > budget && out.is_empty() {
            // Always include at least one file, truncated conceptually by budget mark.
            used += cost;
            out.push(f);
            break;
        }
        used += cost;
        out.push(f);
    }
    (out, used)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_respects_include_exclude() {
        let files = vec![
            DiffFileEntry {
                path: "a.ts".into(),
                status: "modified".into(),
                additions: 1,
                deletions: 0,
                approx_tokens: 10,
                patch: Some("fn a".into()),
            },
            DiffFileEntry {
                path: "b.ts".into(),
                status: "modified".into(),
                additions: 1,
                deletions: 0,
                approx_tokens: 20,
                patch: Some("fn b longer".into()),
            },
        ];
        let include = vec!["a.ts".into()];
        let (sel, used) = select_files_for_budget(&files, Some(&include), None, 100);
        assert_eq!(sel.len(), 1);
        assert_eq!(sel[0].path, "a.ts");
        assert_eq!(used, 10);

        let exclude = vec!["a.ts".into()];
        let (sel2, _) = select_files_for_budget(&files, None, Some(&exclude), 100);
        assert_eq!(sel2.len(), 1);
        assert_eq!(sel2[0].path, "b.ts");
    }
}
