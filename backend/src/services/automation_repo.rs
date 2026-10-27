//! Clone / fetch a project's automation Git repo and index source for LLM context.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::{AutomationRepoRow, ContextFileRef};

pub(crate) const MAX_FILE_BYTES: u64 = 200_000;
pub(crate) const MAX_FILES: usize = 800;
pub(crate) const CHUNK_LINES: usize = 40;
const DEFAULT_TOKEN_BUDGET: usize = 6_000;

const LANG_EXTS: &[(&str, &str)] = &[
    ("ts", "typescript"),
    ("tsx", "typescript"),
    ("js", "javascript"),
    ("jsx", "javascript"),
    ("mjs", "javascript"),
    ("cjs", "javascript"),
    ("py", "python"),
    ("rs", "rust"),
    ("java", "java"),
    ("kt", "kotlin"),
    ("go", "go"),
    ("cs", "csharp"),
    ("rb", "ruby"),
    ("feature", "gherkin"),
    ("json", "json"),
    ("yml", "yaml"),
    ("yaml", "yaml"),
    ("toml", "toml"),
    ("ini", "ini"),
    ("md", "markdown"),
];

fn cache_root() -> PathBuf {
    std::env::var("VULYK_REPO_CACHE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp/vulyk-repos"))
}

pub fn repo_workdir(repo_id: Uuid) -> PathBuf {
    cache_root().join(repo_id.to_string())
}

fn auth_remote_url(remote_url: &str, token: Option<&str>) -> String {
    let Some(token) = token.filter(|t| !t.is_empty()) else {
        return remote_url.to_string();
    };
    if let Some(rest) = remote_url.strip_prefix("https://") {
        // Prefer x-access-token for GitHub; GitLab/generic accept oauth2:token@host.
        if rest.contains("github.com") {
            return format!("https://x-access-token:{token}@{rest}");
        }
        if rest.contains("gitlab") {
            return format!("https://oauth2:{token}@{rest}");
        }
        return format!("https://oauth2:{token}@{rest}");
    }
    remote_url.to_string()
}

fn run_git(args: &[&str], cwd: Option<&Path>) -> AppResult<String> {
    let mut cmd = Command::new("git");
    cmd.args(args);
    cmd.env("GIT_TERMINAL_PROMPT", "0");
    // Block file:// clones in production; tests may set VULYK_EGRESS_ALLOW_PRIVATE=1.
    let allow_private = std::env::var("VULYK_EGRESS_ALLOW_PRIVATE")
        .map(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false);
    if !allow_private {
        cmd.env("GIT_CONFIG_COUNT", "1");
        cmd.env("GIT_CONFIG_KEY_0", "protocol.file.allow");
        cmd.env("GIT_CONFIG_VALUE_0", "never");
    }
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    let out = cmd.output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            AppError::BadRequest(
                "git is not installed in this environment. The generic provider needs the git binary \
                 in the API image; use the github or gitlab provider for REST indexing without git."
                    .into(),
            )
        } else {
            AppError::BadRequest(format!("git not available: {e}"))
        }
    })?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let redacted = crate::services::egress::redact_url_credentials(&stderr);
        return Err(AppError::BadRequest(format!(
            "git {} failed: {redacted}",
            args.first().unwrap_or(&"")
        )));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// True when `git` is on PATH (generic provider only).
pub fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Shallow clone or fetch+checkout the configured branch.
pub fn sync_repo(
    repo_id: Uuid,
    remote_url: &str,
    branch: &str,
    token: Option<&str>,
) -> AppResult<String> {
    crate::services::egress::validate_public_https_url(remote_url)?;
    let dir = repo_workdir(repo_id);
    let url = auth_remote_url(remote_url, token);
    if dir.join(".git").is_dir() {
        // Update remote URL (may include rotated token) then fetch.
        let _ = run_git(&["remote", "set-url", "origin", &url], Some(&dir));
        run_git(&["fetch", "--depth", "1", "origin", branch], Some(&dir))?;
        run_git(
            &["checkout", "-B", branch, &format!("origin/{branch}")],
            Some(&dir),
        )?;
    } else {
        if let Some(parent) = dir.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| AppError::Internal(format!("create repo cache dir: {e}")))?;
        }
        if dir.exists() {
            let _ = std::fs::remove_dir_all(&dir);
        }
        run_git(
            &[
                "clone",
                "--depth",
                "1",
                "--branch",
                branch,
                &url,
                dir.to_str().unwrap_or("/tmp/vulyk-repo"),
            ],
            None,
        )?;
    }
    let sha = run_git(&["rev-parse", "HEAD"], Some(&dir))?;
    Ok(sha)
}

