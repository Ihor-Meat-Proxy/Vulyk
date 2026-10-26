//! Campaigns, iterations, dashboards, and automated-suite helpers.

use chrono::{Duration, Utc};
use serde_json::{json, Value};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::{
    to_dashboard_status, AutomatedSuite, Campaign, CaseExecutionHistoryItem,
    CaseExecutionHistoryPage, CreateCampaign, CreateIteration, ExecutionsTreeNode,
    FlakyTestSummary, Iteration, IterationDashboard, IterationItem, IterationItemView,
    IterationStatusCounts, PassRatePoint, SlowTestSummary, SuiteResultRow, TestRun, UpdateCampaign,
    UpdateIteration, BLOCKED, FAILED, PASSED, READY, RUNNING, SKIPPED, UNTESTABLE,
};

const RUN_COLS: &str = "id, project_id, plan_id, iteration_id, name, description, status, source, external_ref, meta, created_by_name, environment, artifacts, pass_count, fail_count, skip_count, blocked_count, duration_ms, started_at, finished_at, created_at, updated_at";

pub async fn list_campaigns(pool: &PgPool, project_id: Uuid) -> AppResult<Vec<Campaign>> {
    let rows = sqlx::query_as::<_, Campaign>(
        r#"SELECT id, project_id, parent_id, name, description, sort_order, created_at, updated_at
           FROM campaigns WHERE project_id = $1
           ORDER BY sort_order, name"#,
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn create_campaign(
    pool: &PgPool,
    project_id: Uuid,
    body: &CreateCampaign,
) -> AppResult<Campaign> {
    let name = body.name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("name is required".into()));
    }
    if let Some(parent_id) = body.parent_id {
        ensure_campaign(pool, project_id, parent_id).await?;
    }
    let row = sqlx::query_as::<_, Campaign>(
        r#"INSERT INTO campaigns (project_id, parent_id, name, description, sort_order)
           VALUES ($1, $2, $3, $4, $5)
           RETURNING id, project_id, parent_id, name, description, sort_order, created_at, updated_at"#,
    )
    .bind(project_id)
    .bind(body.parent_id)
    .bind(name)
    .bind(body.description.trim())
    .bind(body.sort_order)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn get_campaign(pool: &PgPool, project_id: Uuid, id: Uuid) -> AppResult<Campaign> {
    ensure_campaign(pool, project_id, id).await
}

pub async fn update_campaign(
    pool: &PgPool,
    project_id: Uuid,
    id: Uuid,
    body: &UpdateCampaign,
) -> AppResult<Campaign> {
    let current = ensure_campaign(pool, project_id, id).await?;
    let name = body
        .name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(current.name.as_str());
    let description = body
        .description
        .as_deref()
        .unwrap_or(current.description.as_str());
    let parent_id = match &body.parent_id {
        Some(v) => *v,
        None => current.parent_id,
    };
    if let Some(pid) = parent_id {
        if pid == id {
            return Err(AppError::BadRequest(
                "campaign cannot be its own parent".into(),
            ));
        }
        ensure_campaign(pool, project_id, pid).await?;
    }
    let sort_order = body.sort_order.unwrap_or(current.sort_order);
    let row = sqlx::query_as::<_, Campaign>(
        r#"UPDATE campaigns
           SET name = $3, description = $4, parent_id = $5, sort_order = $6, updated_at = NOW()
           WHERE project_id = $1 AND id = $2
           RETURNING id, project_id, parent_id, name, description, sort_order, created_at, updated_at"#,
    )
    .bind(project_id)
    .bind(id)
    .bind(name)
    .bind(description)
    .bind(parent_id)
    .bind(sort_order)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn delete_campaign(pool: &PgPool, project_id: Uuid, id: Uuid) -> AppResult<()> {
    let res = sqlx::query("DELETE FROM campaigns WHERE project_id = $1 AND id = $2")
        .bind(project_id)
        .bind(id)
        .execute(pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound("campaign".into()));
    }
    Ok(())
}

pub async fn ensure_campaign(pool: &PgPool, project_id: Uuid, id: Uuid) -> AppResult<Campaign> {
    sqlx::query_as::<_, Campaign>(
        r#"SELECT id, project_id, parent_id, name, description, sort_order, created_at, updated_at
           FROM campaigns WHERE project_id = $1 AND id = $2"#,
    )
    .bind(project_id)
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("campaign".into()))
}

