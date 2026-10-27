use axum::{
    extract::{Path, Query, State},
    http::{header::SET_COOKIE, HeaderMap, HeaderValue, StatusCode},
    response::Redirect,
    routing::{get, post},
    Json, Router,
};
use axum_extra::extract::CookieJar;
use cookie::{Cookie, SameSite};
use serde::Deserialize;

use crate::auth::{
    generate_session_token, hash_password_async, hash_secret, issue_jwt, session_cookie_name,
    verify_password_async, verify_password_for_login_async, AuthUser, OptionalAuthUser,
    RequireAuth,
};
use crate::error::{AppError, AppResult};
use crate::models::{
    AuthResponse, AuthStatus, BootstrapRequest, ChangePasswordRequest, LoginRequest,
    RegisterRequest, SsoProviderPublic, UpdateMeRequest, User, UserPublic,
};
use crate::routes::AppState;
use crate::settings::{self, RegistrationMode};
use crate::sso;

const USER_COLUMNS: &str = r#"id, email, display_name, password_hash, is_system_admin,
                  must_change_password, locale, timezone, preferences,
                  disabled_at, created_at, updated_at"#;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/auth/status", get(auth_status))
        .route("/api/v1/auth/bootstrap", post(bootstrap))
        .route("/api/v1/auth/register", post(register))
        .route("/api/v1/auth/login", post(login))
        .route("/api/v1/auth/logout", post(logout))
        .route("/api/v1/auth/me", get(me).patch(update_me))
        .route("/api/v1/auth/change-password", post(change_password))
        .route("/api/v1/auth/sso/providers", get(sso_providers))
        .route("/api/v1/auth/sso/{key}/start", get(sso_start))
        .route("/api/v1/auth/sso/callback", get(sso_callback))
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
    let instance = settings::load_settings(&state.pool).await?;
    let sso_providers = sso::list_enabled_public(&state.pool).await?;

    Ok(Json(AuthStatus {
        auth_required: state.auth.required,
        bootstrap_available: !instance.setup_completed && count == 0,
        setup_required: !instance.setup_completed,
        registration_mode: instance.registration_mode,
        default_locale: instance.default_locale,
        default_timezone: instance.default_timezone,
        default_theme: instance.default_theme,
        instance_name: instance.instance_name,
        authenticated: auth.is_some(),
        user: auth.map(|a| UserPublic::from(a.user)),
        sso_providers,
        password_policy: state.password_policy.public(),
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/bootstrap",
    tag = "auth",
    request_body = BootstrapRequest,
    responses(
        (status = 200, description = "First system admin created", body = AuthResponse),
        (status = 409, description = "Users already exist / setup completed")
    )
)]
pub async fn bootstrap(
    State(state): State<AppState>,
    Json(body): Json<BootstrapRequest>,
) -> AppResult<(HeaderMap, Json<AuthResponse>)> {
    let instance = settings::load_settings(&state.pool).await?;
    if instance.setup_completed {
        return Err(AppError::Conflict(
            "setup already completed; use login or register".into(),
        ));
    }
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&state.pool)
        .await?;
    if count > 0 {
        return Err(AppError::Conflict(
            "bootstrap only allowed when no users exist".into(),
        ));
    }

    let email = normalize_email(&body.email)?;
    let password = state.password_policy.validate(&body.password)?;
    let display_name = body.display_name.trim();
    if display_name.is_empty() {
        return Err(AppError::BadRequest("display_name is required".into()));
    }

    let password_hash = hash_password_async(password.to_string()).await?;
    let user = sqlx::query_as::<_, User>(&format!(
        r#"INSERT INTO users (email, display_name, password_hash, is_system_admin)
           VALUES ($1, $2, $3, TRUE)
           RETURNING {USER_COLUMNS}"#
    ))
    .bind(&email)
    .bind(display_name)
    .bind(&password_hash)
    .fetch_one(&state.pool)
    .await?;

    // Mark setup complete when bootstrapping the first admin.
    let mut settings = instance;
    settings.setup_completed = true;
    settings::save_settings(&state.pool, &settings).await?;
    sqlx::query("UPDATE setup_tokens SET consumed_at = NOW() WHERE consumed_at IS NULL")
        .execute(&state.pool)
        .await?;

    issue_auth_response(&state, user).await
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/register",
    tag = "auth",
    request_body = RegisterRequest,
    responses(
        (status = 200, description = "Registered", body = AuthResponse),
        (status = 403, description = "Registration disabled")
    )
)]
pub async fn register(
    State(state): State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> AppResult<(HeaderMap, Json<AuthResponse>)> {
    let instance = settings::load_settings(&state.pool).await?;
    if !instance.setup_completed {
        return Err(AppError::Forbidden(
            "complete instance setup before registering".into(),
        ));
    }

    let email = normalize_email(&body.email)?;
    let password = state.password_policy.validate(&body.password)?;
    let display_name = body.display_name.trim();
    if display_name.is_empty() {
        return Err(AppError::BadRequest("display_name is required".into()));
    }

    if !settings::email_domain_allowed(&email, &instance.allowed_email_domains) {
        return Err(AppError::Forbidden("email domain not allowed".into()));
    }

    let mut invite_id: Option<uuid::Uuid> = None;
    match instance.registration_mode {
        RegistrationMode::Open => {}
        RegistrationMode::Invite => {
            let token = body
                .invite_token
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| AppError::Forbidden("invite token required".into()))?;
            let hash = hash_secret(token);
            let row: Option<(uuid::Uuid, String)> = sqlx::query_as(
                r#"SELECT id, email FROM invites
                   WHERE token_hash = $1
                     AND accepted_at IS NULL
                     AND expires_at > NOW()"#,
            )
            .bind(&hash)
            .fetch_optional(&state.pool)
            .await?;
            let (id, invite_email) =
                row.ok_or_else(|| AppError::Forbidden("invalid or expired invite token".into()))?;
            if !invite_email.eq_ignore_ascii_case(&email) {
                return Err(AppError::Forbidden(
                    "invite email does not match registration email".into(),
                ));
            }
            invite_id = Some(id);
        }
        RegistrationMode::Disabled => {
            return Err(AppError::Forbidden("registration is disabled".into()));
        }
    }

    let password_hash = hash_password_async(password.to_string()).await?;
    let user = sqlx::query_as::<_, User>(&format!(
        r#"INSERT INTO users (
               email, display_name, password_hash, is_system_admin,
               locale, timezone
           ) VALUES ($1, $2, $3, FALSE, $4, $5)
           RETURNING {USER_COLUMNS}"#
    ))
    .bind(&email)
    .bind(display_name)
    .bind(&password_hash)
    .bind(&instance.default_locale)
    .bind(&instance.default_timezone)
    .fetch_one(&state.pool)
    .await?;

    if let Some(id) = invite_id {
        sqlx::query("UPDATE invites SET accepted_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(&state.pool)
            .await?;
    }

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
    headers_in: HeaderMap,
    Json(body): Json<LoginRequest>,
) -> AppResult<(HeaderMap, Json<AuthResponse>)> {
    let email = normalize_email(&body.email)?;
    let client_ip = client_ip_from_headers(&headers_in);
    state.login_throttle.check(&email, client_ip.as_deref())?;

    let user = sqlx::query_as::<_, User>(&format!(
        "SELECT {USER_COLUMNS} FROM users WHERE email = $1"
    ))
    .bind(&email)
    .fetch_optional(&state.pool)
    .await?;

    // Always pay Argon2 verify cost (dummy hash when user/hash missing).
    let hash = user.as_ref().and_then(|u| u.password_hash.clone());
    let password_ok = verify_password_for_login_async(body.password.clone(), hash).await;

    let Some(mut user) = user else {
        state
            .login_throttle
            .record_failure(&email, client_ip.as_deref());
        return Err(AppError::Unauthorized("invalid email or password".into()));
    };

    if user.disabled_at.is_some() {
        state
            .login_throttle
            .record_failure(&email, client_ip.as_deref());
        return Err(AppError::Forbidden("user account is disabled".into()));
    }

    if user.password_hash.is_none() {
        state
            .login_throttle
            .record_failure(&email, client_ip.as_deref());
        return Err(AppError::Unauthorized(
            "password login not available".into(),
        ));
    }

    if !password_ok {
        state
            .login_throttle
            .record_failure(&email, client_ip.as_deref());
        return Err(AppError::Unauthorized("invalid email or password".into()));
    }

    state
        .login_throttle
        .record_success(&email, client_ip.as_deref());

    // Login never rejects by length; weak / legacy passwords force a change.
    if !state.password_policy.meets(&body.password) && !user.must_change_password {
        sqlx::query(
            r#"UPDATE users
               SET must_change_password = TRUE, updated_at = NOW()
               WHERE id = $1"#,
        )
        .bind(user.id)
        .execute(&state.pool)
        .await?;
        user.must_change_password = true;
    }

    issue_auth_response(&state, user).await
}

