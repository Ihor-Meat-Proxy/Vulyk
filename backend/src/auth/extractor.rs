use axum::{
    body::Body,
    extract::{FromRequestParts, State},
    http::{header::AUTHORIZATION, request::Parts, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use axum_extra::extract::CookieJar;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use super::jwt::decode_jwt;
use super::rbac::Permission;
use super::tokens::hash_secret;
use crate::error::{AppError, AppResult};
use crate::models::User;
use crate::routes::AppState;

const SESSION_COOKIE: &str = "vulyk_session";

#[derive(Clone, Debug)]
pub struct AuthConfig {
    /// When true, protected routes reject anonymous requests.
    pub required: bool,
    pub jwt_secret: String,
    pub jwt_ttl_hours: i64,
    pub session_ttl_hours: i64,
}

impl AuthConfig {
    pub fn from_env() -> Self {
        Self {
            required: require_auth_enabled(),
            jwt_secret: std::env::var("VULYK_JWT_SECRET").unwrap_or_else(|_| {
                tracing::warn!("VULYK_JWT_SECRET unset; using insecure default for local/dev");
                "vulyk-dev-jwt-secret-change-me".into()
            }),
            jwt_ttl_hours: std::env::var("VULYK_JWT_TTL_HOURS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(12),
            session_ttl_hours: std::env::var("VULYK_SESSION_TTL_HOURS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(168),
        }
    }
}

/// Accepts `VULYK_AUTH_REQUIRED` (W1) or legacy `VULYK_REQUIRE_AUTH` (W9/W10).
pub fn require_auth_enabled() -> bool {
    for key in ["VULYK_AUTH_REQUIRED", "VULYK_REQUIRE_AUTH"] {
        if let Ok(v) = std::env::var(key) {
            return matches!(v.to_lowercase().as_str(), "1" | "true" | "yes");
        }
    }
    false
}

#[derive(Clone, Debug)]
pub struct AuthUser {
    pub user: User,
    pub via: AuthVia,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthVia {
    Jwt,
    Session,
    ApiToken { token_id: Uuid },
}

/// Retained for W9/W10 middleware consumers / extensions.
#[derive(Debug, Clone)]
pub struct AuthContext {
    pub token_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
}

/// Optional auth: succeeds with `None` when anonymous is allowed.
pub struct OptionalAuthUser(pub Option<AuthUser>);

/// Requires a logged-in user.
pub struct RequireAuth(pub AuthUser);

/// Requires system admin.
pub struct RequireSystemAdmin(pub AuthUser);

impl AuthUser {
    pub fn id(&self) -> Uuid {
        self.user.id
    }

    pub fn is_system_admin(&self) -> bool {
        self.user.is_system_admin
    }

    pub fn must_change_password(&self) -> bool {
        self.user.must_change_password
    }
}

impl FromRequestParts<AppState> for OptionalAuthUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        match resolve_auth(parts, state).await {
            Ok(user) => Ok(Self(Some(user))),
            Err(AppError::Unauthorized(_)) => Ok(Self(None)),
            Err(other) => Err(other),
        }
    }
}

impl FromRequestParts<AppState> for RequireAuth {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        resolve_auth(parts, state).await.map(Self)
    }
}

impl FromRequestParts<AppState> for RequireSystemAdmin {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let RequireAuth(user) = RequireAuth::from_request_parts(parts, state).await?;
        if !user.is_system_admin() {
            return Err(AppError::Forbidden(
                "system administrator role required".into(),
            ));
        }
        Ok(Self(user))
    }
}

/// Layer used by `routes::app_router`. Accepts JWT, session, or API token.
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

    // Session cookie path: allow anonymous when auth not required.
    if header.is_none() {
        let jar = CookieJar::from_headers(req.headers());
        if let Some(cookie) = jar.get(SESSION_COOKIE) {
            match authenticate_session(&state.pool, cookie.value()).await {
                Ok(user) => {
                    req.extensions_mut().insert(AuthContext {
                        token_id: None,
                        user_id: Some(user.id()),
                    });
                    req.extensions_mut().insert(user);
                    return next.run(req).await;
                }
                Err(_) => {
                    if state.auth.required {
                        return unauthorized("invalid or expired session");
                    }
                }
            }
        } else if state.auth.required {
            return unauthorized("missing bearer token");
        }
        req.extensions_mut().insert(AuthContext {
            token_id: None,
            user_id: None,
        });
        return next.run(req).await;
    }

    let h = header.unwrap();
    let Some(token) = h
        .strip_prefix("Bearer ")
        .or_else(|| h.strip_prefix("bearer "))
        .map(str::trim)
    else {
        return unauthorized("expected Bearer token");
    };
    if token.is_empty() {
        return unauthorized("empty token");
    }

    match authenticate_bearer(&state.pool, &state.auth.jwt_secret, token).await {
        Ok(user) => {
            let token_id = match user.via {
                AuthVia::ApiToken { token_id } => Some(token_id),
                _ => None,
            };
            req.extensions_mut().insert(AuthContext {
                token_id,
                user_id: Some(user.id()),
            });
            req.extensions_mut().insert(user);
            next.run(req).await
        }
        Err(AppError::Unauthorized(msg)) => unauthorized(&msg),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": { "code": "internal", "message": e.to_string() } })),
        )
            .into_response(),
    }
}