pub async fn find_or_create_campaign(
    pool: &PgPool,
    project_id: Uuid,
    name: &str,
    parent_id: Option<Uuid>,
) -> AppResult<Campaign> {
    let name = name.trim();
    if let Some(existing) = sqlx::query_as::<_, Campaign>(
        r#"SELECT id, project_id, parent_id, name, description, sort_order, created_at, updated_at
           FROM campaigns
           WHERE project_id = $1 AND name = $2
             AND (($3::uuid IS NULL AND parent_id IS NULL) OR parent_id = $3)
           LIMIT 1"#,
    )
    .bind(project_id)
    .bind(name)
    .bind(parent_id)
    .fetch_optional(pool)
    .await?
    {
        return Ok(existing);
    }
    create_campaign(
        pool,
        project_id,
        &CreateCampaign {
            name: name.into(),
            description: String::new(),
            parent_id,
            sort_order: 0,
        },
    )
    .await
}

pub async fn list_iterations(
    pool: &PgPool,
    project_id: Uuid,
    campaign_id: Option<Uuid>,
) -> AppResult<Vec<Iteration>> {
    let rows = if let Some(cid) = campaign_id {
        sqlx::query_as::<_, Iteration>(
            r#"SELECT id, project_id, campaign_id, name, reference, description, status, mode,
                      cadence, scheduled_start, scheduled_end, created_at, updated_at
               FROM iterations WHERE project_id = $1 AND campaign_id = $2
               ORDER BY created_at DESC, name"#,
        )
        .bind(project_id)
        .bind(cid)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, Iteration>(
            r#"SELECT id, project_id, campaign_id, name, reference, description, status, mode,
                      cadence, scheduled_start, scheduled_end, created_at, updated_at
               FROM iterations WHERE project_id = $1
               ORDER BY created_at DESC, name"#,
        )
        .bind(project_id)
        .fetch_all(pool)
        .await?
    };
    Ok(rows)
}

pub async fn create_iteration(
    pool: &PgPool,
    project_id: Uuid,
    campaign_id: Uuid,
    body: &CreateIteration,
) -> AppResult<Iteration> {
    ensure_campaign(pool, project_id, campaign_id).await?;
    let name = body.name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("name is required".into()));
    }
    let mut tx = pool.begin().await?;
    let iteration = sqlx::query_as::<_, Iteration>(
        r#"INSERT INTO iterations
             (project_id, campaign_id, name, reference, description, status, mode, cadence,
              scheduled_start, scheduled_end)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
           RETURNING id, project_id, campaign_id, name, reference, description, status, mode,
                     cadence, scheduled_start, scheduled_end, created_at, updated_at"#,
    )
    .bind(project_id)
    .bind(campaign_id)
    .bind(name)
    .bind(body.reference.trim())
    .bind(body.description.trim())
    .bind(body.status.trim())
    .bind(body.mode.trim())
    .bind(body.cadence.trim())
    .bind(body.scheduled_start)
    .bind(body.scheduled_end)
    .fetch_one(&mut *tx)
    .await?;

    let mut case_ids = body.case_ids.clone();
    if !body.suite_ids.is_empty() {
        let suite_cases: Vec<Uuid> = sqlx::query_scalar(
            r#"SELECT id FROM test_cases
               WHERE project_id = $1 AND suite_id = ANY($2)"#,
        )
        .bind(project_id)
        .bind(&body.suite_ids)
        .fetch_all(&mut *tx)
        .await?;
        case_ids.extend(suite_cases);
    }
    case_ids.sort_unstable();
    case_ids.dedup();

    for (idx, case_id) in case_ids.iter().enumerate() {
        sqlx::query(
            r#"INSERT INTO iteration_items (iteration_id, case_id, status, sort_order)
               VALUES ($1, $2, 'Ready', $3)
               ON CONFLICT (iteration_id, case_id) DO NOTHING"#,
        )
        .bind(iteration.id)
        .bind(case_id)
        .bind(idx as i32)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(iteration)
}

pub async fn get_iteration(pool: &PgPool, project_id: Uuid, id: Uuid) -> AppResult<Iteration> {
    sqlx::query_as::<_, Iteration>(
        r#"SELECT id, project_id, campaign_id, name, reference, description, status, mode,
                  cadence, scheduled_start, scheduled_end, created_at, updated_at
           FROM iterations WHERE project_id = $1 AND id = $2"#,
    )
    .bind(project_id)
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("iteration".into()))
}

