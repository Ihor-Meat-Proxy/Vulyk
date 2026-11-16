//! Admin Updates: check GitHub Releases and apply via the updater sidecar.

use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::auth::RequireSystemAdmin;
use crate::error::{AppError, AppResult};
use crate::routes::AppState;
use crate::{db, settings};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/admin/updates", get(get_updates))
        .route("/api/v1/admin/updates/check", post(check_updates))
        .route("/api/v1/admin/updates/apply", post(apply_update))
        .route("/api/v1/admin/updates/rollback", post(apply_rollback))
        .route("/api/v1/admin/updates/jobs", get(list_jobs))
        .route("/api/v1/admin/updates/jobs/{job_id}", get(get_job))
        .route(
            "/api/v1/admin/updates/version-backups",
            get(list_version_backups),
        )
        .route(
            "/api/v1/admin/updates/version-backups/{backup_id}/download",
            get(download_version_backup),
        )
        .route(
            "/api/v1/admin/updates/preferences",
            get(get_prefs).put(put_prefs),
        )
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum UpdatesCapability {
    ComposeUpdater,
    ManualOnly,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct VersionBackupInfo {
    pub id: String,
    pub version: Option<String>,
    pub from_version: Option<String>,
    pub to_version: Option<String>,
    pub roles: Vec<String>,
    pub created_at: Option<String>,
    pub expires_at: Option<String>,
    pub size_bytes: i64,
    pub in_progress: bool,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RollbackPointInfo {
    pub id: String,
    pub version: String,
    pub from_version: Option<String>,
    pub to_version: Option<String>,
    pub expires_at: Option<String>,
    pub created_at: Option<String>,
    pub size_bytes: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct VersionBackupRetention {
    pub rollback_days: i64,
    pub version_count: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct VersionBackupList {
    pub items: Vec<VersionBackupInfo>,
    pub rollback: Option<RollbackPointInfo>,
    pub retention: VersionBackupRetention,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UpdatesStatus {
    pub current_version: &'static str,
    pub latest_version: Option<String>,
    pub latest_notes: Option<String>,
    pub latest_url: Option<String>,
    pub update_available: bool,
    /// RFC3339 time of the last check **attempt** (success or failure).
    ///
    /// Always advanced on both success and failure so auto-check interval /
    /// backoff can suppress hammering GitHub. Last-good `latest_*` metadata is
    /// kept separately on failure.
    pub last_checked_at: Option<String>,
    /// Stable machine code for the last failed check (`git_host_rate_limit`, …).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_check_error_code: Option<String>,
    /// RFC3339 timestamp of the last failed check.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_check_error_at: Option<String>,
    /// RFC3339 rate-limit reset time when known (`x-ratelimit-reset`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_check_error_reset_at: Option<String>,
    pub check_interval_hours: i64,
    pub capability: UpdatesCapability,
    pub manual_command: String,
    pub rollback_command: String,
    pub github_repo: String,
    pub active_job: Option<UpdateJobPublic>,
    pub rollback: Option<RollbackPointInfo>,
    pub version_backups: Vec<VersionBackupInfo>,
    pub retention: VersionBackupRetention,
}

/// Stable codes persisted in `updates_last_check_error_code` and returned to the UI.
pub const CHECK_ERR_RATE_LIMIT: &str = "git_host_rate_limit";
pub const CHECK_ERR_NETWORK: &str = "network_error";
pub const CHECK_ERR_HTTP: &str = "github_http_error";
pub const CHECK_ERR_PARSE: &str = "github_parse_error";

#[derive(Debug, Serialize, ToSchema, FromRow)]
pub struct UpdateJobPublic {
    pub id: Uuid,
    pub target_version: String,
    pub status: String,
    pub progress: i32,
    pub message: String,
    pub log_text: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UpdateJobList {
    pub items: Vec<UpdateJobPublic>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ApplyUpdateRequest {
    pub version: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ApplyRollbackRequest {
    /// Must be the literal string `ROLLBACK`.
    pub confirm: String,
    /// Optional explicit target version; defaults to the active rollback point.
    pub version: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ApplyUpdateResponse {
    pub job: UpdateJobPublic,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct UpdatesPreferences {
    pub check_interval_hours: i64,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdatePreferencesRequest {
    pub check_interval_hours: Option<i64>,
}

fn github_repo() -> String {
    std::env::var("VULYK_GITHUB_REPO").unwrap_or_else(|_| "Ihor-Meat-Proxy/Vulyk".into())
}

/// Optional PAT / fine-grained token for GitHub Releases (raises unauthenticated rate limit).
fn github_token() -> Option<String> {
    std::env::var("VULYK_GITHUB_TOKEN")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn opt_setting(s: String) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Map a non-success GitHub Releases HTTP response to a stable UI code + optional reset time.
pub fn classify_github_release_http_error(
    status: reqwest::StatusCode,
    headers: &reqwest::header::HeaderMap,
    body: &str,
) -> (&'static str, Option<DateTime<Utc>>) {
    let code = status.as_u16();
    if code == 429 || code == 403 {
        let remaining = headers
            .get("x-ratelimit-remaining")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("?");
        if remaining == "0" || code == 429 || body.to_ascii_lowercase().contains("rate limit") {
            let err = crate::services::automation_repo_remote::rate_limit_error(headers, body);
            let reset_at = match err {
                AppError::GitHostRateLimit { reset_at, .. } => reset_at,
                _ => crate::services::automation_repo_remote::rate_limit_reset(headers),
            };
            return (CHECK_ERR_RATE_LIMIT, reset_at);
        }
    }
    (CHECK_ERR_HTTP, None)
}

async fn clear_last_check_error(pool: &sqlx::PgPool) -> AppResult<()> {
    set_string_setting(pool, "updates_last_check_error_code", "").await?;
    set_string_setting(pool, "updates_last_check_error_at", "").await?;
    set_string_setting(pool, "updates_last_check_error_reset_at", "").await?;
    Ok(())
}

async fn persist_last_check_error(
    pool: &sqlx::PgPool,
    code: &str,
    reset_at: Option<DateTime<Utc>>,
) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    set_string_setting(pool, "updates_last_check_error_code", code).await?;
    set_string_setting(pool, "updates_last_check_error_at", &now).await?;
    set_string_setting(
        pool,
        "updates_last_check_error_reset_at",
        &reset_at.map(|t| t.to_rfc3339()).unwrap_or_default(),
    )
    .await?;
    // Record the attempt time even on failure so clients can honour the
    // check interval / rate-limit reset instead of re-hitting GitHub on every visit.
    set_string_setting(pool, "updates_last_checked_at", &now).await?;
    Ok(())
}

/// Whether an automatic (non-Retry) update check should run now.
///
/// - Rate limit with known `reset_at`: wait until that instant (even if shorter
///   or longer than the configured interval).
/// - Other errors (or rate limit without reset): wait
///   `min(interval_hours, 1)` hour(s) from the last attempt.
/// - After success: wait the full `interval_hours` from `last_checked_at`.
/// - `interval_hours <= 0` means periodic checks are disabled.
/// - Explicit Retry / Check passes `force=true` and bypasses this gate.
pub fn auto_check_due(
    now: DateTime<Utc>,
    interval_hours: i64,
    last_checked_at: Option<DateTime<Utc>>,
    error_code: Option<&str>,
    reset_at: Option<DateTime<Utc>>,
) -> bool {
    if interval_hours <= 0 {
        return false;
    }
    let has_error = error_code.map(|c| !c.is_empty()).unwrap_or(false);
    if has_error {
        if error_code == Some(CHECK_ERR_RATE_LIMIT) {
            if let Some(reset) = reset_at {
                return now >= reset;
            }
        }
        let Some(last) = last_checked_at else {
            return true;
        };
        let wait_hours = interval_hours.min(1);
        return now >= last + chrono::Duration::hours(wait_hours);
    }
    let Some(last) = last_checked_at else {
        return true;
    };
    now >= last + chrono::Duration::hours(interval_hours)
}

fn parse_rfc3339_opt(raw: &str) -> Option<DateTime<Utc>> {
    if raw.is_empty() {
        return None;
    }
    DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

/// Persist a GitHub Releases check outcome.
///
/// Success clears `last_check_error_*` and refreshes latest release fields.
/// Failure records the error **and** advances `updates_last_checked_at` (via
/// [`persist_last_check_error`]) while leaving last-good `latest_*` untouched.
pub async fn apply_check_outcome(
    pool: &sqlx::PgPool,
    outcome: Result<Option<GithubReleaseInfo>, (&'static str, Option<DateTime<Utc>>)>,
) -> AppResult<()> {
    match outcome {
        Ok(None) => {
            clear_last_check_error(pool).await?;
            set_string_setting(pool, "updates_last_checked_at", &Utc::now().to_rfc3339()).await?;
        }
        Ok(Some(release)) => {
            clear_last_check_error(pool).await?;
            set_string_setting(pool, "updates_latest_version", &release.tag).await?;
            set_string_setting(pool, "updates_latest_notes", &release.notes).await?;
            set_string_setting(pool, "updates_latest_url", &release.html_url).await?;
            set_string_setting(pool, "updates_last_checked_at", &Utc::now().to_rfc3339()).await?;
        }
        Err((code, reset_at)) => {
            persist_last_check_error(pool, code, reset_at).await?;
        }
    }
    Ok(())
}

fn updater_url() -> Option<String> {
    std::env::var("VULYK_UPDATER_URL")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn updater_token() -> Option<String> {
    std::env::var("VULYK_UPDATER_TOKEN")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn capability() -> UpdatesCapability {
    match (updater_url(), updater_token()) {
        (Some(_), Some(_)) => UpdatesCapability::ComposeUpdater,
        _ => UpdatesCapability::ManualOnly,
    }
}

fn manual_command(version: &str) -> String {
    match capability() {
        UpdatesCapability::ComposeUpdater => {
            format!("./scripts/install.sh upgrade --yes --version {version}")
        }
        UpdatesCapability::ManualOnly => {
            let ver = if version.is_empty() { "X.Y.Z" } else { version };
            format!(
                "# Compose:\n./scripts/install.sh upgrade --yes --version {ver}\n\n# Helm:\nhelm upgrade vulyk deploy/helm/vulyk --namespace vulyk \\\n  --set image.api=ghcr.io/ihor-meat-proxy/vulyk-api:{ver} \\\n  --set image.web=ghcr.io/ihor-meat-proxy/vulyk-web:{ver}"
            )
        }
    }
}

fn rollback_command(version: Option<&str>) -> String {
    match version {
        Some(v) if !v.is_empty() => {
            format!("./scripts/install.sh rollback --version {v} --confirm=ROLLBACK")
        }
        _ => "./scripts/install.sh rollback --confirm=ROLLBACK".into(),
    }
}

fn default_retention() -> VersionBackupRetention {
    let days = std::env::var("VULYK_ROLLBACK_RETENTION_DAYS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(7);
    let count = std::env::var("VULYK_VERSION_BACKUP_COUNT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(3);
    VersionBackupRetention {
        rollback_days: days,
        version_count: count,
    }
}

async fn write_audit(
    pool: &sqlx::PgPool,
    actor: Option<Uuid>,
    action: &str,
    resource_type: &str,
    resource_id: &str,
    details: serde_json::Value,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO audit_events (actor_user_id, action, resource_type, resource_id, details)
           VALUES ($1, $2, $3, $4, $5)"#,
    )
    .bind(actor)
    .bind(action)
    .bind(resource_type)
    .bind(resource_id)
    .bind(details)
    .execute(pool)
    .await?;
    Ok(())
}

async fn fetch_version_backups_from_sidecar(
    current_version: &str,
) -> AppResult<Option<VersionBackupList>> {
    let (Some(base), Some(token)) = (updater_url(), updater_token()) else {
        return Ok(None);
    };
    let client = reqwest::Client::new();
    let url = format!(
        "{}/v1/version-backups?current_version={}",
        base.trim_end_matches('/'),
        urlencoding_loose(current_version)
    );
    let resp = client
        .get(&url)
        .header("X-Vulyk-Updater-Token", token)
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("updater unreachable: {e}")))?;
    if !resp.status().is_success() {
        return Ok(None);
    }
    let payload: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| AppError::BadRequest(format!("updater backups json: {e}")))?;
    Ok(Some(parse_version_backup_list(&payload)))
}

fn urlencoding_loose(s: &str) -> String {
    s.replace(' ', "%20")
}

fn parse_version_backup_list(payload: &serde_json::Value) -> VersionBackupList {
    let retention = VersionBackupRetention {
        rollback_days: payload
            .pointer("/retention/rollback_days")
            .and_then(|v| v.as_i64())
            .unwrap_or(default_retention().rollback_days),
        version_count: payload
            .pointer("/retention/version_count")
            .and_then(|v| v.as_i64())
            .unwrap_or(default_retention().version_count),
    };
    let items = payload
        .get("items")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|it| {
                    Some(VersionBackupInfo {
                        id: it.get("id")?.as_str()?.to_string(),
                        version: it
                            .get("version")
                            .and_then(|v| v.as_str())
                            .map(str::to_string),
                        from_version: it
                            .get("from_version")
                            .and_then(|v| v.as_str())
                            .map(str::to_string),
                        to_version: it
                            .get("to_version")
                            .and_then(|v| v.as_str())
                            .map(str::to_string),
                        roles: it
                            .get("roles")
                            .and_then(|v| v.as_array())
                            .map(|a| {
                                a.iter()
                                    .filter_map(|x| x.as_str().map(str::to_string))
                                    .collect()
                            })
                            .unwrap_or_default(),
                        created_at: it
                            .get("created_at")
                            .and_then(|v| v.as_str())
                            .map(str::to_string),
                        expires_at: it
                            .get("expires_at")
                            .and_then(|v| v.as_str())
                            .map(str::to_string),
                        size_bytes: it.get("size_bytes").and_then(|v| v.as_i64()).unwrap_or(0),
                        in_progress: it
                            .get("in_progress")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let rollback = payload.get("rollback").and_then(|rb| {
        if rb.is_null() {
            return None;
        }
        Some(RollbackPointInfo {
            id: rb.get("id")?.as_str()?.to_string(),
            version: rb
                .get("version")
                .or_else(|| rb.get("from_version"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            from_version: rb
                .get("from_version")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            to_version: rb
                .get("to_version")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            expires_at: rb
                .get("expires_at")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            created_at: rb
                .get("created_at")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            size_bytes: rb.get("size_bytes").and_then(|v| v.as_i64()).unwrap_or(0),
        })
    });
    VersionBackupList {
        items,
        rollback,
        retention,
    }
}

fn normalize_version(raw: &str) -> AppResult<String> {
    let v = raw.trim().trim_start_matches('v');
    if v.is_empty()
        || !v
            .chars()
            .next()
            .map(|c| c.is_ascii_digit())
            .unwrap_or(false)
    {
        return Err(AppError::BadRequest("invalid version".into()));
    }
    Ok(v.to_string())
}

fn semver_gt(a: &str, b: &str) -> bool {
    fn parts(s: &str) -> Vec<u64> {
        s.split('.')
            .take(3)
            .map(|p| {
                p.chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect::<String>()
                    .parse()
                    .unwrap_or(0)
            })
            .collect()
    }
    let pa = parts(a);
    let pb = parts(b);
    for i in 0..3 {
        let x = pa.get(i).copied().unwrap_or(0);
        let y = pb.get(i).copied().unwrap_or(0);
        if x != y {
            return x > y;
        }
    }
    false
}

async fn load_job(pool: &sqlx::PgPool, id: Uuid) -> AppResult<UpdateJobPublic> {
    sqlx::query_as::<_, UpdateJobPublic>(
        r#"SELECT id, target_version, status, progress, message, log_text,
                  created_at, updated_at, finished_at
           FROM update_jobs WHERE id = $1"#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("update job".into()))
}

async fn active_job(pool: &sqlx::PgPool) -> AppResult<Option<UpdateJobPublic>> {
    Ok(sqlx::query_as::<_, UpdateJobPublic>(
        r#"SELECT id, target_version, status, progress, message, log_text,
                  created_at, updated_at, finished_at
           FROM update_jobs
           WHERE status IN ('pending', 'running')
           ORDER BY created_at DESC
           LIMIT 1"#,
    )
    .fetch_optional(pool)
    .await?)
}

async fn prefs_interval(pool: &sqlx::PgPool) -> AppResult<i64> {
    let value: Option<serde_json::Value> =
        sqlx::query_scalar("SELECT value FROM system_settings WHERE key = $1")
            .bind("updates_check_interval_hours")
            .fetch_optional(pool)
            .await?;
    let n = match value {
        Some(serde_json::Value::Number(n)) => n.as_i64().unwrap_or(24),
        Some(serde_json::Value::String(s)) => s.parse().unwrap_or(24),
        _ => 24,
    };
    Ok(n.clamp(0, 24 * 30))
}

async fn set_string_setting(pool: &sqlx::PgPool, key: &str, value: &str) -> AppResult<()> {
    settings::set_value(pool, key, serde_json::Value::String(value.to_string())).await
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/updates",
    tag = "admin",
    responses((status = 200, body = UpdatesStatus)),
    security(("bearer_auth" = []))
)]
pub async fn get_updates(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
) -> AppResult<Json<UpdatesStatus>> {
    Ok(Json(build_status(&state).await?))
}

async fn build_status(state: &AppState) -> AppResult<UpdatesStatus> {
    let current = db::crate_version();
    let latest = settings::get_string(&state.pool, "updates_latest_version", "").await?;
    let notes = settings::get_string(&state.pool, "updates_latest_notes", "").await?;
    let url = settings::get_string(&state.pool, "updates_latest_url", "").await?;
    let last = settings::get_string(&state.pool, "updates_last_checked_at", "").await?;
    let err_code = settings::get_string(&state.pool, "updates_last_check_error_code", "").await?;
    let err_at = settings::get_string(&state.pool, "updates_last_check_error_at", "").await?;
    let err_reset =
        settings::get_string(&state.pool, "updates_last_check_error_reset_at", "").await?;
    let interval = prefs_interval(&state.pool).await?;
    let latest_clean = opt_setting(latest);
    let update_available = latest_clean
        .as_ref()
        .map(|l| semver_gt(l, current))
        .unwrap_or(false);
    let target = latest_clean.clone().unwrap_or_else(|| current.to_string());
    let backups = fetch_version_backups_from_sidecar(current)
        .await?
        .unwrap_or_else(|| VersionBackupList {
            items: vec![],
            rollback: None,
            retention: default_retention(),
        });
    let rb_ver = backups.rollback.as_ref().map(|r| r.version.as_str());
    Ok(UpdatesStatus {
        current_version: current,
        latest_version: latest_clean,
        latest_notes: opt_setting(notes),
        latest_url: opt_setting(url),
        update_available,
        last_checked_at: opt_setting(last),
        last_check_error_code: opt_setting(err_code),
        last_check_error_at: opt_setting(err_at),
        last_check_error_reset_at: opt_setting(err_reset),
        check_interval_hours: interval,
        capability: capability(),
        manual_command: manual_command(&target),
        rollback_command: rollback_command(rb_ver),
        github_repo: github_repo(),
        active_job: active_job(&state.pool).await?,
        rollback: backups.rollback,
        version_backups: backups.items,
        retention: backups.retention,
    })
}

#[derive(Debug, Deserialize, ToSchema, Default)]
pub struct CheckUpdatesQuery {
    /// When true, bypass auto-check backoff (Admin Retry / Check button).
    #[serde(default)]
    pub force: bool,
}

#[utoipa::path(
    post,
    path = "/api/v1/admin/updates/check",
    tag = "admin",
    params(
        ("force" = Option<bool>, Query, description = "Bypass auto-check backoff")
    ),
    responses((status = 200, body = UpdatesStatus)),
    security(("bearer_auth" = []))
)]
pub async fn check_updates(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
    Query(q): Query<CheckUpdatesQuery>,
) -> AppResult<Json<UpdatesStatus>> {
    // Always return 200 with UpdatesStatus (including last_check_error_*) so the
    // Admin UI can show a durable reason after refresh without relying on thrown
    // API errors alone. Last-good latest_version/notes/url are kept on failure;
    // last_checked_at advances on every attempt for backoff.
    if !q.force {
        let interval = prefs_interval(&state.pool).await?;
        let last_raw = settings::get_string(&state.pool, "updates_last_checked_at", "").await?;
        let err_code =
            settings::get_string(&state.pool, "updates_last_check_error_code", "").await?;
        let err_reset =
            settings::get_string(&state.pool, "updates_last_check_error_reset_at", "").await?;
        let due = auto_check_due(
            Utc::now(),
            interval,
            parse_rfc3339_opt(&last_raw),
            opt_setting(err_code).as_deref(),
            parse_rfc3339_opt(&err_reset),
        );
        if !due {
            return Ok(Json(build_status(&state).await?));
        }
    }
    let outcome = fetch_latest_github_release().await;
    apply_check_outcome(&state.pool, outcome).await?;
    Ok(Json(build_status(&state).await?))
}

#[derive(Debug, Clone)]
pub struct GithubReleaseInfo {
    pub tag: String,
    pub notes: String,
    pub html_url: String,
}

/// Fetch GitHub `/releases/latest`. On failure returns a stable error code (+ reset).
async fn fetch_latest_github_release(
) -> Result<Option<GithubReleaseInfo>, (&'static str, Option<DateTime<Utc>>)> {
    let repo = github_repo();
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let client = reqwest::Client::new();
    let mut req = client
        .get(&url)
        .header("User-Agent", "vulyk-api-updates")
        .header("Accept", "application/vnd.github+json");
    if let Some(token) = github_token() {
        req = req.bearer_auth(token);
    }
    let resp = match req.send().await {
        Ok(r) => r,
        Err(_) => return Err((CHECK_ERR_NETWORK, None)),
    };
    let status = resp.status();
    let headers = resp.headers().clone();
    if status == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        let (code, reset) = classify_github_release_http_error(status, &headers, &body);
        return Err((code, reset));
    }
    let payload: serde_json::Value = match resp.json().await {
        Ok(v) => v,
        Err(_) => return Err((CHECK_ERR_PARSE, None)),
    };
    let tag = payload
        .get("tag_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim_start_matches('v')
        .to_string();
    let notes = payload
        .get("body")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .chars()
        .take(8000)
        .collect::<String>();
    let html_url = payload
        .get("html_url")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    Ok(Some(GithubReleaseInfo {
        tag,
        notes,
        html_url,
    }))
}

#[cfg(test)]
mod check_error_tests {
    use super::*;
    use reqwest::header::{HeaderMap, HeaderValue};

    #[test]
    fn classify_rate_limit_403_with_remaining_zero() {
        let mut headers = HeaderMap::new();
        headers.insert("x-ratelimit-remaining", HeaderValue::from_static("0"));
        headers.insert("x-ratelimit-reset", HeaderValue::from_static("1700000000"));
        let (code, reset) = classify_github_release_http_error(
            reqwest::StatusCode::FORBIDDEN,
            &headers,
            r#"{"message":"API rate limit exceeded"}"#,
        );
        assert_eq!(code, CHECK_ERR_RATE_LIMIT);
        assert!(reset.is_some());
    }

    #[test]
    fn classify_rate_limit_429() {
        let headers = HeaderMap::new();
        let (code, _) = classify_github_release_http_error(
            reqwest::StatusCode::TOO_MANY_REQUESTS,
            &headers,
            "slow down",
        );
        assert_eq!(code, CHECK_ERR_RATE_LIMIT);
    }

    #[test]
    fn classify_plain_403_as_http() {
        let mut headers = HeaderMap::new();
        headers.insert("x-ratelimit-remaining", HeaderValue::from_static("58"));
        let (code, reset) = classify_github_release_http_error(
            reqwest::StatusCode::FORBIDDEN,
            &headers,
            r#"{"message":"Forbidden"}"#,
        );
        assert_eq!(code, CHECK_ERR_HTTP);
        assert!(reset.is_none());
    }

    #[test]
    fn classify_500_as_http() {
        let (code, _) = classify_github_release_http_error(
            reqwest::StatusCode::INTERNAL_SERVER_ERROR,
            &HeaderMap::new(),
            "oops",
        );
        assert_eq!(code, CHECK_ERR_HTTP);
    }

    #[test]
    fn auto_check_due_respects_rate_limit_reset() {
        let now = Utc::now();
        let reset = now + chrono::Duration::hours(1);
        assert!(!auto_check_due(
            now,
            24,
            Some(now),
            Some(CHECK_ERR_RATE_LIMIT),
            Some(reset),
        ));
        assert!(auto_check_due(
            reset + chrono::Duration::seconds(1),
            24,
            Some(now),
            Some(CHECK_ERR_RATE_LIMIT),
            Some(reset),
        ));
    }

    #[test]
    fn auto_check_due_uses_short_backoff_for_other_errors() {
        let now = Utc::now();
        // min(24, 1) = 1h — still within window
        let last_recent = now - chrono::Duration::minutes(30);
        assert!(!auto_check_due(
            now,
            24,
            Some(last_recent),
            Some(CHECK_ERR_NETWORK),
            None,
        ));
        // past the 1h error backoff
        let last_old = now - chrono::Duration::hours(2);
        assert!(auto_check_due(
            now,
            24,
            Some(last_old),
            Some(CHECK_ERR_HTTP),
            None,
        ));
    }

    #[test]
    fn auto_check_due_success_uses_full_interval() {
        let now = Utc::now();
        let last = now - chrono::Duration::hours(2);
        assert!(!auto_check_due(now, 24, Some(last), None, None));
        assert!(auto_check_due(
            last + chrono::Duration::hours(25),
            24,
            Some(last),
            None,
            None,
        ));
    }

    #[test]
    fn auto_check_due_disabled_when_interval_zero() {
        let now = Utc::now();
        assert!(!auto_check_due(now, 0, None, None, None));
    }

    #[test]
    fn auto_check_due_true_when_never_checked() {
        let now = Utc::now();
        assert!(auto_check_due(now, 24, None, None, None));
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/admin/updates/apply",
    tag = "admin",
    request_body = ApplyUpdateRequest,
    responses((status = 202, body = ApplyUpdateResponse)),
    security(("bearer_auth" = []))
)]
pub async fn apply_update(
    State(state): State<AppState>,
    RequireSystemAdmin(admin): RequireSystemAdmin,
    Json(body): Json<ApplyUpdateRequest>,
) -> AppResult<(axum::http::StatusCode, Json<ApplyUpdateResponse>)> {
    let version = normalize_version(&body.version)?;
    if matches!(capability(), UpdatesCapability::ManualOnly) {
        return Err(AppError::BadRequest(
            "one-click update unavailable; use the manual upgrade command shown in Admin → Updates"
                .into(),
        ));
    }
    if let Some(job) = active_job(&state.pool).await? {
        return Err(AppError::Conflict(format!(
            "upgrade already in progress ({})",
            job.id
        )));
    }
    let base = updater_url().expect("checked");
    let token = updater_token().expect("checked");
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/v1/upgrade", base.trim_end_matches('/')))
        .header("X-Vulyk-Updater-Token", token)
        .json(&serde_json::json!({ "version": version }))
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("updater unreachable: {e}")))?;
    if resp.status() == reqwest::StatusCode::CONFLICT {
        return Err(AppError::Conflict(
            "updater reports an upgrade already running".into(),
        ));
    }
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(AppError::BadRequest(format!(
            "updater rejected upgrade ({status}): {text}"
        )));
    }
    let payload: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| AppError::BadRequest(format!("updater response: {e}")))?;
    let job_id = payload
        .get("job_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("updater missing job_id".into()))?;
    let id = Uuid::parse_str(job_id)
        .map_err(|_| AppError::BadRequest("updater job_id not a uuid".into()))?;
    sqlx::query(
        r#"INSERT INTO update_jobs
            (id, target_version, status, progress, message, log_text, started_by)
           VALUES ($1, $2, 'pending', 0, 'queued', '', $3)"#,
    )
    .bind(id)
    .bind(&version)
    .bind(admin.id())
    .execute(&state.pool)
    .await?;

    // Spawn poller to sync sidecar job → DB
    let pool = state.pool.clone();
    let base = base.clone();
    let token = updater_token().unwrap();
    tokio::spawn(async move {
        if let Err(e) = poll_sidecar_job(pool, base, token, id).await {
            tracing::warn!(error = %e, job_id = %id, "update job poller ended with error");
        }
    });

    let job = load_job(&state.pool, id).await?;
    Ok((
        axum::http::StatusCode::ACCEPTED,
        Json(ApplyUpdateResponse { job }),
    ))
}

async fn poll_sidecar_job(
    pool: sqlx::PgPool,
    base: String,
    token: String,
    id: Uuid,
) -> Result<(), String> {
    let client = reqwest::Client::new();
    let url = format!("{}/v1/jobs/{id}", base.trim_end_matches('/'));
    for _ in 0..900 {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        let resp = client
            .get(&url)
            .header("X-Vulyk-Updater-Token", &token)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            continue;
        }
        let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        let status = body
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("running");
        let progress = body.get("progress").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let message = body
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let log_text = body
            .get("log")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let finished = matches!(status, "succeeded" | "failed" | "rolled_back");
        sqlx::query(
            r#"UPDATE update_jobs SET
                status = $2, progress = $3, message = $4, log_text = $5, updated_at = NOW(),
                finished_at = CASE WHEN $6 THEN NOW() ELSE finished_at END
               WHERE id = $1"#,
        )
        .bind(id)
        .bind(status)
        .bind(progress)
        .bind(&message)
        .bind(&log_text)
        .bind(finished)
        .execute(&pool)
        .await
        .map_err(|e| e.to_string())?;
        if finished {
            return Ok(());
        }
    }
    Err("timed out waiting for updater job".into())
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/updates/jobs",
    tag = "admin",
    responses((status = 200, body = UpdateJobList)),
    security(("bearer_auth" = []))
)]
pub async fn list_jobs(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
) -> AppResult<Json<UpdateJobList>> {
    let items = sqlx::query_as::<_, UpdateJobPublic>(
        r#"SELECT id, target_version, status, progress, message, log_text,
                  created_at, updated_at, finished_at
           FROM update_jobs
           ORDER BY created_at DESC
           LIMIT 50"#,
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(UpdateJobList { items }))
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/updates/jobs/{job_id}",
    tag = "admin",
    params(("job_id" = Uuid, Path)),
    responses((status = 200, body = UpdateJobPublic)),
    security(("bearer_auth" = []))
)]
pub async fn get_job(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
    Path(job_id): Path<Uuid>,
) -> AppResult<Json<UpdateJobPublic>> {
    Ok(Json(load_job(&state.pool, job_id).await?))
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/updates/preferences",
    tag = "admin",
    responses((status = 200, body = UpdatesPreferences)),
    security(("bearer_auth" = []))
)]
pub async fn get_prefs(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
) -> AppResult<Json<UpdatesPreferences>> {
    Ok(Json(UpdatesPreferences {
        check_interval_hours: prefs_interval(&state.pool).await?,
    }))
}

#[utoipa::path(
    put,
    path = "/api/v1/admin/updates/preferences",
    tag = "admin",
    request_body = UpdatePreferencesRequest,
    responses((status = 200, body = UpdatesPreferences)),
    security(("bearer_auth" = []))
)]
pub async fn put_prefs(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
    Json(body): Json<UpdatePreferencesRequest>,
) -> AppResult<Json<UpdatesPreferences>> {
    if let Some(h) = body.check_interval_hours {
        let h = h.clamp(0, 24 * 30);
        settings::set_value(
            &state.pool,
            "updates_check_interval_hours",
            serde_json::json!(h),
        )
        .await?;
    }
    Ok(Json(UpdatesPreferences {
        check_interval_hours: prefs_interval(&state.pool).await?,
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/updates/version-backups",
    tag = "admin",
    responses((status = 200, body = VersionBackupList)),
    security(("bearer_auth" = []))
)]
pub async fn list_version_backups(
    State(_state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
) -> AppResult<Json<VersionBackupList>> {
    let current = db::crate_version();
    let list = fetch_version_backups_from_sidecar(current)
        .await?
        .unwrap_or_else(|| VersionBackupList {
            items: vec![],
            rollback: None,
            retention: default_retention(),
        });
    Ok(Json(list))
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/updates/version-backups/{backup_id}/download",
    tag = "admin",
    params(("backup_id" = String, Path)),
    responses((status = 200, description = "gzip tarball of the version backup bundle")),
    security(("bearer_auth" = []))
)]
pub async fn download_version_backup(
    State(_state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
    Path(backup_id): Path<String>,
) -> AppResult<axum::response::Response> {
    if !backup_id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
        || !backup_id.starts_with("vb-")
    {
        return Err(AppError::BadRequest("invalid backup id".into()));
    }
    let (Some(base), Some(token)) = (updater_url(), updater_token()) else {
        return Err(AppError::BadRequest(
            "version backup download requires the Compose updater sidecar".into(),
        ));
    };
    let client = reqwest::Client::new();
    let url = format!(
        "{}/v1/version-backups/{backup_id}/download",
        base.trim_end_matches('/')
    );
    let resp = client
        .get(&url)
        .header("X-Vulyk-Updater-Token", token)
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("updater unreachable: {e}")))?;
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(AppError::NotFound("version backup".into()));
    }
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(AppError::BadRequest(format!(
            "updater download failed ({status}): {text}"
        )));
    }
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| AppError::Internal(format!("download body: {e}")))?;
    let mut response = axum::response::Response::new(axum::body::Body::from(bytes));
    *response.status_mut() = axum::http::StatusCode::OK;
    response.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("application/gzip"),
    );
    response.headers_mut().insert(
        axum::http::header::CONTENT_DISPOSITION,
        axum::http::HeaderValue::from_str(&format!("attachment; filename=\"{backup_id}.tar.gz\""))
            .unwrap_or_else(|_| {
                axum::http::HeaderValue::from_static("attachment; filename=\"backup.tar.gz\"")
            }),
    );
    Ok(response)
}

#[utoipa::path(
    post,
    path = "/api/v1/admin/updates/rollback",
    tag = "admin",
    request_body = ApplyRollbackRequest,
    responses((status = 202, body = ApplyUpdateResponse)),
    security(("bearer_auth" = []))
)]
pub async fn apply_rollback(
    State(state): State<AppState>,
    RequireSystemAdmin(admin): RequireSystemAdmin,
    Json(body): Json<ApplyRollbackRequest>,
) -> AppResult<(axum::http::StatusCode, Json<ApplyUpdateResponse>)> {
    if body.confirm != "ROLLBACK" {
        return Err(AppError::BadRequest(
            "confirm must be the literal string ROLLBACK (data created after the upgrade will be lost)"
                .into(),
        ));
    }
    if matches!(capability(), UpdatesCapability::ManualOnly) {
        return Err(AppError::BadRequest(
            "one-click rollback unavailable; use the rollback command shown in Admin → Updates"
                .into(),
        ));
    }
    if let Some(job) = active_job(&state.pool).await? {
        return Err(AppError::Conflict(format!(
            "update/rollback already in progress ({})",
            job.id
        )));
    }
    let version = body
        .version
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(normalize_version)
        .transpose()?;
    let base = updater_url().expect("checked");
    let token = updater_token().expect("checked");
    let client = reqwest::Client::new();
    let mut payload = serde_json::json!({ "confirm": "ROLLBACK" });
    if let Some(ref v) = version {
        payload["version"] = serde_json::Value::String(v.clone());
    }
    let resp = client
        .post(format!("{}/v1/rollback", base.trim_end_matches('/')))
        .header("X-Vulyk-Updater-Token", token)
        .json(&payload)
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("updater unreachable: {e}")))?;
    if resp.status() == reqwest::StatusCode::CONFLICT {
        return Err(AppError::Conflict(
            "updater reports a job already running".into(),
        ));
    }
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(AppError::BadRequest(format!(
            "updater rejected rollback ({status}): {text}"
        )));
    }
    let payload: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| AppError::BadRequest(format!("updater response: {e}")))?;
    let job_id = payload
        .get("job_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::BadRequest("updater missing job_id".into()))?;
    let id = Uuid::parse_str(job_id)
        .map_err(|_| AppError::BadRequest("updater job_id not a uuid".into()))?;
    let target = version
        .clone()
        .unwrap_or_else(|| "active-rollback".to_string());
    sqlx::query(
        r#"INSERT INTO update_jobs
            (id, target_version, status, progress, message, log_text, started_by)
           VALUES ($1, $2, 'pending', 0, 'rollback queued', '', $3)"#,
    )
    .bind(id)
    .bind(&target)
    .bind(admin.id())
    .execute(&state.pool)
    .await?;

    write_audit(
        &state.pool,
        Some(admin.id()),
        "instance.rollback",
        "update_job",
        &id.to_string(),
        serde_json::json!({
            "target_version": target,
            "confirm": "ROLLBACK",
        }),
    )
    .await?;

    let pool = state.pool.clone();
    let base = base.clone();
    let token = updater_token().unwrap();
    tokio::spawn(async move {
        if let Err(e) = poll_sidecar_job(pool, base, token, id).await {
            tracing::warn!(error = %e, job_id = %id, "rollback job poller ended with error");
        }
    });

    let job = load_job(&state.pool, id).await?;
    Ok((
        axum::http::StatusCode::ACCEPTED,
        Json(ApplyUpdateResponse { job }),
    ))
}
