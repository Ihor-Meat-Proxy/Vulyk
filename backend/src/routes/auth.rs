use axum::{
    extract::State,
    http::{header::SET_COOKIE, HeaderMap, HeaderValue, StatusCode},
    routing::{get, post},
    Json, Router,
};
use axum_extra::extract::CookieJar;
use cookie::{Cookie, SameSite};

use crate::auth::{
    generate_session_token, hash_password, hash_secret, issue_jwt, session_cookie_name,
    verify_password, AuthUser, OptionalAuthUser, RequireAuth,
};
use crate::error::{AppError, AppResult};
use crate::models::{AuthResponse, AuthStatus, BootstrapRequest, LoginRequest, User, UserPublic};
use crate::routes::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/auth/status", get(auth_status))
        .route("/api/v1/auth/bootstrap", post(bootstrap))
        .route("/api/v1/auth/login", post(login))
        .route("/api/v1/auth/logout", post(logout))
        .route("/api/v1/auth/me", get(me))
}

#[utoipa::path(
    get,
    path = "/api/v1/auth/status",
    tag = "auth",
    responses((status = 200, description = "Auth configuration and current user", body = AuthStatus))
)]
pub async fn auth_status(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
) -> AppResult<Json<AuthStatus>> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(AuthStatus {
        auth_required: state.auth.required,
        bootstrap_available: count == 0,
        authenticated: auth.is_some(),
        user: auth.map(|a| UserPublic::from(a.user)),
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/bootstrap",
    tag = "auth",
    request_body = BootstrapRequest,
    responses(
        (status = 200, description = "First system admin created", body = AuthResponse),
        (status = 409, description = "Users already exist")
    )
)]
pub async fn bootstrap(
    State(state): State<AppState>,
    Json(body): Json<BootstrapRequest>,
) -> AppResult<(HeaderMap, Json<AuthResponse>)> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&state.pool)
        .await?;
    if count > 0 {
        return Err(AppError::Conflict(
            "bootstrap only allowed when no users exist".into(),
        ));
    }

    let email = normalize_email(&body.email)?;
    let password = validate_password(&body.password)?;
    let display_name = body.display_name.trim();
    if display_name.is_empty() {
        return Err(AppError::BadRequest("display_name is required".into()));
    }

    let password_hash = hash_password(password)?;
    let user = sqlx::query_as::<_, User>(
        r#"INSERT INTO users (email, display_name, password_hash, is_system_admin)
           VALUES ($1, $2, $3, TRUE)
           RETURNING id, email, display_name, password_hash, is_system_admin,
                     disabled_at, created_at, updated_at"#,
    )
    .bind(&email)
    .bind(display_name)
    .bind(&password_hash)
    .fetch_one(&state.pool)
    .await?;

    issue_auth_response(&state, user).await
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/login",
    tag = "auth",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Logged in", body = AuthResponse),
        (status = 401, description = "Invalid credentials")
    )
)]
pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> AppResult<(HeaderMap, Json<AuthResponse>)> {
    let email = normalize_email(&body.email)?;
    let user = sqlx::query_as::<_, User>(
        r#"SELECT id, email, display_name, password_hash, is_system_admin,
                  disabled_at, created_at, updated_at
           FROM users WHERE email = $1"#,
    )
    .bind(&email)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::Unauthorized("invalid email or password".into()))?;

    if user.disabled_at.is_some() {
        return Err(AppError::Forbidden("user account is disabled".into()));
    }

    let hash = user
        .password_hash
        .as_deref()
        .ok_or_else(|| AppError::Unauthorized("password login not available".into()))?;
    if !verify_password(&body.password, hash)? {
        return Err(AppError::Unauthorized("invalid email or password".into()));
    }

    issue_auth_response(&state, user).await
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/logout",
    tag = "auth",
    responses((status = 204, description = "Logged out"))
)]
pub async fn logout(
    State(state): State<AppState>,
    jar: CookieJar,
) -> AppResult<(HeaderMap, StatusCode)> {
    if let Some(cookie) = jar.get(session_cookie_name()) {
        let token_hash = hash_secret(cookie.value());
        sqlx::query("DELETE FROM sessions WHERE token_hash = $1")
            .bind(token_hash)
            .execute(&state.pool)
            .await?;
    }

    let mut headers = HeaderMap::new();
    let clear = Cookie::build((session_cookie_name(), ""))
        .http_only(true)
        .path("/")
        .same_site(SameSite::Lax)
        .max_age(time::Duration::seconds(0))
        .build();
    headers.insert(
        SET_COOKIE,
        HeaderValue::from_str(&clear.to_string())
            .map_err(|e| AppError::Internal(format!("cookie header: {e}")))?,
    );
    Ok((headers, StatusCode::NO_CONTENT))
}

#[utoipa::path(
    get,
    path = "/api/v1/auth/me",
    tag = "auth",
    responses(
        (status = 200, description = "Current user", body = UserPublic),
        (status = 401, description = "Unauthorized")
    ),
    security(("bearer_auth" = []))
)]
pub async fn me(RequireAuth(auth): RequireAuth) -> AppResult<Json<UserPublic>> {
    Ok(Json(UserPublic::from(auth.user)))
}

async fn issue_auth_response(
    state: &AppState,
    user: User,
) -> AppResult<(HeaderMap, Json<AuthResponse>)> {
    let jwt = issue_jwt(
        user.id,
        &user.email,
        user.is_system_admin,
        &state.auth.jwt_secret,
        state.auth.jwt_ttl_hours,
    )?;

    let session_token = generate_session_token();
    let token_hash = hash_secret(&session_token);
    let expires_at = chrono::Utc::now() + chrono::Duration::hours(state.auth.session_ttl_hours);

    sqlx::query(
        r#"INSERT INTO sessions (user_id, token_hash, expires_at)
           VALUES ($1, $2, $3)"#,
    )
    .bind(user.id)
    .bind(&token_hash)
    .bind(expires_at)
    .execute(&state.pool)
    .await?;

    let cookie = Cookie::build((session_cookie_name(), session_token))
        .http_only(true)
        .path("/")
        .same_site(SameSite::Lax)
        .max_age(time::Duration::hours(state.auth.session_ttl_hours))
        .build();

    let mut headers = HeaderMap::new();
    headers.insert(
        SET_COOKIE,
        HeaderValue::from_str(&cookie.to_string())
            .map_err(|e| AppError::Internal(format!("cookie header: {e}")))?,
    );

    Ok((
        headers,
        Json(AuthResponse {
            token: jwt,
            token_type: "Bearer".into(),
            expires_in_hours: state.auth.jwt_ttl_hours,
            user: UserPublic::from(user),
        }),
    ))
}

fn normalize_email(email: &str) -> AppResult<String> {
    let email = email.trim().to_lowercase();
    if !email.contains('@') || email.len() < 3 {
        return Err(AppError::BadRequest("valid email is required".into()));
    }
    Ok(email)
}

fn validate_password(password: &str) -> AppResult<&str> {
    if password.len() < 8 {
        return Err(AppError::BadRequest(
            "password must be at least 8 characters".into(),
        ));
    }
    Ok(password)
}

/// Used by membership/admin helpers.
#[allow(dead_code)]
pub fn auth_user_public(auth: &AuthUser) -> UserPublic {
    UserPublic::from(auth.user.clone())
}
