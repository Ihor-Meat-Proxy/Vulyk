use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::post,
    Json, Router,
};
use serde_json::Value;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::integrations::{github, gitlab};
use crate::models::{CiConfig, CiDispatch, WebhookAck};
use crate::routes::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/webhooks/github", post(github_webhook))
        .route(
            "/api/v1/webhooks/github/{config_id}",
            post(github_webhook_for_config),
        )
        .route("/api/v1/webhooks/gitlab", post(gitlab_webhook))
        .route(
            "/api/v1/webhooks/gitlab/{config_id}",
            post(gitlab_webhook_for_config),
        )
        .route(
            "/api/v1/webhooks/automation-repos/{repo_id}",
            post(automation_repo_webhook),
        )
}

#[utoipa::path(
    post,
    path = "/api/v1/webhooks/github",
    tag = "webhooks",
    request_body(content = String, description = "GitHub webhook JSON payload", content_type = "application/json"),
    responses(
        (status = 200, description = "Webhook accepted", body = WebhookAck),
        (status = 401, description = "Invalid signature")
    )
)]
pub async fn github_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> AppResult<Json<WebhookAck>> {
    handle_github(&state, None, &headers, &body).await
}

#[utoipa::path(
    post,
    path = "/api/v1/webhooks/github/{config_id}",
    tag = "webhooks",
    params(("config_id" = Uuid, Path, description = "CI config id")),
    request_body(content = String, description = "GitHub webhook JSON payload", content_type = "application/json"),
    responses(
        (status = 200, description = "Webhook accepted", body = WebhookAck),
        (status = 401, description = "Invalid signature")
    )
)]
pub async fn github_webhook_for_config(
    State(state): State<AppState>,
    Path(config_id): Path<Uuid>,
    headers: HeaderMap,
    body: Bytes,
) -> AppResult<Json<WebhookAck>> {
    handle_github(&state, Some(config_id), &headers, &body).await
}

#[utoipa::path(
    post,
    path = "/api/v1/webhooks/gitlab",
    tag = "webhooks",
    request_body(content = String, description = "GitLab webhook JSON payload", content_type = "application/json"),
    responses(
        (status = 200, description = "Webhook accepted", body = WebhookAck),
        (status = 401, description = "Invalid token")
    )
)]
pub async fn gitlab_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> AppResult<Json<WebhookAck>> {
    handle_gitlab(&state, None, &headers, &body).await
}

#[utoipa::path(
    post,
    path = "/api/v1/webhooks/gitlab/{config_id}",
    tag = "webhooks",
    params(("config_id" = Uuid, Path, description = "CI config id")),
    request_body(content = String, description = "GitLab webhook JSON payload", content_type = "application/json"),
    responses(
        (status = 200, description = "Webhook accepted", body = WebhookAck),
        (status = 401, description = "Invalid token")
    )
)]
pub async fn gitlab_webhook_for_config(
    State(state): State<AppState>,
    Path(config_id): Path<Uuid>,
    headers: HeaderMap,
    body: Bytes,
) -> AppResult<Json<WebhookAck>> {
    handle_gitlab(&state, Some(config_id), &headers, &body).await
}

