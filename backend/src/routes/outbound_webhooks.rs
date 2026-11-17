//! Project outbound webhooks (signed delivery to operator URLs).

use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::{require_permission, OptionalAuthUser, Permission};
use crate::error::{AppError, AppResult};
use crate::models::{
    CreateOutboundWebhook, CreateOutboundWebhookResponse, OutboundWebhookDeliveryPublic,
    OutboundWebhookEndpoint, OutboundWebhookEndpointPublic, Page, UpdateOutboundWebhook,
};
use crate::routes::cases::ensure_project;
use crate::routes::AppState;
use crate::services::outbound_webhooks::{
    enqueue_test, generate_secret, redeliver, SUBSCRIBABLE_EVENTS,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/projects/{project_id}/outbound-webhooks",
            get(list_endpoints).post(create_endpoint),
        )
        .route(
            "/api/v1/projects/{project_id}/outbound-webhooks/deliveries/{delivery_id}/redeliver",
            post(redeliver_delivery),
        )
        .route(
            "/api/v1/projects/{project_id}/outbound-webhooks/{id}/test",
            post(test_endpoint),
        )
        .route(
            "/api/v1/projects/{project_id}/outbound-webhooks/{id}/deliveries",
            get(list_deliveries),
        )
        .route(
            "/api/v1/projects/{project_id}/outbound-webhooks/{id}",
            get(get_endpoint)
                .put(update_endpoint)
                .delete(delete_endpoint),
        )
}

fn validate_events(events: &[String]) -> AppResult<()> {
    if events.is_empty() {
        return Err(AppError::BadRequest("events required".into()));
    }
    for e in events {
        if !SUBSCRIBABLE_EVENTS.contains(&e.as_str()) {
            return Err(AppError::BadRequest(format!("unknown event: {e}")));
        }
    }
    Ok(())
}

async fn validate_webhook_url(state: &AppState, url: &str) -> AppResult<()> {
    let policy = crate::settings::load_ci_url_policy(&state.pool).await?;
    crate::services::egress::validate_ci_url(url.trim(), &policy)
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/outbound-webhooks",
    tag = "outbound-webhooks",
    params(("project_id" = Uuid, Path)),
    responses((status = 200, body = [OutboundWebhookEndpointPublic])),
    security(("bearer_auth" = []))
)]
pub async fn list_endpoints(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
) -> AppResult<Json<Vec<OutboundWebhookEndpointPublic>>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ManageCiConfig,
    )
    .await?;
    ensure_project(&state.pool, project_id).await?;
    let rows = sqlx::query_as::<_, OutboundWebhookEndpoint>(
        r#"SELECT id, project_id, name, url, secret_ciphertext, secret_nonce, events, enabled,
                  created_at, updated_at
           FROM outbound_webhook_endpoints
           WHERE project_id = $1
           ORDER BY created_at DESC"#,
    )
    .bind(project_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(OutboundWebhookEndpointPublic::from)
            .collect(),
    ))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/outbound-webhooks",
    tag = "outbound-webhooks",
    params(("project_id" = Uuid, Path)),
    request_body = CreateOutboundWebhook,
    responses((status = 200, body = CreateOutboundWebhookResponse)),
    security(("bearer_auth" = []))
)]
pub async fn create_endpoint(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<CreateOutboundWebhook>,
) -> AppResult<Json<CreateOutboundWebhookResponse>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ManageCiConfig,
    )
    .await?;
    ensure_project(&state.pool, project_id).await?;
    if body.name.trim().is_empty() {
        return Err(AppError::BadRequest("name required".into()));
    }
    validate_events(&body.events)?;
    validate_webhook_url(&state, &body.url).await?;

    let secret = generate_secret();
    let (ct, nonce) = state.secrets.encrypt(&secret)?;

    let row = sqlx::query_as::<_, OutboundWebhookEndpoint>(
        r#"INSERT INTO outbound_webhook_endpoints
             (project_id, name, url, secret_ciphertext, secret_nonce, events, enabled)
           VALUES ($1, $2, $3, $4, $5, $6, $7)
           RETURNING id, project_id, name, url, secret_ciphertext, secret_nonce, events, enabled,
                     created_at, updated_at"#,
    )
    .bind(project_id)
    .bind(body.name.trim())
    .bind(body.url.trim())
    .bind(ct)
    .bind(nonce)
    .bind(&body.events)
    .bind(body.enabled)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(CreateOutboundWebhookResponse {
        endpoint: row.into(),
        secret,
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/outbound-webhooks/{id}",
    tag = "outbound-webhooks",
    params(("project_id" = Uuid, Path), ("id" = Uuid, Path)),
    responses((status = 200, body = OutboundWebhookEndpointPublic)),
    security(("bearer_auth" = []))
)]
pub async fn get_endpoint(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<OutboundWebhookEndpointPublic>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ManageCiConfig,
    )
    .await?;
    let row = load_endpoint(&state.pool, project_id, id).await?;
    Ok(Json(row.into()))
}

