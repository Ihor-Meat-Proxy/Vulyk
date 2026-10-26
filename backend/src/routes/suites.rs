use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use uuid::Uuid;

use crate::auth::{require_permission, OptionalAuthUser, Permission};
use crate::error::{AppError, AppResult};
use crate::models::{CreateSuite, DeleteSuiteQuery, Suite, UpdateSuite};
use crate::routes::AppState;
use crate::services::case_keys::retire_case_key;
use crate::services::suites::{get_suite_with_counts, list_suites_with_counts};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/projects/{project_id}/suites",
            get(list_suites).post(create_suite),
        )
        .route(
            "/api/v1/projects/{project_id}/suites/{suite_id}",
            get(get_suite).put(update_suite).delete(delete_suite),
        )
}

pub async fn ensure_project(pool: &sqlx::PgPool, project_id: Uuid) -> AppResult<()> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM projects WHERE id = $1)")
        .bind(project_id)
        .fetch_one(pool)
        .await?;
    if !exists {
        return Err(AppError::NotFound("project".into()));
    }
    Ok(())
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/suites",
    tag = "suites",
    params(("project_id" = Uuid, Path, description = "Project id")),
    responses((status = 200, description = "List suites/folders with recursive case_count", body = [Suite])),
    security(("bearer_auth" = []))
)]
pub async fn list_suites(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
) -> AppResult<Json<Vec<Suite>>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    ensure_project(&state.pool, project_id).await?;
    Ok(Json(
        list_suites_with_counts(&state.pool, project_id).await?,
    ))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/suites",
    tag = "suites",
    params(("project_id" = Uuid, Path, description = "Project id")),
    request_body = CreateSuite,
    responses((status = 200, description = "Created suite/folder", body = Suite)),
    security(("bearer_auth" = []))
)]
pub async fn create_suite(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<CreateSuite>,
) -> AppResult<Json<Suite>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ManageSuites,
    )
    .await?;
    ensure_project(&state.pool, project_id).await?;
    if body.name.trim().is_empty() {
        return Err(AppError::BadRequest("name is required".into()));
    }

    if let Some(parent_id) = body.parent_id {
        let parent_ok: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM suites WHERE project_id = $1 AND id = $2)",
        )
        .bind(project_id)
        .bind(parent_id)
        .fetch_one(&state.pool)
        .await?;
        if !parent_ok {
            return Err(AppError::BadRequest("parent folder not found".into()));
        }
    }

    let suite = sqlx::query_as::<_, Suite>(
        r#"WITH inserted AS (
             INSERT INTO suites (project_id, parent_id, name, sort_order)
             VALUES ($1, $2, $3, $4)
             RETURNING id, project_id, parent_id, name, sort_order, created_at, updated_at
           )
           SELECT i.id, i.project_id, i.parent_id, i.name, i.sort_order,
                  i.created_at, i.updated_at,
                  0::bigint AS case_count,
                  0::bigint AS direct_case_count
           FROM inserted i"#,
    )
    .bind(project_id)
    .bind(body.parent_id)
    .bind(body.name.trim())
    .bind(body.sort_order)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(suite))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/suites/{suite_id}",
    tag = "suites",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("suite_id" = Uuid, Path, description = "Suite/folder id")
    ),
    responses((status = 200, description = "Suite", body = Suite)),
    security(("bearer_auth" = []))
)]
pub async fn get_suite(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, suite_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<Suite>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    let suite = get_suite_with_counts(&state.pool, project_id, suite_id)
        .await?
        .ok_or_else(|| AppError::NotFound("suite".into()))?;
    Ok(Json(suite))
}

async fn would_create_cycle(
    pool: &sqlx::PgPool,
    suite_id: Uuid,
    new_parent: Uuid,
) -> AppResult<bool> {
    if suite_id == new_parent {
        return Ok(true);
    }
    let is_descendant: bool = sqlx::query_scalar(
        r#"WITH RECURSIVE tree AS (
             SELECT id FROM suites WHERE id = $1
             UNION ALL
             SELECT s.id FROM suites s INNER JOIN tree t ON s.parent_id = t.id
           )
           SELECT EXISTS(SELECT 1 FROM tree WHERE id = $2)"#,
    )
    .bind(suite_id)
    .bind(new_parent)
    .fetch_one(pool)
    .await?;
    Ok(is_descendant)
}

