use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::{CreatePlan, TestPlan, TestPlanDetail, UpdatePlan};
use crate::routes::AppState;
use crate::services::cases::ensure_project;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/projects/{project_id}/plans",
            get(list_plans).post(create_plan),
        )
        .route(
            "/api/v1/projects/{project_id}/plans/{plan_id}",
            get(get_plan).put(update_plan).delete(delete_plan),
        )
}

async fn load_plan_detail(pool: &sqlx::PgPool, plan: TestPlan) -> AppResult<TestPlanDetail> {
    let case_ids: Vec<Uuid> = sqlx::query_scalar(
        r#"SELECT case_id FROM test_plan_cases
           WHERE plan_id = $1 ORDER BY sort_order, case_id"#,
    )
    .bind(plan.id)
    .fetch_all(pool)
    .await?;
    let case_count = case_ids.len() as i64;
    Ok(TestPlanDetail {
        plan,
        case_ids,
        case_count,
    })
}

async fn replace_plan_cases(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    plan_id: Uuid,
    case_ids: &[Uuid],
) -> AppResult<()> {
    sqlx::query("DELETE FROM test_plan_cases WHERE plan_id = $1")
        .bind(plan_id)
        .execute(&mut **tx)
        .await?;
    for (idx, case_id) in case_ids.iter().enumerate() {
        sqlx::query(
            r#"INSERT INTO test_plan_cases (plan_id, case_id, sort_order)
               VALUES ($1, $2, $3)"#,
        )
        .bind(plan_id)
        .bind(case_id)
        .bind(idx as i32)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/plans",
    tag = "plans",
    params(("project_id" = Uuid, Path)),
    responses((status = 200, description = "Plans", body = [TestPlan]))
)]
pub async fn list_plans(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
) -> AppResult<Json<Vec<TestPlan>>> {
    ensure_project(&state.pool, project_id).await?;
    let plans = sqlx::query_as::<_, TestPlan>(
        r#"SELECT id, project_id, name, description, created_at, updated_at
           FROM test_plans WHERE project_id = $1 ORDER BY updated_at DESC"#,
    )
    .bind(project_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(plans))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/plans",
    tag = "plans",
    params(("project_id" = Uuid, Path)),
    request_body = CreatePlan,
    responses((status = 200, description = "Created", body = TestPlanDetail))
)]
pub async fn create_plan(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Json(body): Json<CreatePlan>,
) -> AppResult<Json<TestPlanDetail>> {
    ensure_project(&state.pool, project_id).await?;
    if body.name.trim().is_empty() {
        return Err(AppError::BadRequest("name required".into()));
    }
    let mut tx = state.pool.begin().await?;
    let plan = sqlx::query_as::<_, TestPlan>(
        r#"INSERT INTO test_plans (project_id, name, description)
           VALUES ($1, $2, $3)
           RETURNING id, project_id, name, description, created_at, updated_at"#,
    )
    .bind(project_id)
    .bind(body.name.trim())
    .bind(&body.description)
    .fetch_one(&mut *tx)
    .await?;
    replace_plan_cases(&mut tx, plan.id, &body.case_ids).await?;
    tx.commit().await?;
    Ok(Json(load_plan_detail(&state.pool, plan).await?))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/plans/{plan_id}",
    tag = "plans",
    params(("project_id" = Uuid, Path), ("plan_id" = Uuid, Path)),
    responses((status = 200, description = "Plan", body = TestPlanDetail))
)]
pub async fn get_plan(
    State(state): State<AppState>,
    Path((project_id, plan_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<TestPlanDetail>> {
    let plan = sqlx::query_as::<_, TestPlan>(
        r#"SELECT id, project_id, name, description, created_at, updated_at
           FROM test_plans WHERE project_id = $1 AND id = $2"#,
    )
    .bind(project_id)
    .bind(plan_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("plan".into()))?;
    Ok(Json(load_plan_detail(&state.pool, plan).await?))
}

#[utoipa::path(
    put,
    path = "/api/v1/projects/{project_id}/plans/{plan_id}",
    tag = "plans",
    params(("project_id" = Uuid, Path), ("plan_id" = Uuid, Path)),
    request_body = UpdatePlan,
    responses((status = 200, description = "Updated", body = TestPlanDetail))
)]
pub async fn update_plan(
    State(state): State<AppState>,
    Path((project_id, plan_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdatePlan>,
) -> AppResult<Json<TestPlanDetail>> {
    let existing = sqlx::query_as::<_, TestPlan>(
        r#"SELECT id, project_id, name, description, created_at, updated_at
           FROM test_plans WHERE project_id = $1 AND id = $2"#,
    )
    .bind(project_id)
    .bind(plan_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("plan".into()))?;

    let mut tx = state.pool.begin().await?;
    let plan = sqlx::query_as::<_, TestPlan>(
        r#"UPDATE test_plans
           SET name = $3, description = $4, updated_at = NOW()
           WHERE project_id = $1 AND id = $2
           RETURNING id, project_id, name, description, created_at, updated_at"#,
    )
    .bind(project_id)
    .bind(plan_id)
    .bind(body.name.unwrap_or(existing.name))
    .bind(body.description.unwrap_or(existing.description))
    .fetch_one(&mut *tx)
    .await?;

    if let Some(ref case_ids) = body.case_ids {
        replace_plan_cases(&mut tx, plan.id, case_ids).await?;
    }
    tx.commit().await?;
    Ok(Json(load_plan_detail(&state.pool, plan).await?))
}

#[utoipa::path(
    delete,
    path = "/api/v1/projects/{project_id}/plans/{plan_id}",
    tag = "plans",
    params(("project_id" = Uuid, Path), ("plan_id" = Uuid, Path)),
    responses((status = 204, description = "Deleted"))
)]
pub async fn delete_plan(
    State(state): State<AppState>,
    Path((project_id, plan_id)): Path<(Uuid, Uuid)>,
) -> AppResult<axum::http::StatusCode> {
    let res = sqlx::query("DELETE FROM test_plans WHERE project_id = $1 AND id = $2")
        .bind(project_id)
        .bind(plan_id)
        .execute(&state.pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound("plan".into()));
    }
    Ok(axum::http::StatusCode::NO_CONTENT)
}