pub fn test_connection(
    remote_url: &str,
    branch: &str,
    token: Option<&str>,
) -> AppResult<(bool, String, Option<String>)> {
    if let Err(e) = crate::services::egress::validate_public_https_url(remote_url) {
        return Ok((false, e.to_string(), None));
    }
    let url = auth_remote_url(remote_url, token);
    // ls-remote does not need a local clone.
    match run_git(&["ls-remote", "--heads", &url, branch], None) {
        Ok(out) if !out.is_empty() => {
            let sha = out.split_whitespace().next().map(str::to_string);
            Ok((true, format!("Reachable; branch '{branch}' found"), sha))
        }
        Ok(_) => Ok((
            true,
            format!("Reachable; branch '{branch}' not listed (will try clone)"),
            None,
        )),
        Err(e) => Ok((
            false,
            crate::services::egress::redact_url_credentials(&e.to_string()),
            None,
        )),
    }
}

fn language_for(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    LANG_EXTS
        .iter()
        .find(|(e, _)| *e == ext)
        .map(|(_, lang)| *lang)
}

/// Language for a repo-relative path string (used by REST indexing).
pub fn language_for_path(path: &str) -> Option<&'static str> {
    language_for(Path::new(path))
}

fn should_skip_dir(name: &str) -> bool {
    matches!(
        name,
        ".git"
            | "node_modules"
            | "dist"
            | "build"
            | "target"
            | ".venv"
            | "venv"
            | "__pycache__"
            | ".tox"
            | "coverage"
            | ".next"
            | "vendor"
    )
}

fn load_gitignore_patterns(root: &Path) -> HashSet<String> {
    let mut set = HashSet::new();
    let gi = root.join(".gitignore");
    if let Ok(text) = std::fs::read_to_string(gi) {
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            set.insert(line.trim_start_matches('/').to_string());
        }
    }
    set
}

fn ignored(rel: &str, patterns: &HashSet<String>) -> bool {
    for p in patterns {
        if p.ends_with('/') {
            if rel.starts_with(p) || rel.starts_with(p.trim_end_matches('/')) {
                return true;
            }
        } else if rel == p.as_str()
            || rel.starts_with(&format!("{p}/"))
            || rel.ends_with(p.as_str())
            || Path::new(rel).file_name().and_then(|f| f.to_str()) == Some(p.as_str())
        {
            return true;
        }
    }
    false
}

#[derive(Debug, Clone)]
pub struct IndexedFile {
    pub path: String,
    pub language: String,
    pub size_bytes: i32,
    pub content_hash: String,
    pub chunks: Vec<String>,
}

pub fn index_workdir(repo_id: Uuid, subpath: &str) -> AppResult<Vec<IndexedFile>> {
    let root = repo_workdir(repo_id);
    let root_canon = root
        .canonicalize()
        .map_err(|e| AppError::BadRequest(format!("repo workdir not available: {e}")))?;
    let walk_root = if subpath.trim().is_empty() {
        root_canon.clone()
    } else {
        let joined = root.join(subpath.trim().trim_start_matches('/'));
        let canon = joined.canonicalize().map_err(|_| {
            AppError::BadRequest(format!(
                "subpath '{}' not found in cloned repo",
                subpath.trim()
            ))
        })?;
        if !canon.starts_with(&root_canon) {
            return Err(AppError::BadRequest(
                "subpath escapes repository root".into(),
            ));
        }
        canon
    };
    if !walk_root.is_dir() {
        return Err(AppError::BadRequest(format!(
            "subpath '{}' not found in cloned repo",
            subpath.trim()
        )));
    }
    let patterns = load_gitignore_patterns(&root_canon);
    let mut out = Vec::new();
    walk_collect(&walk_root, &walk_root, &patterns, &mut out)?;
    Ok(out)
}

