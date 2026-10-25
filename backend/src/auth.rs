//! Optional Bearer API-token auth for MCP and automation clients.
//!
//! Phase 1 stays open for local UI use. When `VULYK_REQUIRE_AUTH=1`, missing
//! Authorization headers are rejected. When a Bearer token is present it must
//! always be valid (so MCP misconfiguration fails loudly).

use axum::{
    body::Body,
    extract::State,
    http::{header::AUTHORIZATION, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use uuid::Uuid;

use crate::crypto::hash_token;
use crate::routes::AppState;

pub fn require_auth_enabled() -> bool {
    std::env::var("VULYK_REQUIRE_AUTH")
        .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes"))
        .unwrap_or(false)
}

pub async fn bearer_middleware(
    State(state): State<AppState>,
    mut req: Request<Body>,
    next: Next,
) -> Response {
    let header = req
        .headers()
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);

    match header {
        None => {
            if require_auth_enabled() {
                return unauthorized("missing bearer token");
            }
            req.extensions_mut().insert(AuthContext { token_id: None });
            next.run(req).await
        }
        Some(h) => {
            let Some(token) = h.strip_prefix("Bearer ").map(str::trim) else {
                return unauthorized("expected Bearer token");
            };
            if token.is_empty() {
                return unauthorized("empty token");
            }
            let hash = hash_token(token);
            let row: Result<Option<(Uuid,)>, sqlx::Error> = sqlx::query_as(
                r#"SELECT id FROM api_tokens
                   WHERE token_hash = $1 AND revoked_at IS NULL"#,
            )
            .bind(&hash)
            .fetch_optional(&state.pool)
            .await;

            match row {
                Ok(Some((id,))) => {
                    let _ = sqlx::query("UPDATE api_tokens SET last_used_at = NOW() WHERE id = $1")
                        .bind(id)
                        .execute(&state.pool)
                        .await;
                    req.extensions_mut()
                        .insert(AuthContext { token_id: Some(id) });
                    next.run(req).await
                }
                Ok(None) => unauthorized("invalid API token"),
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": { "code": "internal", "message": e.to_string() } })),
                )
                    .into_response(),
            }
        }
    }
}

fn unauthorized(message: &str) -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({ "error": { "code": "unauthorized", "message": message } })),
    )
        .into_response()
}

#[derive(Debug, Clone)]
pub struct AuthContext {
    pub token_id: Option<Uuid>,
}