pub async fn update_iteration(
    pool: &PgPool,
    project_id: Uuid,
    id: Uuid,
    body: &UpdateIteration,
) -> AppResult<Iteration> {
    let current = get_iteration(pool, project_id, id).await?;
    let campaign_id = if let Some(cid) = body.campaign_id {
        ensure_campaign(pool, project_id, cid).await?;
        cid
    } else {
        current.campaign_id
    };
    let name = body
        .name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(current.name.as_str());
    let reference = body
        .reference
        .as_deref()
        .unwrap_or(current.reference.as_str());
    let description = body
        .description
        .as_deref()
        .unwrap_or(current.description.as_str());
    let status = body.status.as_deref().unwrap_or(current.status.as_str());
    let mode = body.mode.as_deref().unwrap_or(current.mode.as_str());
    let cadence = body.cadence.as_deref().unwrap_or(current.cadence.as_str());
    let scheduled_start = match &body.scheduled_start {
        Some(v) => *v,
        None => current.scheduled_start,
    };
    let scheduled_end = match &body.scheduled_end {
        Some(v) => *v,
        None => current.scheduled_end,
    };

    let row = sqlx::query_as::<_, Iteration>(
        r#"UPDATE iterations
           SET campaign_id = $3, name = $4, reference = $5, description = $6, status = $7,
               mode = $8, cadence = $9, scheduled_start = $10, scheduled_end = $11,
               updated_at = NOW()
           WHERE project_id = $1 AND id = $2
           RETURNING id, project_id, campaign_id, name, reference, description, status, mode,
                     cadence, scheduled_start, scheduled_end, created_at, updated_at"#,
    )
    .bind(project_id)
    .bind(id)
    .bind(campaign_id)
    .bind(name)
    .bind(reference)
    .bind(description)
    .bind(status)
    .bind(mode)
    .bind(cadence)
    .bind(scheduled_start)
    .bind(scheduled_end)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn delete_iteration(pool: &PgPool, project_id: Uuid, id: Uuid) -> AppResult<()> {
    let res = sqlx::query("DELETE FROM iterations WHERE project_id = $1 AND id = $2")
        .bind(project_id)
        .bind(id)
        .execute(pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound("iteration".into()));
    }
    Ok(())
}

pub async fn find_or_create_iteration(
    pool: &PgPool,
    project_id: Uuid,
    campaign_id: Uuid,
    name: &str,
    reference: &str,
    cadence: &str,
) -> AppResult<Iteration> {
    if let Some(existing) = sqlx::query_as::<_, Iteration>(
        r#"SELECT id, project_id, campaign_id, name, reference, description, status, mode,
                  cadence, scheduled_start, scheduled_end, created_at, updated_at
           FROM iterations
           WHERE project_id = $1 AND campaign_id = $2 AND name = $3
           LIMIT 1"#,
    )
    .bind(project_id)
    .bind(campaign_id)
    .bind(name.trim())
    .fetch_optional(pool)
    .await?
    {
        return Ok(existing);
    }
    create_iteration(
        pool,
        project_id,
        campaign_id,
        &CreateIteration {
            name: name.into(),
            reference: reference.into(),
            description: String::new(),
            status: "Running".into(),
            mode: "automated".into(),
            cadence: cadence.into(),
            scheduled_start: None,
            scheduled_end: None,
            case_ids: vec![],
            suite_ids: vec![],
        },
    )
    .await
}

pub async fn executions_tree(
    pool: &PgPool,
    project_id: Uuid,
) -> AppResult<Vec<ExecutionsTreeNode>> {
    let campaigns = list_campaigns(pool, project_id).await?;
    let iterations = list_iterations(pool, project_id, None).await?;

    fn build(
        parent_id: Option<Uuid>,
        campaigns: &[Campaign],
        iterations: &[Iteration],
    ) -> Vec<ExecutionsTreeNode> {
        let mut nodes: Vec<ExecutionsTreeNode> = campaigns
            .iter()
            .filter(|c| c.parent_id == parent_id)
            .map(|c| {
                let mut children = build(Some(c.id), campaigns, iterations);
                children.extend(
                    iterations
                        .iter()
                        .filter(|i| i.campaign_id == c.id)
                        .map(|i| ExecutionsTreeNode {
                            id: i.id,
                            kind: "iteration".into(),
                            name: i.name.clone(),
                            reference: Some(i.reference.clone()).filter(|s| !s.is_empty()),
                            status: Some(i.status.clone()),
                            mode: Some(i.mode.clone()),
                            parent_id: Some(c.id),
                            children: vec![],
                        }),
                );
                ExecutionsTreeNode {
                    id: c.id,
                    kind: "campaign".into(),
                    name: c.name.clone(),
                    reference: None,
                    status: None,
                    mode: None,
                    parent_id: c.parent_id,
                    children,
                }
            })
            .collect();
        nodes.sort_by(|a, b| a.name.cmp(&b.name));
        nodes
    }

    Ok(build(None, &campaigns, &iterations))
}

