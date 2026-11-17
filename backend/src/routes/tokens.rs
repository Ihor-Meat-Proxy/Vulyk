use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use uuid::Uuid;

use crate::auth::{
    generate_api_token, hash_secret, require_permission, AuthUser, OptionalAuthUser, Permission,
};
use crate::error::{AppError, AppResult};
use crate::models::{ApiTokenPublic, CreateApiToken, CreateApiTokenResponse};
use crate::routes::AppState;

const TOKEN_COLS: &str = "id, name, token_prefix, created_at, last_used_at, revoked_at, \
                          project_id, permissions";

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/tokens", get(list_tokens).post(create_token))
        .route("/api/v1/tokens/{id}", axum::routing::delete(revoke_token))
}

/// When auth is required: system admin (session/JWT), or membership with ManageApiTokens
/// on the target project. Viewers (and others without that permission) get 403.
async fn ensure_can_manage_api_tokens(state: &AppState, auth: &Option<AuthUser>) -> AppResult<()> {
    if !state.auth.required {
        return Ok(());
    }
    let Some(user) = auth.as_ref().filter(|u| u.id() != Uuid::nil()) else {
        return Err(AppError::Unauthorized("authentication required".into()));
    };
    if user.bypasses_as_system_admin() {
        return Ok(());
    }
    // Any project membership with ManageApiTokens is enough to open the tokens UI /
    // list own tokens; create further checks the selected project.
    let roles = sqlx::query_scalar::<_, crate::auth::ProjectRole>(
        r#"SELECT role FROM memberships WHERE user_id = $1"#,
    )
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

fn parse_token_permissions(raw: &[String]) -> AppResult<Vec<Permission>> {
    if raw.is_empty() {
        return Err(AppError::BadRequest(
            "permissions required (non-empty list of project permission strings)".into(),
        ));
    }
    let mut out = Vec::with_capacity(raw.len());
    let mut seen = std::collections::HashSet::new();
    for s in raw {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            continue;
        }
        let perm: Permission = trimmed
            .parse()
            .map_err(|e: String| AppError::BadRequest(e))?;
        if perm.is_system_only() {
            return Err(AppError::BadRequest(format!(
                "permission is system-only and cannot be granted to an API token: {}",
                perm.as_str()
            )));
        }
        if seen.insert(perm.as_str()) {
            out.push(perm);
        }
    }
    if out.is_empty() {
        return Err(AppError::BadRequest(
            "permissions required (non-empty list of project permission strings)".into(),
        ));
    }
    Ok(out)
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
        if user.bypasses_as_system_admin() {
            // Admins may list unbound legacy tokens so they can revoke them.
            sqlx::query_as::<_, ApiTokenPublic>(sqlx::AssertSqlSafe(format!(
                r#"SELECT {TOKEN_COLS}
                   FROM api_tokens
                   WHERE user_id = $1 OR user_id IS NULL
                   ORDER BY created_at DESC"#
            )))
            .bind(user.id())
            .fetch_all(&state.pool)
            .await?
        } else {
            sqlx::query_as::<_, ApiTokenPublic>(sqlx::AssertSqlSafe(format!(
                r#"SELECT {TOKEN_COLS}
                   FROM api_tokens
                   WHERE user_id = $1
                   ORDER BY created_at DESC"#
            )))
            .bind(user.id())
            .fetch_all(&state.pool)
            .await?
        }
    } else {
        // Open mode (auth not required): list all tokens.
        sqlx::query_as::<_, ApiTokenPublic>(sqlx::AssertSqlSafe(format!(
            r#"SELECT {TOKEN_COLS}
               FROM api_tokens ORDER BY created_at DESC"#
        )))
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
    if body.name.trim().is_empty() {
        return Err(AppError::BadRequest("name required".into()));
    }
    let permissions = parse_token_permissions(&body.permissions)?;
    let permission_strs: Vec<String> = permissions.iter().map(|p| p.as_str().to_string()).collect();

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

    // ManageApiTokens on the target project (or unscoped system admin).
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        body.project_id,
        Permission::ManageApiTokens,
    )
    .await?;

    let project_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM projects WHERE id = $1)")
            .bind(body.project_id)
            .fetch_one(&state.pool)
            .await?;
    if !project_exists {
        return Err(AppError::NotFound("project".into()));
    }

    let (secret, prefix) = generate_api_token();
    let hash = hash_secret(&secret);

    let row = sqlx::query_as::<_, ApiTokenPublic>(sqlx::AssertSqlSafe(format!(
        r#"INSERT INTO api_tokens (name, token_prefix, token_hash, user_id, project_id, permissions)
           VALUES ($1, $2, $3, $4, $5, $6)
           RETURNING {TOKEN_COLS}"#
    )))
    .bind(body.name.trim())
    .bind(&prefix)
    .bind(&hash)
    .bind(user_id)
    .bind(body.project_id)
    .bind(&permission_strs)
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
        // Only unscoped system admins may revoke unbound legacy tokens.
        let q = if user.bypasses_as_system_admin() {
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