fn client_ip_from_headers(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|v| v.to_str().ok())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        })
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

#[utoipa::path(
    patch,
    path = "/api/v1/auth/me",
    tag = "auth",
    request_body = UpdateMeRequest,
    responses((status = 200, description = "Updated profile", body = UserPublic)),
    security(("bearer_auth" = []))
)]
pub async fn update_me(
    State(state): State<AppState>,
    RequireAuth(auth): RequireAuth,
    Json(body): Json<UpdateMeRequest>,
) -> AppResult<Json<UserPublic>> {
    let display_name = body
        .display_name
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| auth.user.display_name.clone());
    let locale = body.locale.or_else(|| auth.user.locale.clone());
    let timezone = body.timezone.or_else(|| auth.user.timezone.clone());

    let mut preferences = auth.user.preferences_typed();
    if let Some(patch) = body.preferences.as_ref() {
        preferences
            .merge_update(patch)
            .map_err(AppError::BadRequest)?;
    }
    let preferences_value = preferences.to_value();

    let user = sqlx::query_as::<_, User>(&format!(
        r#"UPDATE users
           SET display_name = $2, locale = $3, timezone = $4, preferences = $5, updated_at = NOW()
           WHERE id = $1
           RETURNING {USER_COLUMNS}"#
    ))
    .bind(auth.id())
    .bind(display_name)
    .bind(locale)
    .bind(timezone)
    .bind(preferences_value)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(UserPublic::from(user)))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/change-password",
    tag = "auth",
    request_body = ChangePasswordRequest,
    responses((status = 204, description = "Password changed")),
    security(("bearer_auth" = []))
)]
pub async fn change_password(
    State(state): State<AppState>,
    RequireAuth(auth): RequireAuth,
    Json(body): Json<ChangePasswordRequest>,
) -> AppResult<StatusCode> {
    let new_password = state.password_policy.validate(&body.new_password)?;

    if let Some(hash) = auth.user.password_hash.clone() {
        let current = body.current_password.clone().unwrap_or_default();
        // Allow skipping current when must_change_password (forced reset after seed).
        if !auth.user.must_change_password {
            if current.is_empty() || !verify_password_async(current.clone(), hash.clone()).await? {
                return Err(AppError::Unauthorized(
                    "current password is incorrect".into(),
                ));
            }
        } else if !current.is_empty() && !verify_password_async(current, hash).await? {
            return Err(AppError::Unauthorized(
                "current password is incorrect".into(),
            ));
        }
    }

    let password_hash = hash_password_async(new_password.to_string()).await?;
    sqlx::query(
        r#"UPDATE users
           SET password_hash = $2, must_change_password = FALSE, updated_at = NOW()
           WHERE id = $1"#,
    )
    .bind(auth.id())
    .bind(&password_hash)
    .execute(&state.pool)
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    get,
    path = "/api/v1/auth/sso/providers",
    tag = "auth",
    responses((status = 200, description = "Enabled SSO providers", body = [SsoProviderPublic]))
)]
pub async fn sso_providers(
    State(state): State<AppState>,
) -> AppResult<Json<Vec<SsoProviderPublic>>> {
    Ok(Json(sso::list_enabled_public(&state.pool).await?))
}