fn walk_collect(
    root: &Path,
    dir: &Path,
    patterns: &HashSet<String>,
    out: &mut Vec<IndexedFile>,
) -> AppResult<()> {
    if out.len() >= MAX_FILES {
        return Ok(());
    }
    let entries = std::fs::read_dir(dir)
        .map_err(|e| AppError::Internal(format!("read dir {}: {e}", dir.display())))?;
    for entry in entries.flatten() {
        if out.len() >= MAX_FILES {
            break;
        }
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if should_skip_dir(&name) {
                continue;
            }
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            if ignored(&rel, patterns) {
                continue;
            }
            walk_collect(root, &path, patterns, out)?;
            continue;
        }
        let Some(lang) = language_for(&path) else {
            continue;
        };
        let meta = entry
            .metadata()
            .map_err(|e| AppError::Internal(format!("stat: {e}")))?;
        if meta.len() > MAX_FILE_BYTES || meta.len() == 0 {
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        if ignored(&rel, patterns) {
            continue;
        }
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue, // binary / invalid utf-8
        };
        let hash = hex::encode(Sha256::digest(content.as_bytes()));
        let chunks = chunk_text(&content);
        out.push(IndexedFile {
            path: rel,
            language: lang.to_string(),
            size_bytes: meta.len() as i32,
            content_hash: hash,
            chunks,
        });
    }
    Ok(())
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

/// Persist indexed files/chunks, replacing previous index for the repo.
pub async fn persist_index(
    pool: &PgPool,
    repo_id: Uuid,
    files: &[IndexedFile],
    commit_sha: &str,
) -> AppResult<(i32, i32)> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM automation_repo_files WHERE repo_id = $1")
        .bind(repo_id)
        .execute(&mut *tx)
        .await?;

    let mut chunk_count = 0i32;
    for f in files {
        chunk_count += insert_file_with_chunks(&mut tx, repo_id, f).await?;
    }

    finalize_index_meta(
        &mut tx,
        repo_id,
        commit_sha,
        files.len() as i32,
        chunk_count,
    )
    .await?;
    tx.commit().await?;
    Ok((files.len() as i32, chunk_count))
}

/// Load path → content_hash (blob SHA for REST indexes) for incremental sync.
pub async fn load_content_hashes(
    pool: &PgPool,
    repo_id: Uuid,
) -> AppResult<HashMap<String, String>> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        r#"SELECT path, content_hash FROM automation_repo_files WHERE repo_id = $1"#,
    )
    .bind(repo_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().collect())
}

/// Incremental persist: drop removed paths, replace only changed files, keep the rest.
pub async fn persist_index_incremental(
    pool: &PgPool,
    repo_id: Uuid,
    tree_paths: &HashMap<String, String>,
    changed: &[IndexedFile],
    commit_sha: &str,
) -> AppResult<(i32, i32)> {
    let mut tx = pool.begin().await?;

    let existing: Vec<(Uuid, String)> =
        sqlx::query_as(r#"SELECT id, path FROM automation_repo_files WHERE repo_id = $1"#)
            .bind(repo_id)
            .fetch_all(&mut *tx)
            .await?;

    for (file_id, path) in &existing {
        if !tree_paths.contains_key(path) {
            sqlx::query("DELETE FROM automation_repo_files WHERE id = $1")
                .bind(file_id)
                .execute(&mut *tx)
                .await?;
        }
    }

    for f in changed {
        sqlx::query("DELETE FROM automation_repo_files WHERE repo_id = $1 AND path = $2")
            .bind(repo_id)
            .bind(&f.path)
            .execute(&mut *tx)
            .await?;
        insert_file_with_chunks(&mut tx, repo_id, f).await?;
    }

    let file_count: i64 =
        sqlx::query_scalar(r#"SELECT COUNT(*) FROM automation_repo_files WHERE repo_id = $1"#)
            .bind(repo_id)
            .fetch_one(&mut *tx)
            .await?;
    let chunk_count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM automation_repo_chunks c
           JOIN automation_repo_files f ON f.id = c.file_id
           WHERE f.repo_id = $1"#,
    )
    .bind(repo_id)
    .fetch_one(&mut *tx)
    .await?;

    finalize_index_meta(
        &mut tx,
        repo_id,
        commit_sha,
        file_count as i32,
        chunk_count as i32,
    )
    .await?;
    tx.commit().await?;
    Ok((file_count as i32, chunk_count as i32))
}

async fn insert_file_with_chunks(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    repo_id: Uuid,
    f: &IndexedFile,
) -> AppResult<i32> {
    let file_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO automation_repo_files (repo_id, path, language, size_bytes, content_hash)
           VALUES ($1, $2, $3, $4, $5) RETURNING id"#,
    )
    .bind(repo_id)
    .bind(&f.path)
    .bind(&f.language)
    .bind(f.size_bytes)
    .bind(&f.content_hash)
    .fetch_one(&mut **tx)
    .await?;
    let mut chunk_count = 0i32;
    for (idx, chunk) in f.chunks.iter().enumerate() {
        sqlx::query(
            r#"INSERT INTO automation_repo_chunks (file_id, chunk_index, content)
               VALUES ($1, $2, $3)"#,
        )
        .bind(file_id)
        .bind(idx as i32)
        .bind(chunk)
        .execute(&mut **tx)
        .await?;
        chunk_count += 1;
    }
    Ok(chunk_count)
}

