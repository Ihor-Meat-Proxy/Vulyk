//! Outbound project webhooks: HMAC-signed delivery with job-queue retries.

use std::time::Duration;

use chrono::Utc;
use hmac::{Hmac, KeyInit, Mac};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::Sha256;
use sqlx::PgPool;
use uuid::Uuid;

use crate::crypto::SecretsKey;
use crate::error::{AppError, AppResult};
use crate::models::OutboundWebhookDelivery;
use crate::services::egress;
use crate::services::jobs as jobs_svc;

type HmacSha256 = Hmac<Sha256>;

pub const EVENT_CASE_CREATED: &str = "case.created";
pub const EVENT_CASE_UPDATED: &str = "case.updated";
pub const EVENT_CASE_DELETED: &str = "case.deleted";
pub const EVENT_RUN_CREATED: &str = "run.created";
pub const EVENT_RUN_UPDATED: &str = "run.updated";
pub const EVENT_RESULT_CREATED: &str = "result.created";
pub const EVENT_WEBHOOK_TEST: &str = "webhook.test";

pub const SUBSCRIBABLE_EVENTS: &[&str] = &[
    EVENT_CASE_CREATED,
    EVENT_CASE_UPDATED,
    EVENT_CASE_DELETED,
    EVENT_RUN_CREATED,
    EVENT_RUN_UPDATED,
    EVENT_RESULT_CREATED,
    EVENT_WEBHOOK_TEST,
];

const USER_AGENT: &str = "Vulyk-Webhooks/0.6";
const HTTP_TIMEOUT: Duration = Duration::from_secs(15);
const RESPONSE_BODY_MAX: usize = 4096;

pub fn generate_secret() -> String {
    crate::services::automation_repo::generate_webhook_secret()
}

pub fn sign_body(secret: &str, body: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("hmac key");
    mac.update(body);
    format!("sha256={}", hex::encode(mac.finalize().into_bytes()))
}

#[derive(Serialize)]
struct WebhookEnvelope<'a> {
    id: Uuid,
    event: &'a str,
    created_at: chrono::DateTime<Utc>,
    project_id: Uuid,
    data: &'a Value,
}

struct DeliveryEndpointRow {
    delivery: OutboundWebhookDelivery,
    url: String,
    secret_ciphertext: Vec<u8>,
    secret_nonce: Vec<u8>,
    enabled: bool,
}

/// Best-effort fan-out to all enabled endpoints subscribed to `event_type`.
pub async fn enqueue_event(pool: &PgPool, project_id: Uuid, event_type: &str, data: Value) {
    let endpoints: Vec<(Uuid,)> = match sqlx::query_as(
        r#"SELECT id FROM outbound_webhook_endpoints
           WHERE project_id = $1 AND enabled = true AND events @> ARRAY[$2]::text[]"#,
    )
    .bind(project_id)
    .bind(event_type)
    .fetch_all(pool)
    .await
    {
        Ok(rows) => rows,
        Err(e) => {
            tracing::warn!(
                error = %e,
                %project_id,
                event = event_type,
                "outbound webhook: list endpoints failed"
            );
            return;
        }
    };

    for (endpoint_id,) in endpoints {
        if let Err(e) =
            enqueue_delivery(pool, endpoint_id, project_id, event_type, data.clone()).await
        {
            tracing::warn!(
                error = %e,
                %project_id,
                %endpoint_id,
                event = event_type,
                "outbound webhook: enqueue delivery failed"
            );
        }
    }
}

pub async fn enqueue_test(pool: &PgPool, endpoint_id: Uuid, project_id: Uuid) -> AppResult<Uuid> {
    let data = json!({ "message": "Vulyk webhook test" });
    enqueue_delivery(pool, endpoint_id, project_id, EVENT_WEBHOOK_TEST, data).await
}