async fn handle_github(
    state: &AppState,
    config_id: Option<Uuid>,
    headers: &HeaderMap,
    body: &Bytes,
) -> AppResult<Json<WebhookAck>> {
    let event = header_str(headers, "x-github-event").unwrap_or_default();
    let delivery = header_str(headers, "x-github-delivery");
    let signature = header_str(headers, "x-hub-signature-256").unwrap_or_default();

    let payload: Value = serde_json::from_slice(body)
        .map_err(|e| AppError::BadRequest(format!("invalid JSON: {e}")))?;

    let configs = load_configs(state, "github", config_id).await?;
    if configs.is_empty() {
        return Ok(Json(WebhookAck {
            received: true,
            dispatch_id: None,
            status: None,
        }));
    }

    // Fail closed: only configs whose webhook_secret verifies the signature are
    // authorized. Missing secrets never grant access (previously fail-open).
    let matched: Vec<CiConfig> = configs
        .into_iter()
        .filter(|c| {
            c.webhook_secret
                .as_deref()
                .is_some_and(|secret| github::verify_signature(secret, body, &signature))
        })
        .collect();
    if matched.is_empty() {
        return Err(AppError::Unauthorized("invalid GitHub signature".into()));
    }

    let mut dispatch_id = None;
    let mut status = None;

    if event == "workflow_run" {
        if let Some((external_id, mapped, url)) = github::status_from_workflow_run(&payload) {
            if let Some(d) = update_dispatch_status(
                state,
                "github",
                &external_id,
                &mapped,
                url.as_deref(),
                &matched,
            )
            .await?
            {
                dispatch_id = Some(d.id);
                status = Some(d.status);
            }
        }
    }

    log_webhook(
        state,
        "github",
        &event,
        delivery.as_deref(),
        &payload,
        dispatch_id,
    )
    .await?;

    Ok(Json(WebhookAck {
        received: true,
        dispatch_id,
        status,
    }))
}

async fn handle_gitlab(
    state: &AppState,
    config_id: Option<Uuid>,
    headers: &HeaderMap,
    body: &Bytes,
) -> AppResult<Json<WebhookAck>> {
    let token = header_str(headers, "x-gitlab-token");
    let event = header_str(headers, "x-gitlab-event").unwrap_or_default();

    let payload: Value = serde_json::from_slice(body)
        .map_err(|e| AppError::BadRequest(format!("invalid JSON: {e}")))?;

    let configs = load_configs(state, "gitlab", config_id).await?;
    if configs.is_empty() {
        return Ok(Json(WebhookAck {
            received: true,
            dispatch_id: None,
            status: None,
        }));
    }

    // Fail closed: missing webhook_secret must not authorize the request.
    let matched: Vec<CiConfig> = configs
        .into_iter()
        .filter(|c| {
            c.webhook_secret
                .as_deref()
                .is_some_and(|secret| gitlab::verify_token(secret, token.as_deref()))
        })
        .collect();
    if matched.is_empty() {
        return Err(AppError::Unauthorized("invalid GitLab token".into()));
    }

    let mut dispatch_id = None;
    let mut status = None;

    if event.to_lowercase().contains("pipeline")
        || payload.get("object_kind").and_then(|v| v.as_str()) == Some("pipeline")
    {
        if let Some((external_id, mapped, url)) = gitlab::status_from_pipeline(&payload) {
            if let Some(d) = update_dispatch_status(
                state,
                "gitlab",
                &external_id,
                &mapped,
                url.as_deref(),
                &matched,
            )
            .await?
            {
                dispatch_id = Some(d.id);
                status = Some(d.status);
            }
        }
    }

    log_webhook(state, "gitlab", &event, None, &payload, dispatch_id).await?;

    Ok(Json(WebhookAck {
        received: true,
        dispatch_id,
        status,
    }))
}

async fn load_configs(
    state: &AppState,
    provider: &str,
    config_id: Option<Uuid>,
) -> AppResult<Vec<CiConfig>> {
    if let Some(id) = config_id {
        let row = sqlx::query_as::<_, CiConfig>(
            r#"SELECT id, project_id, provider, name, config, webhook_secret, enabled,
                      created_at, updated_at
               FROM ci_configs WHERE id = $1 AND provider = $2 AND enabled = TRUE"#,
        )
        .bind(id)
        .bind(provider)
        .fetch_optional(&state.pool)
        .await?;
        return Ok(row.into_iter().collect());
    }
    let rows = sqlx::query_as::<_, CiConfig>(
        r#"SELECT id, project_id, provider, name, config, webhook_secret, enabled,
                  created_at, updated_at
           FROM ci_configs WHERE provider = $1 AND enabled = TRUE"#,
    )
    .bind(provider)
    .fetch_all(&state.pool)
    .await?;
    Ok(rows)
}