#[utoipa::path(
    put,
    path = "/api/v1/projects/{project_id}/outbound-webhooks/{id}",
    tag = "outbound-webhooks",
    params(("project_id" = Uuid, Path), ("id" = Uuid, Path)),
    request_body = UpdateOutboundWebhook,
    responses((status = 200, body = CreateOutboundWebhookResponse)),
    security(("bearer_auth" = []))
)]
pub async fn update_endpoint(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, id)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateOutboundWebhook>,
) -> AppResult<Json<CreateOutboundWebhookResponse>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ManageCiConfig,
    )
    .await?;
    let existing = load_endpoint(&state.pool, project_id, id).await?;

    let name = body
        .name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or(existing.name);
    let url = if let Some(u) = body.url.as_deref() {
        validate_webhook_url(&state, u).await?;
        u.trim().to_string()
    } else {
        existing.url
    };
    let events = body.events.unwrap_or(existing.events);
    validate_events(&events)?;
    let enabled = body.enabled.unwrap_or(existing.enabled);

    let (secret_plain, ct, nonce) = if body.rotate_secret {
        let secret = generate_secret();
        let (c, n) = state.secrets.encrypt(&secret)?;
        (Some(secret), c, n)
    } else {
        (None, existing.secret_ciphertext, existing.secret_nonce)
    };

    let row = sqlx::query_as::<_, OutboundWebhookEndpoint>(
        r#"UPDATE outbound_webhook_endpoints
           SET name = $3, url = $4, secret_ciphertext = $5, secret_nonce = $6,
               events = $7, enabled = $8, updated_at = NOW()
           WHERE project_id = $1 AND id = $2
           RETURNING id, project_id, name, url, secret_ciphertext, secret_nonce, events, enabled,
                     created_at, updated_at"#,
    )
    .bind(project_id)
    .bind(id)
    .bind(name)
    .bind(url)
    .bind(ct)
    .bind(nonce)
    .bind(events)
    .bind(enabled)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(CreateOutboundWebhookResponse {
        endpoint: row.into(),
        secret: secret_plain.unwrap_or_default(),
    }))
}

#[utoipa::path(
    delete,
    path = "/api/v1/projects/{project_id}/outbound-webhooks/{id}",
    tag = "outbound-webhooks",
    params(("project_id" = Uuid, Path), ("id" = Uuid, Path)),
    responses((status = 204, description = "Deleted")),
    security(("bearer_auth" = []))
)]
pub async fn delete_endpoint(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, id)): Path<(Uuid, Uuid)>,
) -> AppResult<axum::http::StatusCode> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ManageCiConfig,
    )
    .await?;
    let res =
        sqlx::query("DELETE FROM outbound_webhook_endpoints WHERE project_id = $1 AND id = $2")
            .bind(project_id)
            .bind(id)
            .execute(&state.pool)
            .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound("outbound webhook".into()));
    }
    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