async fn enqueue_delivery(
    pool: &PgPool,
    endpoint_id: Uuid,
    project_id: Uuid,
    event_type: &str,
    data: Value,
) -> AppResult<Uuid> {
    let delivery = sqlx::query_as::<_, OutboundWebhookDelivery>(
        r#"INSERT INTO outbound_webhook_deliveries
             (endpoint_id, project_id, event_type, payload, status)
           VALUES ($1, $2, $3, $4, 'pending')
           RETURNING id, endpoint_id, project_id, event_type, payload, status, attempt_count,
                     http_status, response_body, last_error, job_id, created_at, updated_at, delivered_at"#,
    )
    .bind(endpoint_id)
    .bind(project_id)
    .bind(event_type)
    .bind(data)
    .fetch_one(pool)
    .await?;

    match jobs_svc::enqueue(
        pool,
        "outbound_webhook_deliver",
        json!({ "delivery_id": delivery.id }),
        8,
    )
    .await
    {
        Ok(job) => {
            let _ = sqlx::query(
                "UPDATE outbound_webhook_deliveries SET job_id = $2, updated_at = NOW() WHERE id = $1",
            )
            .bind(delivery.id)
            .bind(job.id)
            .execute(pool)
            .await;
        }
        Err(e) => {
            tracing::warn!(
                error = %e,
                delivery_id = %delivery.id,
                "outbound webhook: job enqueue failed"
            );
        }
    }
    Ok(delivery.id)
}

pub async fn redeliver(pool: &PgPool, delivery_id: Uuid) -> AppResult<OutboundWebhookDelivery> {
    let existing = sqlx::query_as::<_, OutboundWebhookDelivery>(
        r#"SELECT id, endpoint_id, project_id, event_type, payload, status, attempt_count,
                  http_status, response_body, last_error, job_id, created_at, updated_at, delivered_at
           FROM outbound_webhook_deliveries WHERE id = $1"#,
    )
    .bind(delivery_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("delivery".into()))?;

    let new_id = enqueue_delivery(
        pool,
        existing.endpoint_id,
        existing.project_id,
        &existing.event_type,
        existing.payload.clone(),
    )
    .await?;

    sqlx::query_as::<_, OutboundWebhookDelivery>(
        r#"SELECT id, endpoint_id, project_id, event_type, payload, status, attempt_count,
                  http_status, response_body, last_error, job_id, created_at, updated_at, delivered_at
           FROM outbound_webhook_deliveries WHERE id = $1"#,
    )
    .bind(new_id)
    .fetch_one(pool)
    .await
    .map_err(Into::into)
}