async fn update_dispatch_status(
    state: &AppState,
    provider: &str,
    external_id: &str,
    status: &str,
    url: Option<&str>,
    configs: &[CiConfig],
) -> AppResult<Option<CiDispatch>> {
    let config_ids: Vec<Uuid> = configs.iter().map(|c| c.id).collect();

    // Prefer exact external_id match.
    if let Some(row) = sqlx::query_as::<_, CiDispatch>(
        r#"UPDATE ci_dispatches d
           SET status = $2,
               external_url = COALESCE($3, external_url),
               updated_at = NOW()
           FROM ci_configs c
           WHERE d.ci_config_id = c.id
             AND c.provider = $4
             AND d.external_id = $1
           RETURNING d.id, d.project_id, d.ci_config_id, d.run_id, d.status, d.external_id,
                     d.external_url, d.inputs, d.last_error, d.created_at, d.updated_at"#,
    )
    .bind(external_id)
    .bind(status)
    .bind(url)
    .bind(provider)
    .fetch_optional(&state.pool)
    .await?
    {
        sync_run_status(state, &row).await?;
        return Ok(Some(row));
    }

    // GitHub workflow_dispatch often has no run id yet — attach to newest open dispatch.
    if let Some(row) = sqlx::query_as::<_, CiDispatch>(
        r#"UPDATE ci_dispatches d
           SET status = $2,
               external_id = COALESCE(d.external_id, $3),
               external_url = COALESCE($4, d.external_url),
               updated_at = NOW()
           WHERE d.id = (
               SELECT d2.id FROM ci_dispatches d2
               WHERE d2.ci_config_id = ANY($1)
                 AND d2.status IN ('queued', 'dispatched', 'running')
               ORDER BY d2.created_at DESC
               LIMIT 1
           )
           RETURNING d.id, d.project_id, d.ci_config_id, d.run_id, d.status, d.external_id,
                     d.external_url, d.inputs, d.last_error, d.created_at, d.updated_at"#,
    )
    .bind(&config_ids)
    .bind(status)
    .bind(external_id)
    .bind(url)
    .fetch_optional(&state.pool)
    .await?
    {
        sync_run_status(state, &row).await?;
        return Ok(Some(row));
    }

    let _ = StatusCode::OK;
    Ok(None)
}

async fn sync_run_status(state: &AppState, dispatch: &CiDispatch) -> AppResult<()> {
    let Some(run_id) = dispatch.run_id else {
        return Ok(());
    };
    let run_status = match dispatch.status.as_str() {
        "succeeded" => "completed",
        "failed" | "cancelled" => "failed",
        "running" => "running",
        _ => return Ok(()),
    };
    sqlx::query(
        r#"UPDATE test_runs
           SET status = $2,
               external_ref = COALESCE($3, external_ref),
               pipeline_url = COALESCE($4, pipeline_url),
               finished_at = CASE WHEN $2 IN ('completed', 'failed') THEN COALESCE(finished_at, NOW()) ELSE finished_at END,
               updated_at = NOW()
           WHERE id = $1"#,
    )
    .bind(run_id)
    .bind(run_status)
    .bind(&dispatch.external_id)
    .bind(&dispatch.external_url)
    .execute(&state.pool)
    .await?;
    Ok(())
}

async fn log_webhook(
    state: &AppState,
    provider: &str,
    event_type: &str,
    delivery_id: Option<&str>,
    payload: &Value,
    ci_dispatch_id: Option<Uuid>,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO webhook_events (provider, event_type, delivery_id, payload, ci_dispatch_id, processed)
           VALUES ($1, $2, $3, $4, $5, TRUE)"#,
    )
    .bind(provider)
    .bind(event_type)
    .bind(delivery_id)
    .bind(payload)
    .bind(ci_dispatch_id)
    .execute(&state.pool)
    .await?;
    Ok(())
}