async fn finalize_index_meta(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    repo_id: Uuid,
    commit_sha: &str,
    file_count: i32,
    chunk_count: i32,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE automation_repos SET
             index_status = 'ready',
             last_indexed_at = NOW(),
             last_index_error = NULL,
             commit_sha = $2,
             file_count = $3,
             chunk_count = $4,
             updated_at = NOW()
           WHERE id = $1"#,
    )
    .bind(repo_id)
    .bind(commit_sha)
    .bind(file_count)
    .bind(chunk_count)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn mark_indexing(pool: &PgPool, repo_id: Uuid) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE automation_repos SET index_status = 'indexing', last_index_error = NULL, updated_at = NOW()
           WHERE id = $1"#,
    )
    .bind(repo_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn mark_index_error(pool: &PgPool, repo_id: Uuid, err: &str) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE automation_repos SET index_status = 'error', last_index_error = $2, updated_at = NOW()
           WHERE id = $1"#,
    )
    .bind(repo_id)
    .bind(err)
    .execute(pool)
    .await?;
    Ok(())
}

#[derive(Debug, sqlx::FromRow)]
struct ChunkRow {
    path: String,
    language: String,
    content: String,
}

fn tokenize(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|t| t.len() > 1)
        .map(str::to_string)
        .collect()
}