pub struct DeliveryListQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 {
    50
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/outbound-webhooks/{id}/deliveries",
    tag = "outbound-webhooks",
    params(
        ("project_id" = Uuid, Path),
        ("id" = Uuid, Path),
        ("limit" = i64, Query),
        ("offset" = i64, Query),
    ),
    responses((status = 200, body = Page<OutboundWebhookDeliveryPublic>)),
    security(("bearer_auth" = []))
)]
pub async fn list_deliveries(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, id)): Path<(Uuid, Uuid)>,
    Query(q): Query<DeliveryListQuery>,
) -> AppResult<Json<Page<OutboundWebhookDeliveryPublic>>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    let _ = load_endpoint(&state.pool, project_id, id).await?;

    let limit = q.limit.clamp(1, 200);
    let offset = q.offset.max(0);

    let total: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*) FROM outbound_webhook_deliveries
           WHERE project_id = $1 AND endpoint_id = $2"#,
    )
    .bind(project_id)
    .bind(id)
    .fetch_one(&state.pool)
    .await?;

    let rows = sqlx::query_as::<_, crate::models::OutboundWebhookDelivery>(
        r#"SELECT id, endpoint_id, project_id, event_type, payload, status, attempt_count,
                  http_status, response_body, last_error, job_id, created_at, updated_at, delivered_at
           FROM outbound_webhook_deliveries
           WHERE project_id = $1 AND endpoint_id = $2
           ORDER BY created_at DESC
           LIMIT $3 OFFSET $4"#,
    )
    .bind(project_id)
    .bind(id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.pool)
    .await?;

    let items = rows
        .into_iter()
        .map(OutboundWebhookDeliveryPublic::from)
        .collect();
    Ok(Json(Page::new(items, total, limit, offset)))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/outbound-webhooks/deliveries/{delivery_id}/redeliver",
    tag = "outbound-webhooks",
    params(("project_id" = Uuid, Path), ("delivery_id" = Uuid, Path)),
    responses((status = 200, body = OutboundWebhookDeliveryPublic)),
    security(("bearer_auth" = []))
)]
pub async fn redeliver_delivery(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, delivery_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<OutboundWebhookDeliveryPublic>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ManageCiConfig,
    )
    .await?;
    let existing_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM outbound_webhook_deliveries WHERE id = $1 AND project_id = $2",
    )
    .bind(delivery_id)
    .bind(project_id)
    .fetch_optional(&state.pool)
    .await?;
    if existing_id.is_none() {
        return Err(AppError::NotFound("delivery".into()));
    }
    let delivery = redeliver(&state.pool, delivery_id).await?;
    Ok(Json(delivery.into()))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/outbound-webhooks/{id}/test",
    tag = "outbound-webhooks",
    params(("project_id" = Uuid, Path), ("id" = Uuid, Path)),
    responses((status = 200, body = OutboundWebhookDeliveryPublic)),
    security(("bearer_auth" = []))
)]
pub async fn test_endpoint(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<OutboundWebhookDeliveryPublic>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ManageCiConfig,
    )
    .await?;
    let _ = load_endpoint(&state.pool, project_id, id).await?;
    let delivery_id = enqueue_test(&state.pool, id, project_id).await?;
    let row = sqlx::query_as::<_, crate::models::OutboundWebhookDelivery>(
        r#"SELECT id, endpoint_id, project_id, event_type, payload, status, attempt_count,
                  http_status, response_body, last_error, job_id, created_at, updated_at, delivered_at
           FROM outbound_webhook_deliveries WHERE id = $1"#,
    )
    .bind(delivery_id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(row.into()))
}

async fn load_endpoint(
    pool: &sqlx::PgPool,
    project_id: Uuid,
    id: Uuid,
) -> AppResult<OutboundWebhookEndpoint> {
    sqlx::query_as::<_, OutboundWebhookEndpoint>(
        r#"SELECT id, project_id, name, url, secret_ciphertext, secret_nonce, events, enabled,
                  created_at, updated_at
           FROM outbound_webhook_endpoints WHERE project_id = $1 AND id = $2"#,
    )
    .bind(project_id)
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("outbound webhook".into()))
}