fn unauthorized(message: &str) -> Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({ "error": { "code": "unauthorized", "message": message } })),
    )
        .into_response()
}

async fn resolve_auth(parts: &mut Parts, state: &AppState) -> AppResult<AuthUser> {
    if let Some(user) = parts.extensions.get::<AuthUser>() {
        return Ok(user.clone());
    }
    if let Some(raw) = bearer_token(parts) {
        return authenticate_bearer(&state.pool, &state.auth.jwt_secret, &raw).await;
    }
    let jar = CookieJar::from_headers(&parts.headers);
    if let Some(cookie) = jar.get(SESSION_COOKIE) {
        return authenticate_session(&state.pool, cookie.value()).await;
    }
    Err(AppError::Unauthorized("missing credentials".into()))
}

fn bearer_token(parts: &Parts) -> Option<String> {
    let value = parts.headers.get(AUTHORIZATION)?.to_str().ok()?;
    let token = value
        .strip_prefix("Bearer ")
        .or_else(|| value.strip_prefix("bearer "))?;
    let token = token.trim();
    if token.is_empty() {
        None
    } else {
        Some(token.to_string())
    }
}

async fn authenticate_bearer(pool: &PgPool, jwt_secret: &str, token: &str) -> AppResult<AuthUser> {
    if token.starts_with("vulyk_at_") || token.starts_with("vulk_") {
        return authenticate_api_token(pool, token).await;
    }
    if token.starts_with("vulyk_s_") {
        return authenticate_session(pool, token).await;
    }

    // Try JWT first; fall back to API token hash for opaque secrets.
    match decode_jwt(token, jwt_secret) {
        Ok(claims) => {
            let user_id: Uuid = claims
                .sub
                .parse()
                .map_err(|_| AppError::Unauthorized("invalid token subject".into()))?;
            let user = load_active_user(pool, user_id).await?;
            Ok(AuthUser {
                user,
                via: AuthVia::Jwt,
            })
        }
        Err(_) => authenticate_api_token(pool, token).await,
    }
}

async fn authenticate_session(pool: &PgPool, raw_token: &str) -> AppResult<AuthUser> {
    let token_hash = hash_secret(raw_token);
    let row = sqlx::query_as::<_, (Uuid, Uuid)>(
        r#"SELECT id, user_id FROM sessions
           WHERE token_hash = $1 AND expires_at > NOW()"#,
    )
    .bind(&token_hash)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::Unauthorized("invalid or expired session".into()))?;

    let (session_id, user_id) = row;
    sqlx::query("UPDATE sessions SET last_seen_at = NOW() WHERE id = $1")
        .bind(session_id)
        .execute(pool)
        .await?;

    let user = load_active_user(pool, user_id).await?;
    Ok(AuthUser {
        user,
        via: AuthVia::Session,
    })
}