pub async fn deliver_one(
    pool: &PgPool,
    secrets: &SecretsKey,
    job_id: Uuid,
    delivery_id: Uuid,
) -> Result<(), String> {
    let row = load_delivery_endpoint(pool, delivery_id)
        .await
        .map_err(|e| e.to_string())?;

    if row.delivery.status == "succeeded" {
        return Ok(());
    }
    if !row.enabled {
        mark_failed(pool, delivery_id, None, None, "endpoint disabled")
            .await
            .map_err(|e| e.to_string())?;
        return Ok(());
    }

    let _ = sqlx::query(
        r#"UPDATE outbound_webhook_deliveries
           SET attempt_count = attempt_count + 1, job_id = $2, updated_at = NOW()
           WHERE id = $1"#,
    )
    .bind(delivery_id)
    .bind(job_id)
    .execute(pool)
    .await;

    let secret = secrets
        .decrypt(&row.secret_ciphertext, &row.secret_nonce)
        .map_err(|e| format!("decrypt secret: {e}"))?;

    let policy = crate::settings::load_ci_url_policy(pool)
        .await
        .map_err(|e| e.to_string())?;
    if let Err(e) = egress::validate_ci_url(&row.url, &policy) {
        mark_failed(pool, delivery_id, None, None, &e.to_string())
            .await
            .map_err(|e| e.to_string())?;
        return Ok(());
    }

    let envelope = WebhookEnvelope {
        id: delivery_id,
        event: &row.delivery.event_type,
        created_at: row.delivery.created_at,
        project_id: row.delivery.project_id,
        data: &row.delivery.payload,
    };
    let body = serde_json::to_vec(&envelope).map_err(|e| e.to_string())?;
    let signature = sign_body(&secret, &body);

    let client = reqwest::Client::builder()
        .timeout(HTTP_TIMEOUT)
        .build()
        .map_err(|e| e.to_string())?;

    let res = client
        .post(&row.url)
        .header("Content-Type", "application/json")
        .header("X-Vulyk-Signature", signature)
        .header("X-Vulyk-Event", &row.delivery.event_type)
        .header("X-Vulyk-Delivery", delivery_id.to_string())
        .header(reqwest::header::USER_AGENT, USER_AGENT)
        .body(body)
        .send()
        .await;

    match res {
        Ok(resp) => {
            let status = resp.status();
            let http_status = status.as_u16() as i32;
            let text = resp.text().await.unwrap_or_default();
            let truncated = truncate_body(&text);

            if status.is_success() {
                sqlx::query(
                    r#"UPDATE outbound_webhook_deliveries
                       SET status = 'succeeded', http_status = $2, response_body = $3,
                           last_error = NULL, delivered_at = NOW(), updated_at = NOW()
                       WHERE id = $1"#,
                )
                .bind(delivery_id)
                .bind(http_status)
                .bind(truncated)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
                Ok(())
            } else if status.is_client_error() && status.as_u16() != 429 {
                mark_failed(
                    pool,
                    delivery_id,
                    Some(http_status),
                    Some(truncated),
                    "non-retryable HTTP response",
                )
                .await
                .map_err(|e| e.to_string())?;
                Ok(())
            } else {
                let _ = sqlx::query(
                    r#"UPDATE outbound_webhook_deliveries
                       SET http_status = $2, response_body = $3,
                           last_error = $4, updated_at = NOW()
                       WHERE id = $1"#,
                )
                .bind(delivery_id)
                .bind(http_status)
                .bind(truncated)
                .bind(format!("HTTP {http_status}"))
                .execute(pool)
                .await;
                Err(format!("retryable HTTP {http_status}"))
            }
        }
        Err(e) => {
            let msg = format!("request failed: {e}");
            let _ = sqlx::query(
                r#"UPDATE outbound_webhook_deliveries
                   SET last_error = $2, updated_at = NOW()
                   WHERE id = $1"#,
            )
            .bind(delivery_id)
            .bind(&msg)
            .execute(pool)
            .await;
            Err(msg)
        }
    }
}

async fn load_delivery_endpoint(
    pool: &PgPool,
    delivery_id: Uuid,
) -> AppResult<DeliveryEndpointRow> {
    let row = sqlx::query_as::<_, OutboundWebhookDelivery>(
        r#"SELECT id, endpoint_id, project_id, event_type, payload, status, attempt_count,
                  http_status, response_body, last_error, job_id, created_at, updated_at, delivered_at
           FROM outbound_webhook_deliveries WHERE id = $1"#,
    )
    .bind(delivery_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("delivery".into()))?;

    let endpoint: Option<(String, Vec<u8>, Vec<u8>, bool)> = sqlx::query_as(
        r#"SELECT url, secret_ciphertext, secret_nonce, enabled
           FROM outbound_webhook_endpoints WHERE id = $1"#,
    )
    .bind(row.endpoint_id)
    .fetch_optional(pool)
    .await?;

    let Some((url, secret_ciphertext, secret_nonce, enabled)) = endpoint else {
        return Err(AppError::NotFound("endpoint".into()));
    };

    Ok(DeliveryEndpointRow {
        delivery: row,
        url,
        secret_ciphertext,
        secret_nonce,
        enabled,
    })
}

async fn mark_failed(
    pool: &PgPool,
    delivery_id: Uuid,
    http_status: Option<i32>,
    response_body: Option<String>,
    last_error: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE outbound_webhook_deliveries
           SET status = 'failed', http_status = $2, response_body = $3,
               last_error = $4, updated_at = NOW()
           WHERE id = $1"#,
    )
    .bind(delivery_id)
    .bind(http_status)
    .bind(response_body)
    .bind(last_error)
    .execute(pool)
    .await?;
    Ok(())
}

fn truncate_body(text: &str) -> String {
    if text.len() <= RESPONSE_BODY_MAX {
        return text.to_string();
    }
    let mut end = RESPONSE_BODY_MAX;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}
