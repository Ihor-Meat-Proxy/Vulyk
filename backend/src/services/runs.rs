use sqlx::Row;
use uuid::Uuid;

use crate::error::AppResult;
use crate::models::{RunItem, RunItemView, RunProgress, TestRun};

pub async fn load_run(pool: &sqlx::PgPool, project_id: Uuid, run_id: Uuid) -> AppResult<TestRun> {
    let run = sqlx::query_as::<_, TestRun>(&format!(
        r#"SELECT {} FROM test_runs WHERE project_id = $1 AND id = $2"#,
        crate::models::TEST_RUN_COLS
    ))
    .bind(project_id)
    .bind(run_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| crate::error::AppError::NotFound("run".into()))?;
    Ok(run)
}

pub async fn run_progress(pool: &sqlx::PgPool, run_id: Uuid) -> AppResult<RunProgress> {
    let row = sqlx::query(
        r#"SELECT
             COUNT(*)::bigint AS total,
             COUNT(*) FILTER (WHERE status = 'Untested')::bigint AS untested,
             COUNT(*) FILTER (WHERE status = 'Passed')::bigint AS passed,
             COUNT(*) FILTER (WHERE status = 'Failed')::bigint AS failed,
             COUNT(*) FILTER (WHERE status = 'Blocked')::bigint AS blocked,
             COUNT(*) FILTER (WHERE status = 'Skipped')::bigint AS skipped,
             COUNT(*) FILTER (WHERE status = 'Retest')::bigint AS retest
           FROM run_items WHERE run_id = $1"#,
    )
    .bind(run_id)
    .fetch_one(pool)
    .await?;

    let total: i64 = row.try_get("total")?;
    let untested: i64 = row.try_get("untested")?;
    let passed: i64 = row.try_get("passed")?;
    let failed: i64 = row.try_get("failed")?;
    let blocked: i64 = row.try_get("blocked")?;
    let skipped: i64 = row.try_get("skipped")?;
    let retest: i64 = row.try_get("retest")?;
    let completed = total - untested;
    let percent = if total == 0 {
        0.0
    } else {
        (completed as f64 / total as f64) * 100.0
    };

    Ok(RunProgress {
        total,
        untested,
        passed,
        failed,
        blocked,
        skipped,
        retest,
        completed,
        percent,
    })
}

pub async fn load_run_items(pool: &sqlx::PgPool, run_id: Uuid) -> AppResult<Vec<RunItemView>> {
    let items = sqlx::query(
        r#"SELECT
             ri.id, ri.run_id, ri.case_id, ri.assignee, ri.status, ri.sort_order,
             ri.comment, ri.executed_at, ri.created_at, ri.updated_at,
             tc.key AS case_key, tc.title AS case_title, tc.priority AS case_priority
           FROM run_items ri
           INNER JOIN test_cases tc ON tc.id = ri.case_id
           WHERE ri.run_id = $1
           ORDER BY ri.sort_order, tc.key"#,
    )
    .bind(run_id)
    .fetch_all(pool)
    .await?;

    let mut out = Vec::with_capacity(items.len());
    for row in items {
        let item = RunItem {
            id: row.try_get("id")?,
            run_id: row.try_get("run_id")?,
            case_id: row.try_get("case_id")?,
            assignee: row.try_get("assignee")?,
            status: row.try_get("status")?,
            sort_order: row.try_get("sort_order")?,
            comment: row.try_get("comment")?,
            executed_at: row.try_get("executed_at")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        };
        out.push(RunItemView {
            item,
            case_key: row.try_get("case_key")?,
            case_title: row.try_get("case_title")?,
            case_priority: row.try_get("case_priority")?,
        });
    }
    Ok(out)
}

pub async fn collect_case_ids_for_run(
    pool: &sqlx::PgPool,
    project_id: Uuid,
    plan_id: Option<Uuid>,
    suite_ids: &[Uuid],
    case_ids: &[Uuid],
    case_statuses: &[String],
    tag_names: &[String],
) -> AppResult<Vec<Uuid>> {
    let mut ids: Vec<Uuid> = case_ids.to_vec();

    if let Some(plan_id) = plan_id {
        let plan_cases: Vec<Uuid> = sqlx::query_scalar(
            r#"SELECT tpc.case_id
               FROM test_plan_cases tpc
               INNER JOIN test_plans tp ON tp.id = tpc.plan_id
               WHERE tpc.plan_id = $1 AND tp.project_id = $2"#,
        )
        .bind(plan_id)
        .bind(project_id)
        .fetch_all(pool)
        .await?;
        for id in plan_cases {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
    }

    if !suite_ids.is_empty() {
        let suite_cases: Vec<Uuid> = sqlx::query_scalar(
            r#"SELECT id FROM test_cases
               WHERE project_id = $1 AND suite_id = ANY($2)"#,
        )
        .bind(project_id)
        .bind(suite_ids)
        .fetch_all(pool)
        .await?;
        for id in suite_cases {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
    }

    if !case_statuses.is_empty() || !tag_names.is_empty() {
        let filtered: Vec<Uuid> = sqlx::query_scalar(
            r#"SELECT DISTINCT tc.id
               FROM test_cases tc
               LEFT JOIN case_tags ct ON ct.case_id = tc.id
               LEFT JOIN tags t ON t.id = ct.tag_id
               WHERE tc.project_id = $1
                 AND (cardinality($2::text[]) = 0 OR tc.status = ANY($2))
                 AND (cardinality($3::text[]) = 0 OR t.name = ANY($3))"#,
        )
        .bind(project_id)
        .bind(case_statuses)
        .bind(tag_names)
        .fetch_all(pool)
        .await?;

        if case_ids.is_empty() && suite_ids.is_empty() && plan_id.is_none() {
            ids = filtered;
        } else {
            ids.retain(|id| filtered.contains(id));
        }
    }

    Ok(ids)
}

pub async fn all_project_case_ids(pool: &sqlx::PgPool, project_id: Uuid) -> AppResult<Vec<Uuid>> {
    Ok(
        sqlx::query_scalar(r#"SELECT id FROM test_cases WHERE project_id = $1 ORDER BY key"#)
            .bind(project_id)
            .fetch_all(pool)
            .await?,
    )
}