/// Simple BM25 over indexed chunks; returns ranked context file refs + joined text.
pub async fn retrieve_context(
    pool: &PgPool,
    repo_id: Uuid,
    query: &str,
    token_budget: Option<usize>,
) -> AppResult<(Vec<ContextFileRef>, String)> {
    let budget = token_budget.unwrap_or(DEFAULT_TOKEN_BUDGET);
    let rows: Vec<ChunkRow> = sqlx::query_as(
        r#"SELECT f.path, f.language, c.content
           FROM automation_repo_chunks c
           JOIN automation_repo_files f ON f.id = c.file_id
           WHERE f.repo_id = $1"#,
    )
    .bind(repo_id)
    .fetch_all(pool)
    .await?;

    if rows.is_empty() {
        return Ok((vec![], String::new()));
    }

    let docs: Vec<Vec<String>> = rows.iter().map(|r| tokenize(&r.content)).collect();
    let n = docs.len() as f64;
    let avgdl = docs.iter().map(|d| d.len() as f64).sum::<f64>() / n.max(1.0);
    let mut df: HashMap<String, f64> = HashMap::new();
    for doc in &docs {
        let mut seen = HashSet::new();
        for t in doc {
            if seen.insert(t.clone()) {
                *df.entry(t.clone()).or_insert(0.0) += 1.0;
            }
        }
    }

    let q_terms = tokenize(query);
    let k1 = 1.5_f64;
    let b = 0.75_f64;
    let mut scored: Vec<(usize, f64)> = Vec::new();
    for (i, doc) in docs.iter().enumerate() {
        let mut tf: HashMap<&str, f64> = HashMap::new();
        for t in doc {
            *tf.entry(t.as_str()).or_insert(0.0) += 1.0;
        }
        let dl = doc.len() as f64;
        let mut score = 0.0;
        for term in &q_terms {
            let freq = *tf.get(term.as_str()).unwrap_or(&0.0);
            if freq == 0.0 {
                continue;
            }
            let n_q = *df.get(term).unwrap_or(&0.0);
            let idf = ((n - n_q + 0.5) / (n_q + 0.5) + 1.0).ln();
            let denom = freq + k1 * (1.0 - b + b * dl / avgdl.max(1.0));
            score += idf * (freq * (k1 + 1.0) / denom);
        }
        // Boost known framework/config and helper paths.
        let path = rows[i].path.to_lowercase();
        if path.contains("playwright.config")
            || path.contains("pytest.ini")
            || path.contains("package.json")
            || path.contains("conftest")
            || path.contains("fixture")
            || path.contains("page")
            || path.contains("helper")
        {
            score += 1.5;
        }
        if score > 0.0 {
            scored.push((i, score));
        }
    }
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut used_paths: HashMap<String, ContextFileRef> = HashMap::new();
    let mut context = String::new();
    let mut tokens_used = 0usize;

    // Always try to include framework detection files first.
    for (i, row) in rows.iter().enumerate() {
        let p = row.path.to_lowercase();
        if p.ends_with("playwright.config.ts")
            || p.ends_with("playwright.config.js")
            || p.ends_with("pytest.ini")
            || p.ends_with("package.json")
            || p.ends_with("conftest.py")
        {
            let approx = row.content.split_whitespace().count();
            if tokens_used + approx > budget {
                continue;
            }
            context.push_str(&format!(
                "\n--- FILE: {} ({}) [framework/config] ---\n{}\n",
                row.path, row.language, row.content
            ));
            tokens_used += approx;
            used_paths.insert(
                row.path.clone(),
                ContextFileRef {
                    path: row.path.clone(),
                    language: row.language.clone(),
                    score: scored
                        .iter()
                        .find(|(idx, _)| *idx == i)
                        .map(|(_, s)| *s)
                        .unwrap_or(2.0),
                    reason: "framework/config".into(),
                },
            );
        }
    }

    for (i, score) in scored {
        let row = &rows[i];
        if used_paths.contains_key(&row.path) {
            // Still may add another chunk from same file if budget allows.
        }
        let approx = row.content.split_whitespace().count();
        if tokens_used + approx > budget {
            continue;
        }
        context.push_str(&format!(
            "\n--- FILE: {} ({}) [score={score:.3}] ---\n{}\n",
            row.path, row.language, row.content
        ));
        tokens_used += approx;
        used_paths
            .entry(row.path.clone())
            .and_modify(|r| {
                if score > r.score {
                    r.score = score;
                }
            })
            .or_insert(ContextFileRef {
                path: row.path.clone(),
                language: row.language.clone(),
                score,
                reason: "bm25".into(),
            });
    }

    let mut refs: Vec<_> = used_paths.into_values().collect();
    refs.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok((refs, context))
}

const REPO_SELECT: &str = r#"id, name, project_id, provider, remote_url, branch, subpath,
    token_ciphertext, token_nonce, index_status, last_indexed_at,
    last_index_error, commit_sha, file_count, chunk_count, created_at, updated_at"#;

const REPO_SELECT_R: &str = r#"r.id, r.name, r.project_id, r.provider, r.remote_url, r.branch, r.subpath,
    r.token_ciphertext, r.token_nonce, r.index_status, r.last_indexed_at,
    r.last_index_error, r.commit_sha, r.file_count, r.chunk_count, r.created_at, r.updated_at"#;

/// Prefer the project's default linked repo; otherwise the first by sort_order.
pub async fn load_repo_for_project(
    pool: &PgPool,
    project_id: Uuid,
) -> AppResult<Option<AutomationRepoRow>> {
    let row = sqlx::query_as::<_, AutomationRepoRow>(&format!(
        r#"SELECT {REPO_SELECT_R}
           FROM automation_repos r
           JOIN project_automation_repos par ON par.repo_id = r.id
           WHERE par.project_id = $1
           ORDER BY par.is_default DESC, par.sort_order ASC, r.created_at ASC
           LIMIT 1"#
    ))
    .bind(project_id)
    .fetch_optional(pool)
    .await?;
    if row.is_some() {
        return Ok(row);
    }
    // Fallback: legacy rows still keyed by automation_repos.project_id without junction.
    let legacy = sqlx::query_as::<_, AutomationRepoRow>(&format!(
        r#"SELECT {REPO_SELECT} FROM automation_repos WHERE project_id = $1 LIMIT 1"#
    ))
    .bind(project_id)
    .fetch_optional(pool)
    .await?;
    Ok(legacy)
}