pub async fn iteration_status_counts(
    pool: &PgPool,
    iteration_id: Uuid,
) -> AppResult<IterationStatusCounts> {
    let row = sqlx::query(
        r#"SELECT
             COUNT(*)::bigint AS total,
             COUNT(*) FILTER (WHERE status IN ('Ready', 'Untested', 'Retest'))::bigint AS ready,
             COUNT(*) FILTER (WHERE status = 'Running')::bigint AS running,
             COUNT(*) FILTER (WHERE status = 'Passed')::bigint AS passed,
             COUNT(*) FILTER (WHERE status = 'Failed')::bigint AS failed,
             COUNT(*) FILTER (WHERE status = 'Blocked')::bigint AS blocked,
             COUNT(*) FILTER (WHERE status = 'Skipped')::bigint AS skipped,
             COUNT(*) FILTER (WHERE status = 'Untestable')::bigint AS untestable
           FROM iteration_items WHERE iteration_id = $1"#,
    )
    .bind(iteration_id)
    .fetch_one(pool)
    .await?;
    Ok(IterationStatusCounts {
        ready: row.try_get("ready")?,
        running: row.try_get("running")?,
        passed: row.try_get("passed")?,
        failed: row.try_get("failed")?,
        blocked: row.try_get("blocked")?,
        skipped: row.try_get("skipped")?,
        untestable: row.try_get("untestable")?,
        total: row.try_get("total")?,
    })
}

pub async fn iteration_dashboard(
    pool: &PgPool,
    project_id: Uuid,
    iteration_id: Uuid,
) -> AppResult<IterationDashboard> {
    let iteration = get_iteration(pool, project_id, iteration_id).await?;
    let campaign = ensure_campaign(pool, project_id, iteration.campaign_id).await?;
    let campaign_path = campaign_path(pool, project_id, campaign.id).await?;
    let status_counts = iteration_status_counts(pool, iteration_id).await?;
    let pass_rate_trend = pass_rate_trend(pool, iteration_id, 15).await?;
    let flaky_tests = flaky_tests(pool, iteration_id, 10).await?;
    let slowest_tests = slowest_tests(pool, iteration_id, 10).await?;
    Ok(IterationDashboard {
        iteration,
        campaign_name: campaign.name,
        campaign_path,
        status_counts,
        pass_rate_trend,
        flaky_tests,
        slowest_tests,
    })
}

async fn campaign_path(
    pool: &PgPool,
    project_id: Uuid,
    campaign_id: Uuid,
) -> AppResult<Vec<String>> {
    let mut path = Vec::new();
    let mut current = Some(campaign_id);
    while let Some(id) = current {
        let c = ensure_campaign(pool, project_id, id).await?;
        path.push(c.name);
        current = c.parent_id;
    }
    path.reverse();
    Ok(path)
}

