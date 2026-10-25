use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::{CreateCase, CreateStep, TestCase, TestCaseDetail, TestStep, UpdateCase};
use crate::routes::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/projects/{project_id}/cases",
            get(list_cases).post(create_case),
        )
        .route(
            "/api/v1/projects/{project_id}/cases/{case_id}",
            get(get_case).put(update_case).delete(delete_case),
        )
}

#[derive(Debug, Deserialize)]
pub struct ListCasesQuery {
    pub suite_id: Option<Uuid>,
    pub q: Option<String>,
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

pub async fn load_steps(pool: &sqlx::PgPool, case_id: Uuid) -> AppResult<Vec<TestStep>> {
    let steps = sqlx::query_as::<_, TestStep>(
        r#"SELECT id, case_id, position, action, expected_result, data
           FROM test_steps WHERE case_id = $1 ORDER BY position"#,
    )
    .bind(case_id)
    .fetch_all(pool)
    .await?;
    Ok(steps)
}

pub async fn replace_steps(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    case_id: Uuid,
    steps: &[CreateStep],
) -> AppResult<Vec<TestStep>> {
    sqlx::query("DELETE FROM test_steps WHERE case_id = $1")
        .bind(case_id)
        .execute(&mut **tx)
        .await?;

    let mut out = Vec::with_capacity(steps.len());
    for (idx, step) in steps.iter().enumerate() {
        let row = sqlx::query_as::<_, TestStep>(
            r#"INSERT INTO test_steps (case_id, position, action, expected_result, data)
               VALUES ($1, $2, $3, $4, $5)
               RETURNING id, case_id, position, action, expected_result, data"#,
        )
        .bind(case_id)
        .bind(idx as i32)
        .bind(&step.action)
        .bind(&step.expected_result)
        .bind(&step.data)
        .fetch_one(&mut **tx)
        .await?;
        out.push(row);
    }
    Ok(out)
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/cases",
    tag = "cases",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("suite_id" = Option<Uuid>, Query, description = "Filter by suite"),
        ("q" = Option<String>, Query, description = "Full-text / title search")
    ),
    responses((status = 200, description = "List cases", body = [TestCase]))
)]
pub async fn list_cases(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Query(query): Query<ListCasesQuery>,
) -> AppResult<Json<Vec<TestCase>>> {
    ensure_project(&state.pool, project_id).await?;

    let cases = if let Some(ref q) = query.q {
        sqlx::query_as::<_, TestCase>(
            r#"SELECT id, project_id, suite_id, key, title, description, precondition,
                      priority, status, version, created_at, updated_at
               FROM test_cases
               WHERE project_id = $1
                 AND ($2::uuid IS NULL OR suite_id = $2)
                 AND (
                   search_vector @@ plainto_tsquery('english', $3)
                   OR title ILIKE '%' || $3 || '%'
                 )
               ORDER BY key"#,
        )
        .bind(project_id)
        .bind(query.suite_id)
        .bind(q)
        .fetch_all(&state.pool)
        .await?
    } else {
        sqlx::query_as::<_, TestCase>(
            r#"SELECT id, project_id, suite_id, key, title, description, precondition,
                      priority, status, version, created_at, updated_at
               FROM test_cases
               WHERE project_id = $1
                 AND ($2::uuid IS NULL OR suite_id = $2)
               ORDER BY key"#,
        )
        .bind(project_id)
        .bind(query.suite_id)
        .fetch_all(&state.pool)
        .await?
    };

    Ok(Json(cases))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/cases",
    tag = "cases",
    params(("project_id" = Uuid, Path, description = "Project id")),
    request_body = CreateCase,
    responses((status = 200, description = "Created case", body = TestCaseDetail))
)]
pub async fn create_case(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Json(body): Json<CreateCase>,
) -> AppResult<Json<TestCaseDetail>> {
    ensure_project(&state.pool, project_id).await?;
    if body.key.trim().is_empty() || body.title.trim().is_empty() {
        return Err(AppError::BadRequest("key and title are required".into()));
    }

    let mut tx = state.pool.begin().await?;

    let case = sqlx::query_as::<_, TestCase>(
        r#"INSERT INTO test_cases
             (project_id, suite_id, key, title, description, precondition, priority, status)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
           RETURNING id, project_id, suite_id, key, title, description, precondition,
                     priority, status, version, created_at, updated_at"#,
    )
    .bind(project_id)
    .bind(body.suite_id)
    .bind(body.key.trim())
    .bind(body.title.trim())
    .bind(&body.description)
    .bind(&body.precondition)
    .bind(&body.priority)
    .bind(&body.status)
    .fetch_one(&mut *tx)
    .await?;

    let steps = replace_steps(&mut tx, case.id, &body.steps).await?;
    tx.commit().await?;

    Ok(Json(TestCaseDetail { case, steps }))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/cases/{case_id}",
    tag = "cases",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("case_id" = Uuid, Path, description = "Case id")
    ),
    responses((status = 200, description = "Case detail", body = TestCaseDetail))
)]
pub async fn get_case(
    State(state): State<AppState>,
    Path((project_id, case_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<TestCaseDetail>> {
    let case = sqlx::query_as::<_, TestCase>(
        r#"SELECT id, project_id, suite_id, key, title, description, precondition,
                  priority, status, version, created_at, updated_at
           FROM test_cases WHERE project_id = $1 AND id = $2"#,
    )
    .bind(project_id)
    .bind(case_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("case".into()))?;

    let steps = load_steps(&state.pool, case.id).await?;
    Ok(Json(TestCaseDetail { case, steps }))
}

#[utoipa::path(
    put,
    path = "/api/v1/projects/{project_id}/cases/{case_id}",
    tag = "cases",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("case_id" = Uuid, Path, description = "Case id")
    ),
    request_body = UpdateCase,
    responses((status = 200, description = "Updated case", body = TestCaseDetail))
)]
pub async fn update_case(
    State(state): State<AppState>,
    Path((project_id, case_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateCase>,
) -> AppResult<Json<TestCaseDetail>> {
    let mut tx = state.pool.begin().await?;

    let existing = sqlx::query_as::<_, TestCase>(
        r#"SELECT id, project_id, suite_id, key, title, description, precondition,
                  priority, status, version, created_at, updated_at
           FROM test_cases WHERE project_id = $1 AND id = $2"#,
    )
    .bind(project_id)
    .bind(case_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("case".into()))?;

    let title = body.title.unwrap_or(existing.title);
    let suite_id = body.suite_id.or(existing.suite_id);
    let description = body.description.unwrap_or(existing.description);
    let precondition = body.precondition.unwrap_or(existing.precondition);
    let priority = body.priority.unwrap_or(existing.priority);
    let status = body.status.unwrap_or(existing.status);

    let case = sqlx::query_as::<_, TestCase>(
        r#"UPDATE test_cases
           SET title = $3, suite_id = $4, description = $5, precondition = $6,
               priority = $7, status = $8, version = version + 1, updated_at = NOW()
           WHERE project_id = $1 AND id = $2
           RETURNING id, project_id, suite_id, key, title, description, precondition,
                     priority, status, version, created_at, updated_at"#,
    )
    .bind(project_id)
    .bind(case_id)
    .bind(title)
    .bind(suite_id)
    .bind(description)
    .bind(precondition)
    .bind(priority)
    .bind(status)
    .fetch_one(&mut *tx)
    .await?;

    let steps = if let Some(ref steps) = body.steps {
        replace_steps(&mut tx, case.id, steps).await?
    } else {
        sqlx::query_as::<_, TestStep>(
            r#"SELECT id, case_id, position, action, expected_result, data
               FROM test_steps WHERE case_id = $1 ORDER BY position"#,
        )
        .bind(case.id)
        .fetch_all(&mut *tx)
        .await?
    };

    tx.commit().await?;
    Ok(Json(TestCaseDetail { case, steps }))
}

#[utoipa::path(
    delete,
    path = "/api/v1/projects/{project_id}/cases/{case_id}",
    tag = "cases",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("case_id" = Uuid, Path, description = "Case id")
    ),
    responses((status = 204, description = "Deleted"))
)]
pub async fn delete_case(
    State(state): State<AppState>,
    Path((project_id, case_id)): Path<(Uuid, Uuid)>,
) -> AppResult<axum::http::StatusCode> {
    let result = sqlx::query("DELETE FROM test_cases WHERE project_id = $1 AND id = $2")
        .bind(project_id)
        .bind(case_id)
        .execute(&state.pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("case".into()));
    }
    Ok(axum::http::StatusCode::NO_CONTENT)
}
