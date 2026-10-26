use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use uuid::Uuid;

use crate::auth::{
    generate_api_token, hash_secret, AuthUser, OptionalAuthUser, Permission, ProjectRole,
};
use crate::error::{AppError, AppResult};
use crate::models::{ApiTokenPublic, CreateApiToken, CreateApiTokenResponse};
use crate::routes::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/tokens", get(list_tokens).post(create_token))
        .route("/api/v1/tokens/{id}", axum::routing::delete(revoke_token))
}

/// When auth is required: system admin, or any membership with ManageApiTokens.
/// Viewers (and others without that permission on any project) get 403.
async fn ensure_can_manage_api_tokens(state: &AppState, auth: &Option<AuthUser>) -> AppResult<()> {
    if !state.auth.required {
        return Ok(());
    }
    let Some(user) = auth.as_ref().filter(|u| u.id() != Uuid::nil()) else {
        return Err(AppError::Unauthorized("authentication required".into()));
    };
    if user.is_system_admin() {
        return Ok(());
    }
    let roles =
        sqlx::query_scalar::<_, ProjectRole>(r#"SELECT role FROM memberships WHERE user_id = $1"#)
            .bind(user.id())
            .fetch_all(&state.pool)
            .await?;
    if roles
        .iter()
        .any(|role| role.allows(Permission::ManageApiTokens))
    {
        return Ok(());
    }
    Err(AppError::Forbidden(
        "missing permission: manage_api_tokens".into(),
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/tokens",
    tag = "tokens",
    responses((status = 200, description = "List API tokens", body = [ApiTokenPublic])),
    security(("bearer_auth" = []))
)]
pub async fn list_tokens(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
) -> AppResult<Json<Vec<ApiTokenPublic>>> {
    ensure_can_manage_api_tokens(&state, &auth).await?;
    let rows = if let Some(user) = auth.filter(|u| u.id() != Uuid::nil()) {
        if user.is_system_admin() {
            // Admins may list unbound legacy tokens so they can revoke them.
            sqlx::query_as::<_, ApiTokenPublic>(
                r#"SELECT id, name, token_prefix, created_at, last_used_at, revoked_at
                   FROM api_tokens
                   WHERE user_id = $1 OR user_id IS NULL
                   ORDER BY created_at DESC"#,
            )
            .bind(user.id())
            .fetch_all(&state.pool)
            .await?
        } else {
            sqlx::query_as::<_, ApiTokenPublic>(
                r#"SELECT id, name, token_prefix, created_at, last_used_at, revoked_at
                   FROM api_tokens
                   WHERE user_id = $1
                   ORDER BY created_at DESC"#,
            )
            .bind(user.id())
            .fetch_all(&state.pool)
            .await?
        }
    } else {
        // Open mode (auth not required): list all tokens.
        sqlx::query_as::<_, ApiTokenPublic>(
            r#"SELECT id, name, token_prefix, created_at, last_used_at, revoked_at
               FROM api_tokens ORDER BY created_at DESC"#,
        )
        .fetch_all(&state.pool)
        .await?
    };
    Ok(Json(rows))
}

#[utoipa::path(
    post,
    path = "/api/v1/tokens",
    tag = "tokens",
    request_body = CreateApiToken,
    responses((status = 200, description = "Created token (secret shown once)", body = CreateApiTokenResponse)),
    security(("bearer_auth" = []))
)]
pub async fn create_token(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Json(body): Json<CreateApiToken>,
) -> AppResult<Json<CreateApiTokenResponse>> {
    ensure_can_manage_api_tokens(&state, &auth).await?;
    if body.name.trim().is_empty() {
        return Err(AppError::BadRequest("name required".into()));
    }
    // Tokens must be bound to a real user — unbound (headless) tokens are rejected
    // at authentication time and must not be created.
    let user_id = auth
        .as_ref()
        .filter(|u| u.id() != Uuid::nil())
        .map(|u| u.id())
        .ok_or_else(|| {
            AppError::Unauthorized(
                "authentication required to create an API token bound to a user".into(),
            )
        })?;
    let (secret, prefix) = generate_api_token();
    let hash = hash_secret(&secret);

    let row = sqlx::query_as::<_, ApiTokenPublic>(
        r#"INSERT INTO api_tokens (name, token_prefix, token_hash, user_id)
           VALUES ($1, $2, $3, $4)
           RETURNING id, name, token_prefix, created_at, last_used_at, revoked_at"#,
    )
    .bind(body.name.trim())
    .bind(&prefix)
    .bind(&hash)
    .bind(user_id)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(CreateApiTokenResponse { token: row, secret }))
}

#[utoipa::path(
    delete,
    path = "/api/v1/tokens/{id}",
    tag = "tokens",
    params(("id" = Uuid, Path, description = "Token id")),
    responses((status = 204, description = "Revoked")),
    security(("bearer_auth" = []))
)]
pub async fn revoke_token(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusNoContent> {
    ensure_can_manage_api_tokens(&state, &auth).await?;
    let res = if let Some(user) = auth.filter(|u| u.id() != Uuid::nil()) {
        // Only system admins may revoke unbound legacy tokens.
        let q = if user.is_system_admin() {
            sqlx::query(
                r#"UPDATE api_tokens SET revoked_at = NOW()
                   WHERE id = $1 AND revoked_at IS NULL
                     AND (user_id = $2 OR user_id IS NULL)"#,
            )
            .bind(id)
            .bind(user.id())
        } else {
            sqlx::query(
                r#"UPDATE api_tokens SET revoked_at = NOW()
                   WHERE id = $1 AND revoked_at IS NULL AND user_id = $2"#,
            )
            .bind(id)
            .bind(user.id())
        };
        q.execute(&state.pool).await?
    } else {
        sqlx::query(
            r#"UPDATE api_tokens SET revoked_at = NOW()
               WHERE id = $1 AND revoked_at IS NULL"#,
        )
        .bind(id)
        .execute(&state.pool)
        .await?
    };
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