fn header_str(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

#[utoipa::path(
    post,
    path = "/api/v1/webhooks/automation-repos/{repo_id}",
    tag = "webhooks",
    params(("repo_id" = Uuid, Path, description = "Automation repo id")),
    request_body(content = String, description = "GitHub push or GitLab Push Hook JSON", content_type = "application/json"),
    responses(
        (status = 200, description = "Webhook accepted", body = WebhookAck),
        (status = 401, description = "Invalid signature or token")
    )
)]
pub async fn automation_repo_webhook(
    State(state): State<AppState>,
    Path(repo_id): Path<Uuid>,
    headers: HeaderMap,
    body: Bytes,
) -> AppResult<Json<WebhookAck>> {
    let row = crate::services::automation_repo::load_repo_by_id(&state.pool, repo_id)
        .await?
        .ok_or_else(|| AppError::NotFound("automation repo".into()))?;

    if row.webhook_secret.is_empty() {
        return Err(AppError::Unauthorized(
            "automation repo webhook secret not configured".into(),
        ));
    }

    let provider = row.provider.to_lowercase();
    let gh_event = header_str(&headers, "x-github-event");
    let gl_event = header_str(&headers, "x-gitlab-event");
    let gl_token = header_str(&headers, "x-gitlab-token");
    let gh_sig = header_str(&headers, "x-hub-signature-256").unwrap_or_default();

    let verified = if gh_event.is_some() || provider == "github" {
        github::verify_signature(&row.webhook_secret, &body, &gh_sig)
    } else if gl_event.is_some() || provider == "gitlab" {
        gitlab::verify_token(&row.webhook_secret, gl_token.as_deref())
    } else {
        false
    };
    if !verified {
        return Err(AppError::Unauthorized(
            "invalid automation-repo webhook credentials".into(),
        ));
    }

    let payload: Value = serde_json::from_slice(&body)
        .map_err(|e| AppError::BadRequest(format!("invalid JSON: {e}")))?;

    let event_label = gh_event
        .clone()
        .or(gl_event.clone())
        .unwrap_or_else(|| "push".into());

    let should_reindex = branch_moved_for_repo(&row.branch, &provider, &event_label, &payload);
    if should_reindex {
        // Fire-and-forget so the webhook returns quickly; skip if already indexing.
        let pool = state.pool.clone();
        let secrets = state.secrets.clone();
        let row_clone = row.clone();
        tokio::spawn(async move {
            match crate::services::automation_repo_index::run_reindex(
                &pool, &secrets, &row_clone, "webhook",
            )
            .await
            {
                Ok(res) if res.status == "ready" => {
                    tracing::info!(repo_id = %row_clone.id, "automation-repo webhook reindex ready");
                }
                Ok(res) => {
                    tracing::info!(
                        repo_id = %row_clone.id,
                        status = %res.status,
                        error = ?res.error,
                        "automation-repo webhook reindex finished"
                    );
                }
                Err(e) => {
                    tracing::warn!(repo_id = %row_clone.id, error = %e, "automation-repo webhook reindex failed");
                }
            }
        });
    }

    let _ = log_webhook(
        &state,
        "automation_repo",
        &event_label,
        header_str(&headers, "x-github-delivery").as_deref(),
        &payload,
        None,
    )
    .await;

    Ok(Json(WebhookAck {
        received: true,
        dispatch_id: None,
        status: if should_reindex {
            Some("reindex_queued".into())
        } else {
            Some("ignored".into())
        },
    }))
}

fn branch_moved_for_repo(configured: &str, provider: &str, event: &str, payload: &Value) -> bool {
    let configured = configured.trim();
    if provider == "github" || event == "push" {
        if event != "push" && provider == "github" {
            return false;
        }
        // GitHub: ref = "refs/heads/main"
        if let Some(r) = payload.get("ref").and_then(|v| v.as_str()) {
            let branch = r.strip_prefix("refs/heads/").unwrap_or(r);
            return branch == configured;
        }
    }
    if provider == "gitlab"
        || event.to_lowercase().contains("push")
        || payload.get("object_kind").and_then(|v| v.as_str()) == Some("push")
    {
        // GitLab: ref = "refs/heads/main" or checkout_sha present
        if let Some(r) = payload.get("ref").and_then(|v| v.as_str()) {
            let branch = r.strip_prefix("refs/heads/").unwrap_or(r);
            return branch == configured;
        }
    }
    false
}