async fn pass_rate_trend(
    pool: &PgPool,
    iteration_id: Uuid,
    limit: i64,
) -> AppResult<Vec<PassRatePoint>> {
    let rows = sqlx::query(
        r#"SELECT id, name, created_at, pass_count, fail_count, skip_count, blocked_count
           FROM test_runs
           WHERE iteration_id = $1
           ORDER BY created_at DESC
           LIMIT $2"#,
    )
    .bind(iteration_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows.into_iter().rev() {
        let passed: i32 = row.try_get("pass_count")?;
        let failed: i32 = row.try_get("fail_count")?;
        let skipped: i32 = row.try_get("skip_count")?;
        let blocked: i32 = row.try_get("blocked_count")?;
        let total = (passed + failed + skipped + blocked) as i64;
        let pass_rate = if total > 0 {
            (passed as f64) * 100.0 / total as f64
        } else {
            0.0
        };
        out.push(PassRatePoint {
            run_id: row.try_get("id")?,
            run_name: row.try_get("name")?,
            created_at: row.try_get("created_at")?,
            pass_rate,
            passed: passed as i64,
            failed: failed as i64,
            total,
        });
    }
    Ok(out)
}

async fn flaky_tests(
    pool: &PgPool,
    iteration_id: Uuid,
    limit: i64,
) -> AppResult<Vec<FlakyTestSummary>> {
    let rows = sqlx::query(
        r#"WITH recent AS (
             SELECT tr.case_id, tr.status, tr.executed_at, r.created_at,
                    ROW_NUMBER() OVER (PARTITION BY tr.case_id ORDER BY r.created_at DESC) AS rn
             FROM test_results tr
             JOIN test_runs r ON r.id = tr.run_id
             WHERE r.iteration_id = $1 AND tr.case_id IS NOT NULL
           ),
           seq AS (
             SELECT case_id, status, rn,
                    LAG(status) OVER (PARTITION BY case_id ORDER BY rn DESC) AS prev
             FROM recent WHERE rn <= 8
           ),
           flips AS (
             SELECT case_id,
                    COUNT(*) FILTER (WHERE prev IS NOT NULL AND prev <> status)::bigint AS flips,
                    (ARRAY_AGG(status ORDER BY rn))[1] AS latest_status,
                    ARRAY_AGG(status ORDER BY rn) AS statuses
             FROM seq
             GROUP BY case_id
             HAVING COUNT(*) FILTER (WHERE prev IS NOT NULL AND prev <> status) > 0
           )
           SELECT f.case_id, tc.key AS case_key, tc.title AS case_title,
                  f.flips, f.latest_status, f.statuses
           FROM flips f
           JOIN test_cases tc ON tc.id = f.case_id
           ORDER BY f.flips DESC, tc.key
           LIMIT $2"#,
    )
    .bind(iteration_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    let mut out = Vec::new();
    for row in rows {
        let statuses: Vec<String> = row.try_get("statuses")?;
        out.push(FlakyTestSummary {
            case_id: row.try_get("case_id")?,
            case_key: row.try_get("case_key")?,
            case_title: row.try_get("case_title")?,
            flips: row.try_get("flips")?,
            latest_status: row.try_get("latest_status")?,
            statuses,
        });
    }
    Ok(out)
}

async fn slowest_tests(
    pool: &PgPool,
    iteration_id: Uuid,
    limit: i64,
) -> AppResult<Vec<SlowTestSummary>> {
    let rows = sqlx::query(
        r#"SELECT DISTINCT ON (tr.case_id)
             tr.case_id, tc.key AS case_key, tc.title AS case_title,
             COALESCE(tr.duration_ms, 0)::bigint AS duration_ms, tr.status
           FROM test_results tr
           JOIN test_runs r ON r.id = tr.run_id
           JOIN test_cases tc ON tc.id = tr.case_id
           WHERE r.iteration_id = $1 AND tr.case_id IS NOT NULL
           ORDER BY tr.case_id, tr.executed_at DESC"#,
    )
    .bind(iteration_id)
    .fetch_all(pool)
    .await?;

    let mut items: Vec<SlowTestSummary> = rows
        .into_iter()
        .map(|row| SlowTestSummary {
            case_id: row.get("case_id"),
            case_key: row.get("case_key"),
            case_title: row.get("case_title"),
            duration_ms: row.get("duration_ms"),
            status: row.get("status"),
        })
        .collect();
    items.sort_by_key(|a| std::cmp::Reverse(a.duration_ms));
    items.truncate(limit as usize);
    Ok(items)
}

pub async fn list_iteration_items(
    pool: &PgPool,
    iteration_id: Uuid,
) -> AppResult<Vec<IterationItemView>> {
    let rows = sqlx::query(
        r#"SELECT ii.id, ii.iteration_id, ii.case_id, ii.status, ii.sort_order, ii.updated_at,
                  tc.key AS case_key, tc.title AS case_title, tc.priority AS case_priority
           FROM iteration_items ii
           JOIN test_cases tc ON tc.id = ii.case_id
           WHERE ii.iteration_id = $1
           ORDER BY ii.sort_order, tc.key"#,
    )
    .bind(iteration_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| IterationItemView {
            item: IterationItem {
                id: row.get("id"),
                iteration_id: row.get("iteration_id"),
                case_id: row.get("case_id"),
                status: row.get("status"),
                sort_order: row.get("sort_order"),
                updated_at: row.get("updated_at"),
            },
            case_key: row.get("case_key"),
            case_title: row.get("case_title"),
            case_priority: row.get("case_priority"),
        })
        .collect())
}

#[allow(clippy::too_many_arguments)]
pub async fn list_automated_suites(
    pool: &PgPool,
    project_id: Uuid,
    iteration_id: Uuid,
    status: Option<&str>,
    created_by: Option<&str>,
    q: Option<&str>,
    limit: i64,
    offset: i64,
    sort: &str,
) -> AppResult<(Vec<AutomatedSuite>, i64)> {
    let order = match sort {
        "created_at_asc" => "created_at ASC",
        "name_asc" => "name ASC",
        "name_desc" => "name DESC",
        "status_asc" => "status ASC",
        _ => "created_at DESC",
    };
    let total: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)::bigint FROM test_runs
           WHERE project_id = $1 AND iteration_id = $2
             AND ($3::text IS NULL OR status ILIKE $3)
             AND ($4::text IS NULL OR COALESCE(created_by_name, '') ILIKE '%' || $4 || '%')
             AND ($5::text IS NULL OR name ILIKE '%' || $5 || '%')"#,
    )
    .bind(project_id)
    .bind(iteration_id)
    .bind(status)
    .bind(created_by)
    .bind(q)
    .fetch_one(pool)
    .await?;

    let sql = format!(
        r#"SELECT {RUN_COLS}
           FROM test_runs
           WHERE project_id = $1 AND iteration_id = $2
             AND ($3::text IS NULL OR status ILIKE $3)
             AND ($4::text IS NULL OR COALESCE(created_by_name, '') ILIKE '%' || $4 || '%')
             AND ($5::text IS NULL OR name ILIKE '%' || $5 || '%')
           ORDER BY {order}
           LIMIT $6 OFFSET $7"#
    );
    let runs = sqlx::query_as::<_, TestRun>(&sql)
        .bind(project_id)
        .bind(iteration_id)
        .bind(status)
        .bind(created_by)
        .bind(q)
        .bind(limit.clamp(1, 200))
        .bind(offset.max(0))
        .fetch_all(pool)
        .await?;

    Ok((runs.into_iter().map(suite_from_run).collect(), total))
}

