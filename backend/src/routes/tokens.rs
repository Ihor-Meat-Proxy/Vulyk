use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use rand::RngCore;
use uuid::Uuid;

use crate::crypto::hash_token;
use crate::error::{AppError, AppResult};
use crate::models::{ApiTokenPublic, CreateApiToken, CreateApiTokenResponse};
use crate::routes::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/tokens", get(list_tokens).post(create_token))
        .route("/api/v1/tokens/{id}", axum::routing::delete(revoke_token))
}

#[utoipa::path(
    get,
    path = "/api/v1/tokens",
    tag = "tokens",
    responses((status = 200, description = "List API tokens", body = [ApiTokenPublic]))
)]
pub async fn list_tokens(State(state): State<AppState>) -> AppResult<Json<Vec<ApiTokenPublic>>> {
    let rows = sqlx::query_as::<_, ApiTokenPublic>(
        r#"SELECT id, name, token_prefix, created_at, last_used_at, revoked_at
           FROM api_tokens ORDER BY created_at DESC"#,
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

#[utoipa::path(
    post,
    path = "/api/v1/tokens",
    tag = "tokens",
    request_body = CreateApiToken,
    responses((status = 200, description = "Created token (secret shown once)", body = CreateApiTokenResponse))
)]
pub async fn create_token(
    State(state): State<AppState>,
    Json(body): Json<CreateApiToken>,
) -> AppResult<Json<CreateApiTokenResponse>> {
    if body.name.trim().is_empty() {
        return Err(AppError::BadRequest("name required".into()));
    }
    let secret = mint_secret();
    let prefix: String = secret.chars().take(12).collect();
    let hash = hash_token(&secret);

    let row = sqlx::query_as::<_, ApiTokenPublic>(
        r#"INSERT INTO api_tokens (name, token_prefix, token_hash)
           VALUES ($1, $2, $3)
           RETURNING id, name, token_prefix, created_at, last_used_at, revoked_at"#,
    )
    .bind(body.name.trim())
    .bind(&prefix)
    .bind(&hash)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(CreateApiTokenResponse { token: row, secret }))
}

#[utoipa::path(
    delete,
    path = "/api/v1/tokens/{id}",
    tag = "tokens",
    params(("id" = Uuid, Path, description = "Token id")),
    responses((status = 204, description = "Revoked"))
)]
pub async fn revoke_token(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<StatusNoContent> {
    let res = sqlx::query(
        r#"UPDATE api_tokens SET revoked_at = NOW()
           WHERE id = $1 AND revoked_at IS NULL"#,
    )
    .bind(id)
    .execute(&state.pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound("token".into()));
    }
    Ok(StatusNoContent)
}

pub struct StatusNoContent;

impl axum::response::IntoResponse for StatusNoContent {
    fn into_response(self) -> axum::response::Response {
        axum::http::StatusCode::NO_CONTENT.into_response()
    }
}

fn mint_secret() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    format!("vulk_{}", hex::encode(bytes))
}