#[utoipa::path(
    put,
    path = "/api/v1/projects/{project_id}/suites/{suite_id}",
    tag = "suites",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("suite_id" = Uuid, Path, description = "Suite/folder id")
    ),
    request_body = UpdateSuite,
    responses((status = 200, description = "Updated suite/folder", body = Suite)),
    security(("bearer_auth" = []))
)]
pub async fn update_suite(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, suite_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateSuite>,
) -> AppResult<Json<Suite>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ManageSuites,
    )
    .await?;
    let existing = get_suite_with_counts(&state.pool, project_id, suite_id)
        .await?
        .ok_or_else(|| AppError::NotFound("suite".into()))?;

    let name = body.name.unwrap_or(existing.name);
    let parent_id = if body.clear_parent {
        None
    } else {
        body.parent_id.or(existing.parent_id)
    };
    let sort_order = body.sort_order.unwrap_or(existing.sort_order);

    if let Some(pid) = parent_id {
        if would_create_cycle(&state.pool, suite_id, pid).await? {
            return Err(AppError::BadRequest(
                "cannot move a folder under itself or a descendant".into(),
            ));
        }
        let parent_ok: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM suites WHERE project_id = $1 AND id = $2)",
        )
        .bind(project_id)
        .bind(pid)
        .fetch_one(&state.pool)
        .await?;
        if !parent_ok {
            return Err(AppError::BadRequest("parent folder not found".into()));
        }
    }

    sqlx::query(
        r#"UPDATE suites
           SET name = $3, parent_id = $4, sort_order = $5, updated_at = NOW()
           WHERE project_id = $1 AND id = $2"#,
    )
    .bind(project_id)
    .bind(suite_id)
    .bind(&name)
    .bind(parent_id)
    .bind(sort_order)
    .execute(&state.pool)
    .await?;

    let suite = get_suite_with_counts(&state.pool, project_id, suite_id)
        .await?
        .ok_or_else(|| AppError::NotFound("suite".into()))?;
    Ok(Json(suite))
}

#[utoipa::path(
    delete,
    path = "/api/v1/projects/{project_id}/suites/{suite_id}",
    tag = "suites",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("suite_id" = Uuid, Path, description = "Suite/folder id"),
        ("on_nonempty" = Option<String>, Query, description = "move_to_parent (default) or delete_cases")
    ),
    responses((status = 204, description = "Deleted")),
    security(("bearer_auth" = []))
)]
pub async fn delete_suite(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, suite_id)): Path<(Uuid, Uuid)>,
    Query(query): Query<DeleteSuiteQuery>,
) -> AppResult<axum::http::StatusCode> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ManageSuites,
    )
    .await?;

    let existing = get_suite_with_counts(&state.pool, project_id, suite_id)
        .await?
        .ok_or_else(|| AppError::NotFound("suite".into()))?;

    let mut tx = state.pool.begin().await?;

    let descendant_ids: Vec<Uuid> = sqlx::query_scalar(
        r#"WITH RECURSIVE tree AS (
             SELECT id FROM suites WHERE id = $1
             UNION ALL
             SELECT s.id FROM suites s INNER JOIN tree t ON s.parent_id = t.id
           )
           SELECT id FROM tree"#,
    )
    .bind(suite_id)
    .fetch_all(&mut *tx)
    .await?;

    match query.on_nonempty.as_str() {
        "delete_cases" => {
            require_permission(
                &state.pool,
                &auth,
                state.auth.required,
                project_id,
                Permission::DeleteCases,
            )
            .await?;
            let keys: Vec<(String,)> = sqlx::query_as(
                "SELECT key FROM test_cases WHERE project_id = $1 AND suite_id = ANY($2)",
            )
            .bind(project_id)
            .bind(&descendant_ids)
            .fetch_all(&mut *tx)
            .await?;
            for (key,) in &keys {
                retire_case_key(&mut tx, project_id, key).await?;
            }
            sqlx::query("DELETE FROM test_cases WHERE project_id = $1 AND suite_id = ANY($2)")
                .bind(project_id)
                .bind(&descendant_ids)
                .execute(&mut *tx)
                .await?;
        }
        "move_to_parent" | "" => {
            let parent = existing.parent_id;
            sqlx::query(
                r#"UPDATE test_cases
                   SET suite_id = $3, updated_at = NOW()
                   WHERE project_id = $1 AND suite_id = $2"#,
            )
            .bind(project_id)
            .bind(suite_id)
            .bind(parent)
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                r#"UPDATE suites
                   SET parent_id = $3, updated_at = NOW()
                   WHERE project_id = $1 AND parent_id = $2"#,
            )
            .bind(project_id)
            .bind(suite_id)
            .bind(parent)
            .execute(&mut *tx)
            .await?;
        }
        other => {
            return Err(AppError::BadRequest(format!(
                "on_nonempty must be move_to_parent or delete_cases, got {other}"
            )));
        }
    }

    let result = sqlx::query("DELETE FROM suites WHERE project_id = $1 AND id = $2")
        .bind(project_id)
        .bind(suite_id)
        .execute(&mut *tx)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("suite".into()));
    }
    tx.commit().await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}