fn suite_from_run(run: TestRun) -> AutomatedSuite {
    AutomatedSuite {
        id: run.id,
        project_id: run.project_id,
        iteration_id: run.iteration_id,
        name: run.name,
        status: run.status,
        source: run.source,
        created_by_name: run.created_by_name,
        created_at: run.created_at,
        started_at: run.started_at,
        finished_at: run.finished_at,
        duration_ms: run.duration_ms,
        pass_count: run.pass_count,
        fail_count: run.fail_count,
        skip_count: run.skip_count,
        blocked_count: run.blocked_count,
        environment: run.environment,
        artifacts: run.artifacts,
        external_ref: run.external_ref,
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn create_automated_suite_run(
    pool: &PgPool,
    project_id: Uuid,
    iteration_id: Uuid,
    name: &str,
    source: &str,
    created_by_name: Option<&str>,
    external_ref: Option<&str>,
    environment: Value,
    artifacts: Value,
) -> AppResult<TestRun> {
    get_iteration(pool, project_id, iteration_id).await?;
    let run = sqlx::query_as::<_, TestRun>(&format!(
        r#"INSERT INTO test_runs
             (project_id, iteration_id, name, description, status, source, external_ref,
              created_by_name, environment, artifacts, started_at)
           VALUES ($1, $2, $3, '', 'running', $4, $5, $6, $7, $8, NOW())
           RETURNING {RUN_COLS}"#
    ))
    .bind(project_id)
    .bind(iteration_id)
    .bind(name)
    .bind(source)
    .bind(external_ref)
    .bind(created_by_name)
    .bind(environment)
    .bind(artifacts)
    .fetch_one(pool)
    .await?;
    Ok(run)
}

pub async fn refresh_run_counters(pool: &PgPool, run_id: Uuid) -> AppResult<()> {
    sqlx::query(
        r#"UPDATE test_runs r SET
             pass_count = COALESCE((
               SELECT COUNT(*) FILTER (WHERE status = 'Passed') FROM test_results WHERE run_id = r.id
             ), 0),
             fail_count = COALESCE((
               SELECT COUNT(*) FILTER (WHERE status = 'Failed') FROM test_results WHERE run_id = r.id
             ), 0),
             skip_count = COALESCE((
               SELECT COUNT(*) FILTER (WHERE status = 'Skipped') FROM test_results WHERE run_id = r.id
             ), 0),
             blocked_count = COALESCE((
               SELECT COUNT(*) FILTER (WHERE status IN ('Blocked', 'Untestable'))
               FROM test_results WHERE run_id = r.id
             ), 0),
             duration_ms = COALESCE((
               SELECT SUM(COALESCE(duration_ms, 0))::bigint FROM test_results WHERE run_id = r.id
             ), 0),
             updated_at = NOW()
           WHERE id = $1"#,
    )
    .bind(run_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Sync iteration_items from the latest result per case in this iteration's runs.
pub async fn sync_iteration_item_statuses(pool: &PgPool, iteration_id: Uuid) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO iteration_items (iteration_id, case_id, status, sort_order)
           SELECT $1, tr.case_id, 'Ready', 0
           FROM test_results tr
           JOIN test_runs r ON r.id = tr.run_id
           WHERE r.iteration_id = $1 AND tr.case_id IS NOT NULL
           ON CONFLICT (iteration_id, case_id) DO NOTHING"#,
    )
    .bind(iteration_id)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"WITH latest AS (
             SELECT DISTINCT ON (tr.case_id) tr.case_id, tr.status
             FROM test_results tr
             JOIN test_runs r ON r.id = tr.run_id
             WHERE r.iteration_id = $1 AND tr.case_id IS NOT NULL
             ORDER BY tr.case_id, tr.executed_at DESC
           )
           UPDATE iteration_items ii
           SET status = CASE
                 WHEN latest.status IN ('Untested', 'Retest') THEN 'Ready'
                 ELSE latest.status
               END,
               updated_at = NOW()
           FROM latest
           WHERE ii.iteration_id = $1 AND ii.case_id = latest.case_id"#,
    )
    .bind(iteration_id)
    .execute(pool)
    .await?;

    // Mark iteration Running/Completed based on item statuses.
    let counts = iteration_status_counts(pool, iteration_id).await?;
    let status = if counts.total == 0 {
        "Undefined"
    } else if counts.ready + counts.running == counts.total {
        if counts.running > 0 {
            "Running"
        } else {
            "Ready"
        }
    } else if counts.ready + counts.running > 0 {
        "Running"
    } else {
        "Completed"
    };
    sqlx::query(r#"UPDATE iterations SET status = $2, updated_at = NOW() WHERE id = $1"#)
        .bind(iteration_id)
        .bind(status)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list_suite_results(
    pool: &PgPool,
    run_id: Uuid,
    limit: i64,
    offset: i64,
) -> AppResult<(Vec<SuiteResultRow>, i64)> {
    let total: i64 =
        sqlx::query_scalar(r#"SELECT COUNT(*)::bigint FROM test_results WHERE run_id = $1"#)
            .bind(run_id)
            .fetch_one(pool)
            .await?;

    let rows = sqlx::query(
        r#"SELECT tr.id AS result_id, tr.case_id, tc.key AS case_key, tc.title AS case_title,
                  tr.status, tr.message, tr.environment, tr.environment_vars, tr.dataset,
                  tr.executed_at, tr.duration_ms, tr.external_id,
                  (SELECT COUNT(*)::bigint FROM attachments a
                     WHERE a.entity_type = 'result' AND a.entity_id = tr.id) AS attachment_count
           FROM test_results tr
           LEFT JOIN test_cases tc ON tc.id = tr.case_id
           WHERE tr.run_id = $1
           ORDER BY tr.executed_at DESC, tr.id
           LIMIT $2 OFFSET $3"#,
    )
    .bind(run_id)
    .bind(limit.clamp(1, 200))
    .bind(offset.max(0))
    .fetch_all(pool)
    .await?;

    let items = rows
        .into_iter()
        .map(|row| SuiteResultRow {
            result_id: row.get("result_id"),
            case_id: row.get("case_id"),
            case_key: row.get("case_key"),
            case_title: row.get("case_title"),
            status: row.get("status"),
            message: row.get("message"),
            environment: row.get("environment"),
            environment_vars: row.get("environment_vars"),
            dataset: row.get("dataset"),
            executed_at: row.get("executed_at"),
            duration_ms: row.get("duration_ms"),
            attachment_count: row.get("attachment_count"),
            external_id: row.get("external_id"),
        })
        .collect();
    Ok((items, total))
}

pub async fn case_execution_history(
    pool: &PgPool,
    project_id: Uuid,
    case_id: Uuid,
    limit: i64,
    offset: i64,
) -> AppResult<CaseExecutionHistoryPage> {
    let total: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)::bigint
           FROM test_results tr
           JOIN test_runs r ON r.id = tr.run_id
           WHERE r.project_id = $1 AND tr.case_id = $2"#,
    )
    .bind(project_id)
    .bind(case_id)
    .fetch_one(pool)
    .await?;

    let rows = sqlx::query(
        r#"SELECT tr.id AS result_id, tr.status, tr.executed_at, tr.duration_ms, tr.run_id,
                  r.iteration_id, r.source, r.name AS run_name, r.created_by_name,
                  i.name AS iteration_name, c.name AS campaign_name,
                  COALESCE(u.display_name, r.created_by_name) AS executed_by,
                  (SELECT COUNT(*)::bigint FROM defects d WHERE d.result_id = tr.id) AS linked_issues,
                  ROW_NUMBER() OVER (PARTITION BY tr.run_id ORDER BY tr.executed_at) AS exec_number
           FROM test_results tr
           JOIN test_runs r ON r.id = tr.run_id
           LEFT JOIN iterations i ON i.id = r.iteration_id
           LEFT JOIN campaigns c ON c.id = i.campaign_id
           LEFT JOIN users u ON u.id = tr.executed_by
           WHERE r.project_id = $1 AND tr.case_id = $2
           ORDER BY tr.executed_at DESC
           LIMIT $3 OFFSET $4"#,
    )
    .bind(project_id)
    .bind(case_id)
    .bind(limit.clamp(1, 200))
    .bind(offset.max(0))
    .fetch_all(pool)
    .await?;

    let spark_rows = sqlx::query(
        r#"SELECT tr.status
           FROM test_results tr
           JOIN test_runs r ON r.id = tr.run_id
           WHERE r.project_id = $1 AND tr.case_id = $2
           ORDER BY tr.executed_at ASC
           LIMIT 40"#,
    )
    .bind(project_id)
    .bind(case_id)
    .fetch_all(pool)
    .await?;

    let sparkline: Vec<String> = spark_rows
        .iter()
        .map(|r| to_dashboard_status(&r.get::<String, _>("status")).to_string())
        .collect();
    let passed = sparkline.iter().filter(|s| s.as_str() == PASSED).count() as f64;
    let denom = sparkline
        .iter()
        .filter(|s| matches!(s.as_str(), PASSED | FAILED | BLOCKED | SKIPPED | UNTESTABLE))
        .count() as f64;
    let pass_rate = if denom > 0.0 {
        (passed * 100.0) / denom
    } else {
        0.0
    };

    let items = rows
        .into_iter()
        .map(|row| {
            let campaign: Option<String> = row.try_get("campaign_name").ok();
            let iteration: Option<String> = row.try_get("iteration_name").ok();
            let run_name: String = row.get("run_name");
            let path = match (campaign, iteration) {
                (Some(c), Some(i)) => format!("{c} > {i}"),
                (Some(c), None) => c,
                _ => run_name.clone(),
            };
            let source: String = row.get("source");
            let mode = if source == "manual" || source == "sample" {
                "manual"
            } else {
                "automated"
            };
            CaseExecutionHistoryItem {
                result_id: row.get("result_id"),
                path,
                plan_type: "Iteration".into(),
                exec_number: row.get("exec_number"),
                mode: mode.into(),
                status: row.get("status"),
                executed_by: row.get("executed_by"),
                executed_on: row.get("executed_at"),
                linked_issues: row.get("linked_issues"),
                run_id: row.get("run_id"),
                iteration_id: row.get("iteration_id"),
                duration_ms: row.get("duration_ms"),
            }
        })
        .collect();

    Ok(CaseExecutionHistoryPage {
        items,
        total,
        limit,
        offset,
        pass_rate,
        sparkline,
    })
}

/// Seed helper: create a realistic historical automated suite under an iteration.
#[allow(clippy::too_many_arguments)]
pub async fn seed_historical_suite(
    pool: &PgPool,
    project_id: Uuid,
    iteration_id: Uuid,
    name: &str,
    created_by_name: &str,
    days_ago: i64,
    case_results: &[(Uuid, &str, i32)],
    environment: Value,
    artifacts: Value,
) -> AppResult<Uuid> {
    let created_at = Utc::now() - Duration::days(days_ago) - Duration::minutes(days_ago * 7);
    let finished_at = created_at + Duration::minutes(12);
    let mut pass = 0i32;
    let mut fail = 0i32;
    let mut skip = 0i32;
    let mut blocked = 0i32;
    let mut duration = 0i64;
    for (_, status, ms) in case_results {
        duration += i64::from(*ms);
        match to_dashboard_status(status) {
            PASSED => pass += 1,
            FAILED => fail += 1,
            SKIPPED => skip += 1,
            BLOCKED | UNTESTABLE => blocked += 1,
            READY | RUNNING => {}
            _ => {}
        }
    }
    let status = if fail > 0 { "failed" } else { "completed" };

    let run_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO test_runs
             (project_id, iteration_id, name, description, status, source, created_by_name,
              environment, artifacts, pass_count, fail_count, skip_count, blocked_count,
              duration_ms, started_at, finished_at, created_at, updated_at)
           VALUES ($1, $2, $3, '', $4, 'ci', $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $13, $14)
           RETURNING id"#,
    )
    .bind(project_id)
    .bind(iteration_id)
    .bind(name)
    .bind(status)
    .bind(created_by_name)
    .bind(&environment)
    .bind(&artifacts)
    .bind(pass)
    .bind(fail)
    .bind(skip)
    .bind(blocked)
    .bind(duration)
    .bind(created_at)
    .bind(finished_at)
    .fetch_one(pool)
    .await?;

    for (idx, (case_id, status, ms)) in case_results.iter().enumerate() {
        let executed_at = created_at + Duration::seconds((idx as i64) * 15);
        sqlx::query(
            r#"INSERT INTO test_results
                 (run_id, case_id, status, message, source, duration_ms, executed_at,
                  environment, environment_vars, dataset, raw)
               VALUES ($1, $2, $3, $4, 'ci', $5, $6, $7, $8, $9, $10)"#,
        )
        .bind(run_id)
        .bind(case_id)
        .bind(*status)
        .bind(if *status == FAILED {
            "AssertionError: expected success"
        } else {
            ""
        })
        .bind(ms)
        .bind(executed_at)
        .bind(
            environment
                .get("runner")
                .and_then(|v| v.as_str())
                .unwrap_or("Playwright"),
        )
        .bind(json!({"BROWSER": "chromium"}))
        .bind(Option::<String>::None)
        .bind(json!({"seed": true}))
        .execute(pool)
        .await?;
    }

    sync_iteration_item_statuses(pool, iteration_id).await?;
    Ok(run_id)
}
