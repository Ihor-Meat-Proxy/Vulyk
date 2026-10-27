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

const INSECURE_JWT_DEFAULT: &str = "vulyk-dev-jwt-secret-change-me";

impl AuthConfig {
    pub fn from_env() -> Self {
        let required = require_auth_enabled();
        let jwt_secret = match std::env::var("VULYK_JWT_SECRET") {
            Ok(s) if !s.is_empty() && s != INSECURE_JWT_DEFAULT => s,
            Ok(_) | Err(_) if required => {
                panic!(
                    "VULYK_JWT_SECRET must be set to a non-default value when auth is required \
                     (VULYK_AUTH_REQUIRED / VULYK_REQUIRE_AUTH)"
                );
            }
            _ => {
                tracing::warn!("VULYK_JWT_SECRET unset; using insecure default for local/dev");
                INSECURE_JWT_DEFAULT.into()
            }
        };
        Self {
            required,
            jwt_secret,
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

/// True when deployment looks production-like (`VULYK_ENV=production|prod` or
/// `VULYK_BASE_URL` is `https://…`). Used to fail closed on auth (H11).
pub fn production_like_deployment(env: Option<&str>, base_url: Option<&str>) -> bool {
    if let Some(env) = env.map(str::trim).filter(|s| !s.is_empty()) {
        let lower = env.to_ascii_lowercase();
        if matches!(lower.as_str(), "production" | "prod") {
            return true;
        }
    }
    if let Some(base) = base_url.map(str::trim).filter(|s| !s.is_empty()) {
        if base.to_ascii_lowercase().starts_with("https://") {
            return true;
        }
    }
    false
}

fn production_like_from_env() -> bool {
    production_like_deployment(
        std::env::var("VULYK_ENV").ok().as_deref(),
        std::env::var("VULYK_BASE_URL").ok().as_deref(),
    )
}

/// Explicit auth flag from env: `Some(true/false)` when set, `None` when unset/empty.
fn explicit_auth_required_from_env() -> Option<bool> {
    for key in ["VULYK_AUTH_REQUIRED", "VULYK_REQUIRE_AUTH"] {
        if let Ok(v) = std::env::var(key) {
            let trimmed = v.trim();
            if trimmed.is_empty() {
                continue;
            }
            return Some(matches!(
                trimmed.to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            ));
        }
    }
    None
}

/// Resolve whether auth is required.
///
/// - Local/dev (no production markers): defaults to **open** when unset.
/// - Production-like (`VULYK_ENV=production|prod` or https `VULYK_BASE_URL`):
///   defaults to **required**; panics if auth is explicitly disabled.
pub fn resolve_auth_required(explicit: Option<bool>, production_like: bool) -> bool {
    match (explicit, production_like) {
        (Some(false), true) => {
            panic!(
                "refusing to start with auth disabled under production markers \
                 (VULYK_ENV=production|prod or https VULYK_BASE_URL). \
                 Set VULYK_AUTH_REQUIRED=true (or unset it to auto-enable)"
            );
        }
        (Some(v), _) => v,
        (None, true) => {
            tracing::warn!(
                "production markers detected without VULYK_AUTH_REQUIRED; enabling auth (fail closed)"
            );
            true
        }
        (None, false) => false,
    }
}

/// Accepts `VULYK_AUTH_REQUIRED` (W1) or legacy `VULYK_REQUIRE_AUTH` (W9/W10).
/// Fail-closed under production markers (see [`resolve_auth_required`]).
pub fn require_auth_enabled() -> bool {
    resolve_auth_required(
        explicit_auth_required_from_env(),
        production_like_from_env(),
    )
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
    ApiToken {
        token_id: Uuid,
    },
    /// Project-scoped CI reporting token (`vulyk_ci_…`).
    CiToken {
        token_id: Uuid,
        project_id: Uuid,
    },
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
        let user = resolve_auth(parts, state).await?;
        let method = parts.method.as_str();
        let path = parts.uri.path();
        if user.must_change_password() && !password_change_allowed_path(method, path) {
            return Err(AppError::Forbidden(
                "password change required before accessing this resource".into(),
            ));
        }
        Ok(Self(user))
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

/// Paths a user with `must_change_password` may still call (method-agnostic match
/// uses the request method separately where needed).
pub fn password_change_allowed_path(method: &str, path: &str) -> bool {
    let path = path.split('?').next().unwrap_or(path);
    matches!(
        (method, path),
        ("GET", "/api/v1/auth/status")
            | ("GET", "/api/v1/auth/me")
            | ("POST", "/api/v1/auth/change-password")
            | ("POST", "/api/v1/auth/logout")
            | ("GET", "/health")
            | ("GET", "/api/health")
    ) || path.starts_with("/api/docs")
}

fn forbid_must_change_password() -> Response {
    (
        StatusCode::FORBIDDEN,
        Json(json!({
            "error": {
                "code": "password_change_required",
                "message": "password change required before accessing this resource"
            }
        })),
    )
        .into_response()
}

fn block_if_must_change(user: &AuthUser, method: &str, path: &str) -> Option<Response> {
    if user.must_change_password() && !password_change_allowed_path(method, path) {
        Some(forbid_must_change_password())
    } else {
        None
    }
}

/// Layer used by `routes::app_router`. Accepts JWT, session, or API token.
pub async fn bearer_middleware(
    State(state): State<AppState>,
    mut req: Request<Body>,
    next: Next,
) -> Response {
    let method = req.method().as_str().to_string();
    let path = req.uri().path().to_string();
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
                    if let Some(resp) = block_if_must_change(&user, &method, &path) {
                        return resp;
                    }
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
            if let Some(resp) = block_if_must_change(&user, &method, &path) {
                return resp;
            }
            let token_id = match user.via {
                AuthVia::ApiToken { token_id } => Some(token_id),
                AuthVia::CiToken { token_id, .. } => Some(token_id),
                _ => None,
            };
            let user_id = match &user.via {
                AuthVia::CiToken { .. } if user.id() == Uuid::nil() => None,
                _ => Some(user.id()),
            };
            req.extensions_mut()
                .insert(AuthContext { token_id, user_id });
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
    if token.starts_with("vulyk_ci_") {
        return authenticate_ci_token(pool, token).await;
    }
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

async fn authenticate_ci_token(pool: &PgPool, raw_token: &str) -> AppResult<AuthUser> {
    let token_hash = hash_secret(raw_token);
    let row = sqlx::query_as::<_, (Uuid, Uuid)>(
        r#"SELECT id, project_id FROM project_ci_tokens
           WHERE token_hash = $1 AND revoked_at IS NULL"#,
    )
    .bind(&token_hash)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::Unauthorized("invalid or revoked CI token".into()))?;

    let (token_id, project_id) = row;
    sqlx::query("UPDATE project_ci_tokens SET last_used_at = NOW() WHERE id = $1")
        .bind(token_id)
        .execute(pool)
        .await?;

    Ok(AuthUser {
        user: User {
            id: Uuid::nil(),
            email: "ci-token@vulyk.local".into(),
            display_name: "CI token".into(),
            password_hash: None,
            is_system_admin: false,
            must_change_password: false,
            locale: None,
            timezone: None,
            preferences: serde_json::json!({}),
            disabled_at: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        },
        via: AuthVia::CiToken {
            token_id,
            project_id,
        },
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

    // Reject legacy headless tokens (null user_id). They previously synthesized a
    // system-admin principal and bypassed RBAC — that is no longer allowed.
    let user_id = user_id.ok_or_else(|| {
        AppError::Unauthorized(
            "API token is not bound to a user; revoke it and create a new personal token".into(),
        )
    })?;

    let user = load_active_user(pool, user_id).await?;
    Ok(AuthUser {
        user,
        via: AuthVia::ApiToken { token_id },
    })
}

async fn load_active_user(pool: &PgPool, user_id: Uuid) -> AppResult<User> {
    let user = sqlx::query_as::<_, User>(
        r#"SELECT id, email, display_name, password_hash, is_system_admin,
                  must_change_password, locale, timezone, preferences,
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
            // Project CI tokens: least privilege — upload results + view their project only.
            // ExecuteRuns / manage / mutate APIs are not granted (use a user token instead).
            if let AuthVia::CiToken {
                project_id: token_project,
                ..
            } = &user.via
            {
                if *token_project != project_id {
                    return Err(AppError::Forbidden(
                        "CI token is not scoped to this project".into(),
                    ));
                }
                return match permission {
                    Permission::UploadResults | Permission::ViewProject => Ok(()),
                    _ => Err(AppError::Forbidden(format!(
                        "CI token missing permission: {}",
                        permission.as_str()
                    ))),
                };
            }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_like_detects_env_and_https_base() {
        assert!(!production_like_deployment(None, None));
        assert!(!production_like_deployment(
            Some("development"),
            Some("http://localhost:8080")
        ));
        assert!(!production_like_deployment(
            Some("staging"),
            Some("http://app.example")
        ));
        assert!(production_like_deployment(Some("production"), None));
        assert!(production_like_deployment(Some("PROD"), None));
        assert!(production_like_deployment(
            None,
            Some("https://vulyk.example")
        ));
        assert!(production_like_deployment(
            Some("production"),
            Some("https://vulyk.example")
        ));
    }

    #[test]
    fn resolve_auth_keeps_local_open_by_default() {
        assert!(!resolve_auth_required(None, false));
        assert!(!resolve_auth_required(Some(false), false));
        assert!(resolve_auth_required(Some(true), false));
    }

    #[test]
    fn resolve_auth_fail_closed_under_production_markers() {
        assert!(resolve_auth_required(None, true));
        assert!(resolve_auth_required(Some(true), true));
    }

    #[test]
    #[should_panic(expected = "refusing to start with auth disabled")]
    fn resolve_auth_panics_when_explicitly_disabled_in_production() {
        let _ = resolve_auth_required(Some(false), true);
    }
}