#[derive(Debug, Deserialize)]
pub struct SsoStartQuery {
    pub redirect_to: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/v1/auth/sso/{key}/start",
    tag = "auth",
    params(("key" = String, Path, description = "Provider key")),
    responses((status = 302, description = "Redirect to IdP"))
)]
pub async fn sso_start(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Query(query): Query<SsoStartQuery>,
) -> AppResult<Redirect> {
    let provider = sso::load_provider_by_key(&state.pool, &key)
        .await?
        .ok_or_else(|| AppError::NotFound("sso provider".into()))?;

    let instance = settings::load_settings(&state.pool).await?;
    let base = if instance.base_url.is_empty() {
        std::env::var("VULYK_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".into())
    } else {
        instance.base_url.clone()
    };
    let redirect_uri = sso::callback_redirect_uri(&base);
    let start = sso::start_authorization(
        &state.pool,
        &provider,
        &redirect_uri,
        query.redirect_to.as_deref(),
    )
    .await?;

    Ok(Redirect::temporary(&start.authorize_url))
}

#[derive(Debug, Deserialize)]
pub struct SsoCallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/v1/auth/sso/callback",
    tag = "auth",
    responses((status = 302, description = "Redirect after SSO login"))
)]
pub async fn sso_callback(
    State(state): State<AppState>,
    Query(query): Query<SsoCallbackQuery>,
) -> AppResult<(HeaderMap, Redirect)> {
    if let Some(err) = query.error {
        let desc = query.error_description.unwrap_or_default();
        return Err(AppError::Unauthorized(format!("SSO error: {err} {desc}")));
    }
    let code = query
        .code
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::BadRequest("missing code".into()))?;
    let oauth_state = query
        .state
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::BadRequest("missing state".into()))?;

    let instance = settings::load_settings(&state.pool).await?;
    let base = if instance.base_url.is_empty() {
        std::env::var("VULYK_BASE_URL").unwrap_or_else(|_| "http://localhost:8080".into())
    } else {
        instance.base_url.clone()
    };
    let redirect_uri = sso::callback_redirect_uri(&base);

    let (user, redirect_to) = sso::handle_callback(
        &state.pool,
        &state.secrets,
        &code,
        &oauth_state,
        &redirect_uri,
    )
    .await?;

    if user.disabled_at.is_some() {
        return Err(AppError::Forbidden("user account is disabled".into()));
    }

    let (headers, _) = issue_auth_response(&state, user).await?;
    let dest = redirect_to
        .filter(|s| s.starts_with('/') || s.starts_with(&base))
        .unwrap_or_else(|| format!("{}/", base.trim_end_matches('/')));

    Ok((headers, Redirect::temporary(&dest)))
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

/// Used by membership/admin helpers.
#[allow(dead_code)]
pub fn auth_user_public(auth: &AuthUser) -> UserPublic {
    UserPublic::from(auth.user.clone())
}