pub async fn load_repos_for_project(
    pool: &PgPool,
    project_id: Uuid,
) -> AppResult<Vec<AutomationRepoRow>> {
    let rows = sqlx::query_as::<_, AutomationRepoRow>(&format!(
        r#"SELECT {REPO_SELECT_R}
           FROM automation_repos r
           JOIN project_automation_repos par ON par.repo_id = r.id
           WHERE par.project_id = $1
           ORDER BY par.is_default DESC, par.sort_order ASC, r.name ASC"#
    ))
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn load_repo_by_id(pool: &PgPool, repo_id: Uuid) -> AppResult<Option<AutomationRepoRow>> {
    let row = sqlx::query_as::<_, AutomationRepoRow>(&format!(
        r#"SELECT {REPO_SELECT} FROM automation_repos WHERE id = $1"#
    ))
    .bind(repo_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn list_repos(pool: &PgPool) -> AppResult<Vec<AutomationRepoRow>> {
    let rows = sqlx::query_as::<_, AutomationRepoRow>(&format!(
        r#"SELECT {REPO_SELECT} FROM automation_repos ORDER BY name ASC, created_at ASC"#
    ))
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Repos visible to a user: all when system admin / open mode; otherwise those linked
/// to projects where the user is a member.
pub async fn list_repos_for_user(
    pool: &PgPool,
    user_id: Option<Uuid>,
    is_system_admin: bool,
) -> AppResult<Vec<AutomationRepoRow>> {
    if is_system_admin || user_id.is_none() {
        return list_repos(pool).await;
    }
    let user_id = user_id.unwrap();
    let rows = sqlx::query_as::<_, AutomationRepoRow>(&format!(
        r#"SELECT DISTINCT ON (r.id) {REPO_SELECT_R}
           FROM automation_repos r
           JOIN project_automation_repos par ON par.repo_id = r.id
           JOIN memberships m ON m.project_id = par.project_id
           WHERE m.user_id = $1
           ORDER BY r.id, r.name ASC"#
    ))
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn project_ids_for_repo(pool: &PgPool, repo_id: Uuid) -> AppResult<Vec<Uuid>> {
    let ids = sqlx::query_scalar::<_, Uuid>(
        r#"SELECT project_id FROM project_automation_repos
           WHERE repo_id = $1
           ORDER BY is_default DESC, sort_order ASC, project_id"#,
    )
    .bind(repo_id)
    .fetch_all(pool)
    .await?;
    Ok(ids)
}

/// Replace the project link set for a repo. `default_project_id` marks is_default.
pub async fn set_project_links(
    pool: &PgPool,
    repo_id: Uuid,
    project_ids: &[Uuid],
    default_project_id: Option<Uuid>,
) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM project_automation_repos WHERE repo_id = $1")
        .bind(repo_id)
        .execute(&mut *tx)
        .await?;

    let default_id = default_project_id
        .filter(|id| project_ids.contains(id))
        .or_else(|| project_ids.first().copied());

    for (idx, pid) in project_ids.iter().enumerate() {
        let is_default = Some(*pid) == default_id;
        // Clear existing default on the project before inserting ours.
        if is_default {
            sqlx::query(
                r#"UPDATE project_automation_repos SET is_default = FALSE
                   WHERE project_id = $1 AND is_default"#,
            )
            .bind(pid)
            .execute(&mut *tx)
            .await?;
        }
        sqlx::query(
            r#"INSERT INTO project_automation_repos (project_id, repo_id, is_default, sort_order)
               VALUES ($1, $2, $3, $4)"#,
        )
        .bind(pid)
        .bind(repo_id)
        .bind(is_default)
        .bind(idx as i32)
        .execute(&mut *tx)
        .await?;
    }

    // Keep legacy project_id column pointing at the default (or first) link.
    sqlx::query(r#"UPDATE automation_repos SET project_id = $2, updated_at = NOW() WHERE id = $1"#)
        .bind(repo_id)
        .bind(default_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_splits_lines() {
        let text = (0..100)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let chunks = chunk_text(&text);
        assert!(chunks.len() >= 2);
        assert!(chunks[0].contains("line 0"));
    }

    #[test]
    fn auth_url_injects_github_token() {
        let u = auth_remote_url("https://github.com/acme/tests.git", Some("tok"));
        assert!(u.contains("x-access-token:tok@github.com"));
    }
}
