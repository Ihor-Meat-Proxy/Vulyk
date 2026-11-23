use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::{
    CreateSharedStep, CreateSharedStepLine, SharedStep, SharedStepDetail, SharedStepLine,
    SharedStepSummary, UpdateSharedStep,
};

const NAME_UNIQUE: &str = "shared_steps_project_id_name_key";

fn map_name_conflict(err: sqlx::Error) -> AppError {
    match &err {
        sqlx::Error::Database(db) if db.constraint() == Some(NAME_UNIQUE) => {
            AppError::Conflict("a shared step with this name already exists".into())
        }
        _ => err.into(),
    }
}

fn normalize_name(name: &str) -> AppResult<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("name required".into()));
    }
    Ok(name.to_string())
}

fn validate_lines(lines: &[CreateSharedStepLine]) -> AppResult<()> {
    if lines.is_empty() {
        return Err(AppError::BadRequest(
            "shared step needs at least one line".into(),
        ));
    }
    if let Some(idx) = lines.iter().position(|l| l.action.trim().is_empty()) {
        return Err(AppError::BadRequest(format!(
            "line {} action required",
            idx + 1
        )));
    }
    Ok(())
}

pub async fn list(pool: &sqlx::PgPool, project_id: Uuid) -> AppResult<Vec<SharedStepSummary>> {
    let rows = sqlx::query_as::<_, SharedStepSummary>(
        r#"SELECT s.id, s.project_id, s.name, s.description, s.created_at, s.updated_at,
                  (SELECT COUNT(*) FROM shared_step_lines l
                    WHERE l.shared_step_id = s.id) AS line_count,
                  (SELECT COUNT(DISTINCT ts.case_id) FROM test_steps ts
                    WHERE ts.shared_step_id = s.id) AS usage_count
           FROM shared_steps s
           WHERE s.project_id = $1
           ORDER BY s.name"#,
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

async fn fetch<'e, E>(executor: E, project_id: Uuid, id: Uuid) -> AppResult<SharedStep>
where
    E: sqlx::PgExecutor<'e>,
{
    sqlx::query_as::<_, SharedStep>(
        r#"SELECT id, project_id, name, description, created_at, updated_at
           FROM shared_steps WHERE project_id = $1 AND id = $2"#,
    )
    .bind(project_id)
    .bind(id)
    .fetch_optional(executor)
    .await?
    .ok_or_else(|| AppError::NotFound("shared step".into()))
}

pub async fn load_lines<'e, E>(executor: E, shared_step_id: Uuid) -> AppResult<Vec<SharedStepLine>>
where
    E: sqlx::PgExecutor<'e>,
{
    Ok(sqlx::query_as::<_, SharedStepLine>(
        r#"SELECT id, shared_step_id, position, action, expected_result, data
           FROM shared_step_lines WHERE shared_step_id = $1 ORDER BY position"#,
    )
    .bind(shared_step_id)
    .fetch_all(executor)
    .await?)
}

async fn usage_count<'e, E>(executor: E, shared_step_id: Uuid) -> AppResult<i64>
where
    E: sqlx::PgExecutor<'e>,
{
    Ok(sqlx::query_scalar(
        "SELECT COUNT(DISTINCT case_id) FROM test_steps WHERE shared_step_id = $1",
    )
    .bind(shared_step_id)
    .fetch_one(executor)
    .await?)
}

pub async fn get_detail(
    pool: &sqlx::PgPool,
    project_id: Uuid,
    id: Uuid,
) -> AppResult<SharedStepDetail> {
    let shared_step = fetch(pool, project_id, id).await?;
    let lines = load_lines(pool, id).await?;
    let usage_count = usage_count(pool, id).await?;
    Ok(SharedStepDetail {
        shared_step,
        lines,
        usage_count,
    })
}

async fn replace_lines(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    shared_step_id: Uuid,
    lines: &[CreateSharedStepLine],
) -> AppResult<()> {
    sqlx::query("DELETE FROM shared_step_lines WHERE shared_step_id = $1")
        .bind(shared_step_id)
        .execute(&mut **tx)
        .await?;
    for (idx, line) in lines.iter().enumerate() {
        sqlx::query(
            r#"INSERT INTO shared_step_lines
                 (shared_step_id, position, action, expected_result, data)
               VALUES ($1, $2, $3, $4, $5)"#,
        )
        .bind(shared_step_id)
        .bind(idx as i32)
        .bind(line.action.trim())
        .bind(&line.expected_result)
        .bind(&line.data)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

pub async fn create(
    pool: &sqlx::PgPool,
    project_id: Uuid,
    body: &CreateSharedStep,
) -> AppResult<SharedStepDetail> {
    let name = normalize_name(&body.name)?;
    validate_lines(&body.lines)?;

    let mut tx = pool.begin().await?;
    let shared_step = sqlx::query_as::<_, SharedStep>(
        r#"INSERT INTO shared_steps (project_id, name, description)
           VALUES ($1, $2, $3)
           RETURNING id, project_id, name, description, created_at, updated_at"#,
    )
    .bind(project_id)
    .bind(&name)
    .bind(body.description.trim())
    .fetch_one(&mut *tx)
    .await
    .map_err(map_name_conflict)?;
    replace_lines(&mut tx, shared_step.id, &body.lines).await?;
    tx.commit().await?;

    get_detail(pool, project_id, shared_step.id).await
}

pub async fn update(
    pool: &sqlx::PgPool,
    project_id: Uuid,
    id: Uuid,
    body: &UpdateSharedStep,
) -> AppResult<SharedStepDetail> {
    let mut tx = pool.begin().await?;
    let existing = fetch(&mut *tx, project_id, id).await?;
    let name = match body.name.as_deref() {
        Some(name) => normalize_name(name)?,
        None => existing.name,
    };
    let description = body
        .description
        .as_deref()
        .map(|d| d.trim().to_string())
        .unwrap_or(existing.description);
    if let Some(ref lines) = body.lines {
        validate_lines(lines)?;
    }

    sqlx::query(
        r#"UPDATE shared_steps SET name = $3, description = $4, updated_at = NOW()
           WHERE project_id = $1 AND id = $2"#,
    )
    .bind(project_id)
    .bind(id)
    .bind(&name)
    .bind(&description)
    .execute(&mut *tx)
    .await
    .map_err(map_name_conflict)?;
    if let Some(ref lines) = body.lines {
        replace_lines(&mut tx, id, lines).await?;
    }
    tx.commit().await?;

    get_detail(pool, project_id, id).await
}

pub async fn delete(pool: &sqlx::PgPool, project_id: Uuid, id: Uuid) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    fetch(&mut *tx, project_id, id).await?;
    let usage = usage_count(&mut *tx, id).await?;
    if usage > 0 {
        return Err(AppError::Conflict(format!(
            "shared step is in use by {usage} case(s)"
        )));
    }
    sqlx::query("DELETE FROM shared_steps WHERE project_id = $1 AND id = $2")
        .bind(project_id)
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

/// Lines to expand into case steps; rejects ids from another project.
pub async fn lines_for_expansion(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    project_id: Uuid,
    shared_step_id: Uuid,
) -> AppResult<Vec<SharedStepLine>> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM shared_steps WHERE id = $1 AND project_id = $2)",
    )
    .bind(shared_step_id)
    .bind(project_id)
    .fetch_one(&mut **tx)
    .await?;
    if !exists {
        return Err(AppError::BadRequest(
            "shared_step_id does not belong to this project".into(),
        ));
    }
    load_lines(&mut **tx, shared_step_id).await
}
