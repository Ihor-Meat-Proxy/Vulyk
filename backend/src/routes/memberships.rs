use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use uuid::Uuid;

use crate::auth::{
    require_permission, require_project_role, OptionalAuthUser, Permission, ProjectRole,
};
use crate::error::{AppError, AppResult};
use crate::models::{Membership, MembershipView, UpdateMembership, UpsertMembership};
use crate::routes::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/projects/{project_id}/memberships",
            get(list_memberships).post(upsert_membership),
        )
        .route(
            "/api/v1/projects/{project_id}/memberships/{membership_id}",
            axum::routing::put(update_membership).delete(delete_membership),
        )
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/memberships",
    tag = "memberships",
    params(("project_id" = Uuid, Path, description = "Project id")),
    responses((status = 200, description = "Project memberships", body = [MembershipView])),
    security(("bearer_auth" = []))
)]
pub async fn list_memberships(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
) -> AppResult<Json<Vec<MembershipView>>> {
    require_project_role(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        ProjectRole::Viewer,
    )
    .await?;

    let rows = sqlx::query_as::<_, MembershipView>(
        r#"SELECT m.id, m.project_id, m.user_id, m.role, u.email, u.display_name,
                  m.created_at, m.updated_at
           FROM memberships m
           JOIN users u ON u.id = m.user_id
           WHERE m.project_id = $1
           ORDER BY u.email"#,
    )
    .bind(project_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/memberships",
    tag = "memberships",
    params(("project_id" = Uuid, Path, description = "Project id")),
    request_body = UpsertMembership,
    responses((status = 200, description = "Membership upserted", body = Membership)),
    security(("bearer_auth" = []))
)]
pub async fn upsert_membership(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<UpsertMembership>,
) -> AppResult<Json<Membership>> {
    ensure_can_manage_members(&state, &auth, project_id).await?;
    ensure_project_exists(&state, project_id).await?;
    ensure_user_exists(&state, body.user_id).await?;

    let membership = sqlx::query_as::<_, Membership>(
        r#"INSERT INTO memberships (project_id, user_id, role)
           VALUES ($1, $2, $3)
           ON CONFLICT (project_id, user_id)
           DO UPDATE SET role = EXCLUDED.role, updated_at = NOW()
           RETURNING id, project_id, user_id, role, created_at, updated_at"#,
    )
    .bind(project_id)
    .bind(body.user_id)
    .bind(body.role)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(membership))
}

#[utoipa::path(
    put,
    path = "/api/v1/projects/{project_id}/memberships/{membership_id}",
    tag = "memberships",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("membership_id" = Uuid, Path, description = "Membership id")
    ),
    request_body = UpdateMembership,
    responses((status = 200, description = "Updated membership", body = Membership)),
    security(("bearer_auth" = []))
)]
pub async fn update_membership(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, membership_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateMembership>,
) -> AppResult<Json<Membership>> {
    ensure_can_manage_members(&state, &auth, project_id).await?;

    let membership = sqlx::query_as::<_, Membership>(
        r#"UPDATE memberships
           SET role = $3, updated_at = NOW()
           WHERE id = $1 AND project_id = $2
           RETURNING id, project_id, user_id, role, created_at, updated_at"#,
    )
    .bind(membership_id)
    .bind(project_id)
    .bind(body.role)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("membership".into()))?;

    Ok(Json(membership))
}

#[utoipa::path(
    delete,
    path = "/api/v1/projects/{project_id}/memberships/{membership_id}",
    tag = "memberships",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("membership_id" = Uuid, Path, description = "Membership id")
    ),
    responses((status = 204, description = "Removed")),
    security(("bearer_auth" = []))
)]
pub async fn delete_membership(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, membership_id)): Path<(Uuid, Uuid)>,
) -> AppResult<axum::http::StatusCode> {
    ensure_can_manage_members(&state, &auth, project_id).await?;

    let result = sqlx::query("DELETE FROM memberships WHERE id = $1 AND project_id = $2")
        .bind(membership_id)
        .bind(project_id)
        .execute(&state.pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("membership".into()));
    }
    Ok(axum::http::StatusCode::NO_CONTENT)
}

async fn ensure_can_manage_members(
    state: &AppState,
    auth: &Option<crate::auth::AuthUser>,
    project_id: Uuid,
) -> AppResult<()> {
    require_permission(
        &state.pool,
        auth,
        state.auth.required,
        project_id,
        Permission::ManageMemberships,
    )
    .await
}

async fn ensure_project_exists(state: &AppState, project_id: Uuid) -> AppResult<()> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM projects WHERE id = $1)")
        .bind(project_id)
        .fetch_one(&state.pool)
        .await?;
    if !exists {
        return Err(AppError::NotFound("project".into()));
    }
    Ok(())
}

async fn ensure_user_exists(state: &AppState, user_id: Uuid) -> AppResult<()> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE id = $1)")
        .bind(user_id)
        .fetch_one(&state.pool)
        .await?;
    if !exists {
        return Err(AppError::NotFound("user".into()));
    }
    Ok(())
}
