use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use uuid::Uuid;

use crate::auth::{hash_password, RequireSystemAdmin};
use crate::error::{AppError, AppResult};
use crate::models::{CreateUserRequest, UpdateUserRequest, User, UserPublic};
use crate::routes::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/users", get(list_users).post(create_user))
        .route(
            "/api/v1/users/{id}",
            get(get_user).put(update_user).delete(delete_user),
        )
}

#[utoipa::path(
    get,
    path = "/api/v1/users",
    tag = "users",
    responses((status = 200, description = "List users", body = [UserPublic])),
    security(("bearer_auth" = []))
)]
pub async fn list_users(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
) -> AppResult<Json<Vec<UserPublic>>> {
    let users = sqlx::query_as::<_, User>(
        r#"SELECT id, email, display_name, password_hash, is_system_admin,
                  disabled_at, created_at, updated_at
           FROM users ORDER BY email"#,
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(users.into_iter().map(UserPublic::from).collect()))
}

#[utoipa::path(
    post,
    path = "/api/v1/users",
    tag = "users",
    request_body = CreateUserRequest,
    responses(
        (status = 200, description = "Created user", body = UserPublic),
        (status = 403, description = "Forbidden")
    ),
    security(("bearer_auth" = []))
)]
pub async fn create_user(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
    Json(body): Json<CreateUserRequest>,
) -> AppResult<Json<UserPublic>> {
    let email = body.email.trim().to_lowercase();
    if !email.contains('@') {
        return Err(AppError::BadRequest("valid email is required".into()));
    }
    if body.password.len() < 8 {
        return Err(AppError::BadRequest(
            "password must be at least 8 characters".into(),
        ));
    }
    let display_name = body.display_name.trim();
    if display_name.is_empty() {
        return Err(AppError::BadRequest("display_name is required".into()));
    }

    let password_hash = hash_password(&body.password)?;
    let user = sqlx::query_as::<_, User>(
        r#"INSERT INTO users (email, display_name, password_hash, is_system_admin)
           VALUES ($1, $2, $3, $4)
           RETURNING id, email, display_name, password_hash, is_system_admin,
                     disabled_at, created_at, updated_at"#,
    )
    .bind(&email)
    .bind(display_name)
    .bind(&password_hash)
    .bind(body.is_system_admin)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(UserPublic::from(user)))
}

#[utoipa::path(
    get,
    path = "/api/v1/users/{id}",
    tag = "users",
    params(("id" = Uuid, Path, description = "User id")),
    responses((status = 200, description = "User", body = UserPublic)),
    security(("bearer_auth" = []))
)]
pub async fn get_user(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
    Path(id): Path<Uuid>,
) -> AppResult<Json<UserPublic>> {
    let user = load_user(&state, id).await?;
    Ok(Json(UserPublic::from(user)))
}

#[utoipa::path(
    put,
    path = "/api/v1/users/{id}",
    tag = "users",
    params(("id" = Uuid, Path, description = "User id")),
    request_body = UpdateUserRequest,
    responses((status = 200, description = "Updated user", body = UserPublic)),
    security(("bearer_auth" = []))
)]
pub async fn update_user(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateUserRequest>,
) -> AppResult<Json<UserPublic>> {
    let existing = load_user(&state, id).await?;
    let display_name = body
        .display_name
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or(existing.display_name);
    let is_system_admin = body.is_system_admin.unwrap_or(existing.is_system_admin);
    let disabled_at = match body.disabled {
        Some(true) => Some(existing.disabled_at.unwrap_or_else(chrono::Utc::now)),
        Some(false) => None,
        None => existing.disabled_at,
    };
    let password_hash = if let Some(password) = body.password {
        if password.len() < 8 {
            return Err(AppError::BadRequest(
                "password must be at least 8 characters".into(),
            ));
        }
        Some(hash_password(&password)?)
    } else {
        existing.password_hash
    };

    let user = sqlx::query_as::<_, User>(
        r#"UPDATE users
           SET display_name = $2,
               password_hash = $3,
               is_system_admin = $4,
               disabled_at = $5,
               updated_at = NOW()
           WHERE id = $1
           RETURNING id, email, display_name, password_hash, is_system_admin,
                     disabled_at, created_at, updated_at"#,
    )
    .bind(id)
    .bind(display_name)
    .bind(password_hash)
    .bind(is_system_admin)
    .bind(disabled_at)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(UserPublic::from(user)))
}

#[utoipa::path(
    delete,
    path = "/api/v1/users/{id}",
    tag = "users",
    params(("id" = Uuid, Path, description = "User id")),
    responses((status = 204, description = "Deleted")),
    security(("bearer_auth" = []))
)]
pub async fn delete_user(
    State(state): State<AppState>,
    RequireSystemAdmin(admin): RequireSystemAdmin,
    Path(id): Path<Uuid>,
) -> AppResult<axum::http::StatusCode> {
    if admin.id() == id {
        return Err(AppError::BadRequest(
            "cannot delete your own user account".into(),
        ));
    }
    let result = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("user".into()));
    }
    Ok(axum::http::StatusCode::NO_CONTENT)
}

async fn load_user(state: &AppState, id: Uuid) -> AppResult<User> {
    sqlx::query_as::<_, User>(
        r#"SELECT id, email, display_name, password_hash, is_system_admin,
                  disabled_at, created_at, updated_at
           FROM users WHERE id = $1"#,
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("user".into()))
}
