//! Admin Updates: check GitHub Releases and apply via the updater sidecar.

use axum::{
    extract::{Path, State},
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
        .route("/api/v1/admin/updates/jobs", get(list_jobs))
        .route("/api/v1/admin/updates/jobs/{job_id}", get(get_job))
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
pub struct UpdatesStatus {
    pub current_version: &'static str,
    pub latest_version: Option<String>,
    pub latest_notes: Option<String>,
    pub latest_url: Option<String>,
    pub update_available: bool,
    pub last_checked_at: Option<String>,
    pub check_interval_hours: i64,
    pub capability: UpdatesCapability,
    pub manual_command: String,
    pub github_repo: String,
    pub active_job: Option<UpdateJobPublic>,
}

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
    let interval = prefs_interval(&state.pool).await?;
    let latest_clean = if latest.is_empty() {
        None
    } else {
        Some(latest)
    };
    let update_available = latest_clean
        .as_ref()
        .map(|l| semver_gt(l, current))
        .unwrap_or(false);
    let target = latest_clean.clone().unwrap_or_else(|| current.to_string());
    Ok(UpdatesStatus {
        current_version: current,
        latest_version: latest_clean,
        latest_notes: if notes.is_empty() { None } else { Some(notes) },
        latest_url: if url.is_empty() { None } else { Some(url) },
        update_available,
        last_checked_at: if last.is_empty() { None } else { Some(last) },
        check_interval_hours: interval,
        capability: capability(),
        manual_command: manual_command(&target),
        github_repo: github_repo(),
        active_job: active_job(&state.pool).await?,
    })
}

#[utoipa::path(
    post,
    path = "/api/v1/admin/updates/check",
    tag = "admin",
    responses((status = 200, body = UpdatesStatus)),
    security(("bearer_auth" = []))
)]
pub async fn check_updates(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
) -> AppResult<Json<UpdatesStatus>> {
    let repo = github_repo();
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .header("User-Agent", "vulyk-api-updates")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| AppError::BadRequest(format!("github releases request failed: {e}")))?;
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        set_string_setting(
            &state.pool,
            "updates_last_checked_at",
            &Utc::now().to_rfc3339(),
        )
        .await?;
        return Ok(Json(build_status(&state).await?));
    }
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::BadRequest(format!(
            "github releases HTTP {status}: {body}"
        )));
    }
    let payload: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| AppError::BadRequest(format!("github releases json: {e}")))?;
    let tag = payload
        .get("tag_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim_start_matches('v');
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
    set_string_setting(&state.pool, "updates_latest_version", tag).await?;
    set_string_setting(&state.pool, "updates_latest_notes", &notes).await?;
    set_string_setting(&state.pool, "updates_latest_url", &html_url).await?;
    set_string_setting(
        &state.pool,
        "updates_last_checked_at",
        &Utc::now().to_rfc3339(),
    )
    .await?;
    Ok(Json(build_status(&state).await?))
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