async fn authenticate_api_token(pool: &PgPool, raw_token: &str) -> AppResult<AuthUser> {
    let token_hash = hash_secret(raw_token);
    let row = sqlx::query_as::<_, (Uuid, Option<Uuid>)>(
        r#"SELECT id, user_id FROM api_tokens
           WHERE token_hash = $1
             AND revoked_at IS NULL
             AND (expires_at IS NULL OR expires_at > NOW())"#,
    )
    .bind(&token_hash)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::Unauthorized("invalid or revoked API token".into()))?;

    let (token_id, user_id) = row;
    sqlx::query("UPDATE api_tokens SET last_used_at = NOW() WHERE id = $1")
        .bind(token_id)
        .execute(pool)
        .await?;

    // Legacy W9/W10 tokens may have no user_id — synthesize a system principal.
    if let Some(user_id) = user_id {
        let user = load_active_user(pool, user_id).await?;
        return Ok(AuthUser {
            user,
            via: AuthVia::ApiToken { token_id },
        });
    }

    // Headless token: treat as system admin for MCP until rebound to a user.
    Ok(AuthUser {
        user: User {
            id: Uuid::nil(),
            email: "api-token@vulyk.local".into(),
            display_name: "API token".into(),
            password_hash: None,
            is_system_admin: true,
            must_change_password: false,
            locale: None,
            timezone: None,
            disabled_at: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        },
        via: AuthVia::ApiToken { token_id },
    })
}

async fn load_active_user(pool: &PgPool, user_id: Uuid) -> AppResult<User> {
    let user = sqlx::query_as::<_, User>(
        r#"SELECT id, email, display_name, password_hash, is_system_admin,
                  must_change_password, locale, timezone,
                  disabled_at, created_at, updated_at
           FROM users WHERE id = $1"#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::Unauthorized("user not found".into()))?;

    if user.disabled_at.is_some() {
        return Err(AppError::Forbidden("user account is disabled".into()));
    }
    Ok(user)
}

/// Helper used by routes to enforce project role when auth is active.
/// Prefer [`require_permission`] for the new matrix.
pub async fn require_project_role(
    pool: &PgPool,
    auth: &Option<AuthUser>,
    auth_required: bool,
    project_id: Uuid,
    min_role: super::ProjectRole,
) -> AppResult<()> {
    // Map legacy "at least role" to a representative permission for compatibility.
    let permission = match min_role {
        super::ProjectRole::Viewer => Permission::ViewProject,
        super::ProjectRole::Developer => Permission::ExecuteRuns,
        super::ProjectRole::TestAutomationEngineer => Permission::CreateEditCases,
        super::ProjectRole::TestArchitect => Permission::ManagePlans,
        super::ProjectRole::Admin => Permission::DeleteProject,
    };
    require_permission(pool, auth, auth_required, project_id, permission).await
}

/// Enforce a fine-grained project permission.
pub async fn require_permission(
    pool: &PgPool,
    auth: &Option<AuthUser>,
    auth_required: bool,
    project_id: Uuid,
    permission: Permission,
) -> AppResult<()> {
    if permission.is_system_only() {
        return match auth {
            Some(user) if user.is_system_admin() => Ok(()),
            Some(_) => Err(AppError::Forbidden(format!(
                "requires system admin ({})",
                permission.as_str()
            ))),
            None if auth_required => Err(AppError::Unauthorized("authentication required".into())),
            None => Ok(()),
        };
    }

    match auth {
        Some(user) if user.is_system_admin() => Ok(()),
        Some(user) => {
            let role = sqlx::query_scalar::<_, super::ProjectRole>(
                r#"SELECT role FROM memberships
                   WHERE project_id = $1 AND user_id = $2"#,
            )
            .bind(project_id)
            .bind(user.id())
            .fetch_optional(pool)
            .await?;

            match role {
                Some(r) if r.allows(permission) => Ok(()),
                Some(_) => Err(AppError::Forbidden(format!(
                    "missing permission: {}",
                    permission.as_str()
                ))),
                None => Err(AppError::Forbidden("not a member of this project".into())),
            }
        }
        None if auth_required => Err(AppError::Unauthorized("authentication required".into())),
        None => Ok(()),
    }
}

/// Load the caller's project role (if any).
pub async fn project_role_for(
    pool: &PgPool,
    user_id: Uuid,
    project_id: Uuid,
) -> AppResult<Option<super::ProjectRole>> {
    let role = sqlx::query_scalar::<_, super::ProjectRole>(
        r#"SELECT role FROM memberships
           WHERE project_id = $1 AND user_id = $2"#,
    )
    .bind(project_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(role)
}

pub fn session_cookie_name() -> &'static str {
    SESSION_COOKIE
}
