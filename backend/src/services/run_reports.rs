//! Run report drill-down: filtered/paginated case results, grouping, coverage detail.

use serde_json::Value;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::{
    CoverageDetailRow, CoverageLinkedCase, RequirementCoverageDetail, RunCaseResultExpand,
    RunCaseResultRow, RunCaseResultsPage, RunGroupsQuery, RunReportDetail, RunResultAttachment,
    RunResultCommentEntry, RunResultDefect, RunResultGroup, RunResultGroups, RunResultStepRow,
    RunResultsQuery, StatusCounts,
};
use crate::services::reports::{self, empty_counts, from_rows, CountRow};

fn json_str(v: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(s) = v
            .get(*key)
            .and_then(|x| x.as_str())
            .filter(|s| !s.is_empty())
        {
            return Some(s.to_string());
        }
    }
    None
}

fn source_kind(source: &str) -> String {
    let s = source.to_ascii_lowercase();
    if s == "manual" {
        "manual".into()
    } else if s.contains("ci")
        || s.contains("github")
        || s.contains("gitlab")
        || s.contains("jenkins")
        || s.contains("pipeline")
    {
        "ci".into()
    } else {
        "automated".into()
    }
}

fn extract_pipeline_url(
    meta: &Value,
    artifacts: &Value,
    external_ref: Option<&str>,
) -> Option<String> {
    if let Some(u) = json_str(
        meta,
        &[
            "pipeline_url",
            "ci_url",
            "html_url",
            "url",
            "pipelineUrl",
            "web_url",
        ],
    ) {
        return Some(u);
    }
    if let Some(arr) = artifacts.as_array() {
        for a in arr {
            let kind = a
                .get("kind")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if kind == "pipeline" || kind == "ci" || kind == "report" {
                if let Some(u) = a.get("url").and_then(|v| v.as_str()) {
                    return Some(u.to_string());
                }
            }
        }
        if let Some(u) = arr
            .iter()
            .find_map(|a| a.get("url").and_then(|v| v.as_str()))
        {
            return Some(u.to_string());
        }
    }
    external_ref
        .filter(|r| r.starts_with("http://") || r.starts_with("https://"))
        .map(|s| s.to_string())
}

fn environment_label(env: &Value) -> Option<String> {
    if let Some(s) = env.as_str().filter(|s| !s.is_empty()) {
        return Some(s.to_string());
    }
    if let Some(s) = json_str(env, &["name", "label", "env", "environment"]) {
        return Some(s);
    }
    let parts: Vec<String> = ["runner", "browser", "os", "node"]
        .iter()
        .filter_map(|k| {
            env.get(*k)
                .and_then(|v| v.as_str())
                .map(|s| format!("{k}={s}"))
        })
        .collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(", "))
    }
}

fn comparison(prev: Option<&str>, curr: &str) -> Option<String> {
    let curr_l = curr.to_ascii_lowercase();
    let prev_l = prev.map(|s| s.to_ascii_lowercase());
    match (prev_l.as_deref(), curr_l.as_str()) {
        (Some("passed"), "failed") | (Some("passed"), "error") => Some("newly_failing".into()),
        (Some("failed"), "passed") | (Some("error"), "passed") => Some("fixed".into()),
        (Some(p), c) if p != c && is_decided(p) && is_decided(c) => Some("flaky".into()),
        (Some(p), c) if p == c => Some("unchanged".into()),
        _ => None,
    }
}

fn is_decided(s: &str) -> bool {
    matches!(s, "passed" | "failed" | "error")
}

fn parse_sort(sort: Option<&str>) -> (&'static str, bool) {
    let raw = sort.unwrap_or("key_asc").to_ascii_lowercase();
    let field = raw.trim_end_matches("_asc").trim_end_matches("_desc");
    let col = match field {
        "title" => "case_title",
        "status" => "status",
        "duration" => "duration_ms",
        "folder" => "folder_path",
        "priority" => "priority",
        "assignee" => "assignee",
        _ => "case_key",
    };
    let descending = if raw.ends_with("_asc") {
        false
    } else if raw.ends_with("_desc") {
        true
    } else {
        field == "duration"
    };
    (col, descending)
}

/// Shared filter args extracted from query.
struct Filters {
    status: Option<String>,
    folder: Option<String>,
    suite_id: Option<Uuid>,
    priority: Option<String>,
    assignee: Option<String>,
    q: Option<String>,
}

impl Filters {
    fn from_results(q: &RunResultsQuery) -> Self {
        Self {
            status: normalize_opt_status(q.status.as_deref()),
            folder: q.folder.clone().filter(|s| !s.trim().is_empty()),
            suite_id: q.suite_id,
            priority: q.priority.clone().filter(|s| !s.trim().is_empty()),
            assignee: q.assignee.clone().filter(|s| !s.trim().is_empty()),
            q: q.q.clone().filter(|s| !s.trim().is_empty()),
        }
    }

    fn from_groups(q: &RunGroupsQuery) -> Self {
        Self {
            status: normalize_opt_status(q.status.as_deref()),
            folder: q.folder.clone().filter(|s| !s.trim().is_empty()),
            suite_id: q.suite_id,
            priority: q.priority.clone().filter(|s| !s.trim().is_empty()),
            assignee: q.assignee.clone().filter(|s| !s.trim().is_empty()),
            q: q.q.clone().filter(|s| !s.trim().is_empty()),
        }
    }
}

fn normalize_opt_status(s: Option<&str>) -> Option<String> {
    // Accept both Passed and passed; match case-insensitively in SQL via lower().
    s.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

/// Build the filtered base set of case results for a run.
/// Prefer `run_items` when the run has any; otherwise latest `test_results` per case.
async fn load_base_rows(
    pool: &PgPool,
    project_id: Uuid,
    run_id: Uuid,
    filters: &Filters,
) -> AppResult<Vec<sqlx::postgres::PgRow>> {
    // Ensure run belongs to project.
    let exists: Option<(Uuid,)> =
        sqlx::query_as(r#"SELECT id FROM test_runs WHERE id = $1 AND project_id = $2"#)
            .bind(run_id)
            .bind(project_id)
            .fetch_optional(pool)
            .await?;
    if exists.is_none() {
        return Err(AppError::NotFound(format!("run {run_id}")));
    }

    let rows = sqlx::query(
        r#"
        WITH RECURSIVE suite_paths AS (
            SELECT id, parent_id, name, name::text AS path
            FROM suites
            WHERE project_id = $1 AND parent_id IS NULL
            UNION ALL
            SELECT s.id, s.parent_id, s.name, sp.path || ' / ' || s.name
            FROM suites s
            JOIN suite_paths sp ON s.parent_id = sp.id
            WHERE s.project_id = $1
        ),
        has_items AS (
            SELECT EXISTS(SELECT 1 FROM run_items WHERE run_id = $2) AS yes
        ),
        latest_result AS (
            SELECT DISTINCT ON (COALESCE(tr.run_item_id::text, tr.case_id::text))
                   tr.id, tr.run_id, tr.case_id, tr.run_item_id, tr.status, tr.message,
                   tr.assignee, tr.duration_ms, tr.elapsed_ms, tr.executed_by, tr.executed_at
            FROM test_results tr
            WHERE tr.run_id = $2
            ORDER BY COALESCE(tr.run_item_id::text, tr.case_id::text),
                     tr.executed_at DESC, tr.created_at DESC
        ),
        prev_status AS (
            SELECT DISTINCT ON (tr.case_id)
                   tr.case_id, tr.status
            FROM test_results tr
            JOIN test_runs r ON r.id = tr.run_id
            WHERE r.project_id = $1
              AND tr.run_id <> $2
              AND tr.case_id IS NOT NULL
              AND r.created_at < (SELECT created_at FROM test_runs WHERE id = $2)
            ORDER BY tr.case_id, tr.executed_at DESC, tr.created_at DESC
        ),
        from_items AS (
            SELECT
                ri.id AS run_item_id,
                ri.case_id,
                COALESCE(lr.status, ri.status) AS status,
                COALESCE(ri.assignee, lr.assignee) AS assignee,
                CASE
                    WHEN COALESCE(NULLIF(ri.comment, ''), NULLIF(lr.message, '')) IS NULL THEN ''
                    WHEN NULLIF(ri.comment, '') IS NOT NULL THEN ri.comment
                    ELSE lr.message
                END AS comment,
                ri.sort_order,
                lr.id AS result_id,
                COALESCE(lr.duration_ms::bigint, lr.elapsed_ms::bigint) AS duration_ms,
                lr.executed_by,
                u.display_name AS executed_by_name
            FROM run_items ri
            LEFT JOIN latest_result lr
              ON lr.run_item_id = ri.id
              OR (lr.run_item_id IS NULL AND lr.case_id = ri.case_id)
            LEFT JOIN users u ON u.id = lr.executed_by
            WHERE ri.run_id = $2 AND (SELECT yes FROM has_items)
        ),
        from_results AS (
            SELECT
                NULL::uuid AS run_item_id,
                tr.case_id,
                tr.status,
                tr.assignee,
                COALESCE(tr.message, '') AS comment,
                0 AS sort_order,
                tr.id AS result_id,
                COALESCE(tr.duration_ms::bigint, tr.elapsed_ms::bigint) AS duration_ms,
                tr.executed_by,
                u.display_name AS executed_by_name
            FROM (
                SELECT DISTINCT ON (case_id) *
                FROM test_results
                WHERE run_id = $2 AND case_id IS NOT NULL
                ORDER BY case_id, executed_at DESC, created_at DESC
            ) tr
            LEFT JOIN users u ON u.id = tr.executed_by
            WHERE NOT (SELECT yes FROM has_items)
        ),
        base AS (
            SELECT * FROM from_items
            UNION ALL
            SELECT * FROM from_results
        )
        SELECT
            b.run_item_id,
            b.case_id,
            b.status,
            b.assignee,
            b.comment,
            b.sort_order,
            b.result_id,
            b.duration_ms,
            b.executed_by_name,
            tc.key AS case_key,
            tc.title AS case_title,
            tc.priority,
            tc.suite_id,
            COALESCE(sp.path, 'Unfiled') AS folder_path,
            ps.status AS previous_status,
            COALESCE((
                SELECT COUNT(*)::bigint FROM attachments a
                WHERE a.entity_type = 'result' AND a.entity_id = b.result_id
            ), 0) AS attachment_count,
            COALESCE((
                SELECT COUNT(*)::bigint FROM defects d WHERE d.result_id = b.result_id
            ), 0) AS defect_count
        FROM base b
        JOIN test_cases tc ON tc.id = b.case_id
        LEFT JOIN suite_paths sp ON sp.id = tc.suite_id
        LEFT JOIN prev_status ps ON ps.case_id = b.case_id
        WHERE ($3::text IS NULL OR lower(b.status) = lower($3))
          AND ($4::uuid IS NULL OR tc.suite_id = $4)
          AND ($5::text IS NULL OR COALESCE(sp.path, 'Unfiled') ILIKE ('%' || $5 || '%'))
          AND ($6::text IS NULL OR lower(tc.priority) = lower($6))
          AND ($7::text IS NULL OR COALESCE(b.assignee, '') ILIKE ('%' || $7 || '%'))
          AND (
            $8::text IS NULL
            OR tc.key ILIKE ('%' || $8 || '%')
            OR tc.title ILIKE ('%' || $8 || '%')
            OR b.comment ILIKE ('%' || $8 || '%')
          )
        "#,
    )
    .bind(project_id)
    .bind(run_id)
    .bind(filters.status.as_deref())
    .bind(filters.suite_id)
    .bind(filters.folder.as_deref())
    .bind(filters.priority.as_deref())
    .bind(filters.assignee.as_deref())
    .bind(filters.q.as_deref())
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

async fn enrich_row(
    pool: &PgPool,
    row: &sqlx::postgres::PgRow,
    load_details: bool,
) -> AppResult<RunCaseResultRow> {
    let result_id: Option<Uuid> = row.try_get("result_id")?;
    let status: String = row.try_get("status")?;
    let previous_status: Option<String> = row.try_get("previous_status")?;
    let cmp = comparison(previous_status.as_deref(), &status);
    let attachment_count: i64 = row.try_get("attachment_count").unwrap_or(0);
    let defect_count: i64 = row.try_get("defect_count").unwrap_or(0);

    let mut defects = Vec::new();
    let mut attachments = Vec::new();

    if load_details {
        if let Some(rid) = result_id {
            if defect_count > 0 {
                let defect_rows =
                    sqlx::query_as::<_, (Uuid, Option<String>, String, String, String)>(
                        r#"SELECT id, external_key, external_url, title, status
                           FROM defects WHERE result_id = $1
                           ORDER BY created_at DESC LIMIT 20"#,
                    )
                    .bind(rid)
                    .fetch_all(pool)
                    .await?;
                defects = defect_rows
                    .into_iter()
                    .map(
                        |(id, external_key, external_url, title, status)| RunResultDefect {
                            id,
                            external_key,
                            external_url,
                            title,
                            status,
                        },
                    )
                    .collect();
            }
            if attachment_count > 0 {
                let att_rows = sqlx::query_as::<_, (Uuid, String, String, i64)>(
                    r#"SELECT id, filename, content_type, size_bytes
                       FROM attachments
                       WHERE entity_type = 'result' AND entity_id = $1
                       ORDER BY created_at DESC LIMIT 20"#,
                )
                .bind(rid)
                .fetch_all(pool)
                .await?;
                attachments = att_rows
                    .into_iter()
                    .map(
                        |(id, filename, content_type, size_bytes)| RunResultAttachment {
                            id,
                            filename,
                            content_type,
                            size_bytes,
                        },
                    )
                    .collect();
            }
        }
    }

    Ok(RunCaseResultRow {
        case_id: row.try_get("case_id")?,
        case_key: row.try_get("case_key")?,
        case_title: row.try_get("case_title")?,
        folder_path: row.try_get("folder_path")?,
        suite_id: row.try_get("suite_id")?,
        priority: row.try_get("priority")?,
        status,
        run_item_id: row.try_get("run_item_id")?,
        result_id,
        assignee: row.try_get("assignee")?,
        executed_by_name: row.try_get("executed_by_name")?,
        duration_ms: row.try_get("duration_ms")?,
        comment: row.try_get("comment")?,
        attachment_count,
        defect_count,
        defects,
        attachments,
        previous_status,
        comparison: cmp,
    })
}

pub async fn run_report_detail(
    pool: &PgPool,
    project_id: Uuid,
    run_id: Uuid,
) -> AppResult<RunReportDetail> {
    let summary = reports::run_summary(pool, project_id, run_id).await?;

    let row = sqlx::query(
        r#"SELECT id, project_id, name, description, status, source, external_ref, meta,
                  created_by_name, environment, artifacts, duration_ms, started_at, finished_at
           FROM test_runs WHERE id = $1 AND project_id = $2"#,
    )
    .bind(run_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("run {run_id}")))?;

    let source: String = row.try_get("source")?;
    let meta: Value = row.try_get("meta")?;
    let environment: Value = row.try_get("environment")?;
    let artifacts: Value = row.try_get("artifacts")?;
    let external_ref: Option<String> = row.try_get("external_ref")?;

    let branch = json_str(&meta, &["branch", "ref", "git_branch"])
        .or_else(|| json_str(&environment, &["branch", "ref", "git_branch"]));
    let commit = json_str(&meta, &["commit", "commit_sha", "sha", "git_commit"])
        .or_else(|| json_str(&environment, &["commit", "commit_sha", "sha"]));
    let pipeline_url = extract_pipeline_url(&meta, &artifacts, external_ref.as_deref());
    let env_label = environment_label(&environment);

    let duration_ms: Option<i64> = row.try_get("duration_ms")?;
    let started_at: Option<chrono::DateTime<chrono::Utc>> = row.try_get("started_at")?;
    let finished_at: Option<chrono::DateTime<chrono::Utc>> = row.try_get("finished_at")?;
    let duration_ms = duration_ms.or_else(|| match (started_at, finished_at) {
        (Some(s), Some(f)) => Some((f - s).num_milliseconds()),
        _ => None,
    });

    Ok(RunReportDetail {
        run_id: row.try_get("id")?,
        project_id: row.try_get("project_id")?,
        name: row.try_get("name")?,
        description: row.try_get("description")?,
        status: row.try_get("status")?,
        source: source.clone(),
        source_kind: source_kind(&source),
        external_ref,
        created_by_name: row.try_get("created_by_name")?,
        environment,
        artifacts,
        meta,
        branch,
        commit,
        pipeline_url,
        environment_label: env_label,
        started_at,
        finished_at,
        duration_ms,
        counts: summary.counts,
        pass_rate: summary.pass_rate,
    })
}

pub async fn list_run_case_results(
    pool: &PgPool,
    project_id: Uuid,
    run_id: Uuid,
    query: &RunResultsQuery,
) -> AppResult<RunCaseResultsPage> {
    list_run_case_results_inner(pool, project_id, run_id, query, true).await
}

/// Export path: skip loading defect/attachment detail objects (counts only).
pub async fn list_run_case_results_for_export(
    pool: &PgPool,
    project_id: Uuid,
    run_id: Uuid,
    query: &RunResultsQuery,
) -> AppResult<RunCaseResultsPage> {
    list_run_case_results_inner(pool, project_id, run_id, query, false).await
}

async fn list_run_case_results_inner(
    pool: &PgPool,
    project_id: Uuid,
    run_id: Uuid,
    query: &RunResultsQuery,
    load_details: bool,
) -> AppResult<RunCaseResultsPage> {
    let filters = Filters::from_results(query);
    let mut rows = load_base_rows(pool, project_id, run_id, &filters).await?;
    let total = rows.len() as i64;

    let (sort_col, descending) = parse_sort(query.sort.as_deref());
    rows.sort_by(|a, b| {
        let ord = match sort_col {
            "case_title" => {
                let av: String = a.try_get("case_title").unwrap_or_default();
                let bv: String = b.try_get("case_title").unwrap_or_default();
                av.to_lowercase().cmp(&bv.to_lowercase())
            }
            "status" => {
                let av: String = a.try_get("status").unwrap_or_default();
                let bv: String = b.try_get("status").unwrap_or_default();
                av.to_lowercase().cmp(&bv.to_lowercase())
            }
            "duration_ms" => {
                let av: Option<i64> = a.try_get("duration_ms").ok().flatten();
                let bv: Option<i64> = b.try_get("duration_ms").ok().flatten();
                av.cmp(&bv)
            }
            "folder_path" => {
                let av: String = a.try_get("folder_path").unwrap_or_default();
                let bv: String = b.try_get("folder_path").unwrap_or_default();
                av.to_lowercase().cmp(&bv.to_lowercase())
            }
            "priority" => {
                let av: String = a.try_get("priority").unwrap_or_default();
                let bv: String = b.try_get("priority").unwrap_or_default();
                priority_rank(&av).cmp(&priority_rank(&bv))
            }
            "assignee" => {
                let av: Option<String> = a.try_get("assignee").ok().flatten();
                let bv: Option<String> = b.try_get("assignee").ok().flatten();
                av.cmp(&bv)
            }
            _ => {
                let av: String = a.try_get("case_key").unwrap_or_default();
                let bv: String = b.try_get("case_key").unwrap_or_default();
                compare_keys(&av, &bv)
            }
        };
        if descending {
            ord.reverse()
        } else {
            ord
        }
    });

    // Export may request up to 10_000; interactive pages stay within PageQuery max.
    let max_limit = if load_details { 500 } else { 20_000 };
    let limit = query.limit.unwrap_or(50).clamp(1, max_limit);
    let offset = query.offset.unwrap_or(0).max(0);
    let page: Vec<_> = rows
        .into_iter()
        .skip(offset as usize)
        .take(limit as usize)
        .collect();

    let mut items = Vec::with_capacity(page.len());
    for row in &page {
        items.push(enrich_row(pool, row, load_details).await?);
    }

    Ok(RunCaseResultsPage {
        items,
        total,
        limit,
        offset,
    })
}

fn priority_rank(p: &str) -> u8 {
    match p.to_ascii_lowercase().as_str() {
        "critical" => 0,
        "high" => 1,
        "medium" => 2,
        "low" => 3,
        _ => 4,
    }
}

fn compare_keys(a: &str, b: &str) -> std::cmp::Ordering {
    fn split(k: &str) -> (String, i64) {
        if let Some((prefix, num)) = k.rsplit_once('-') {
            if let Ok(n) = num.parse::<i64>() {
                return (prefix.to_ascii_lowercase(), n);
            }
        }
        (k.to_ascii_lowercase(), 0)
    }
    let (ap, an) = split(a);
    let (bp, bn) = split(b);
    ap.cmp(&bp).then(an.cmp(&bn)).then(a.cmp(b))
}

pub async fn expand_run_case_result(
    pool: &PgPool,
    project_id: Uuid,
    run_id: Uuid,
    case_id: Uuid,
) -> AppResult<RunCaseResultExpand> {
    let _ = sqlx::query_as::<_, (Uuid,)>(
        r#"SELECT id FROM test_runs WHERE id = $1 AND project_id = $2"#,
    )
    .bind(run_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("run {run_id}")))?;

    let run_item_id: Option<Uuid> =
        sqlx::query_scalar(r#"SELECT id FROM run_items WHERE run_id = $1 AND case_id = $2"#)
            .bind(run_id)
            .bind(case_id)
            .fetch_optional(pool)
            .await?;

    let result_id: Option<Uuid> = if let Some(ri) = run_item_id {
        sqlx::query_scalar(
            r#"SELECT id FROM test_results
               WHERE run_id = $1 AND (run_item_id = $2 OR case_id = $3)
               ORDER BY executed_at DESC, created_at DESC LIMIT 1"#,
        )
        .bind(run_id)
        .bind(ri)
        .bind(case_id)
        .fetch_optional(pool)
        .await?
    } else {
        sqlx::query_scalar(
            r#"SELECT id FROM test_results
               WHERE run_id = $1 AND case_id = $2
               ORDER BY executed_at DESC, created_at DESC LIMIT 1"#,
        )
        .bind(run_id)
        .bind(case_id)
        .fetch_optional(pool)
        .await?
    };

    let mut steps = Vec::new();
    if let Some(rid) = result_id {
        let step_rows = sqlx::query_as::<_, (i32, String, String, String, String, String)>(
            r#"SELECT position, action, expected_result, actual_result, status, comment
               FROM result_steps WHERE result_id = $1 ORDER BY position"#,
        )
        .bind(rid)
        .fetch_all(pool)
        .await?;
        steps = step_rows
            .into_iter()
            .map(
                |(position, action, expected_result, actual_result, status, comment)| {
                    RunResultStepRow {
                        position,
                        action,
                        expected_result,
                        actual_result,
                        status,
                        comment,
                    }
                },
            )
            .collect();
    }

    // If no result steps, fall back to case definition steps as untested.
    if steps.is_empty() {
        let case_steps = sqlx::query_as::<_, (i32, String, String)>(
            r#"SELECT position, action, expected_result
               FROM test_steps WHERE case_id = $1 ORDER BY position"#,
        )
        .bind(case_id)
        .fetch_all(pool)
        .await?;
        steps = case_steps
            .into_iter()
            .map(|(position, action, expected_result)| RunResultStepRow {
                position,
                action,
                expected_result,
                actual_result: String::new(),
                status: "untested".into(),
                comment: String::new(),
            })
            .collect();
    }

    let history_rows = sqlx::query(
        r#"SELECT tr.id, tr.status, tr.message, tr.executed_at, u.display_name AS executed_by_name
           FROM test_results tr
           LEFT JOIN users u ON u.id = tr.executed_by
           WHERE tr.run_id = $1 AND tr.case_id = $2
           ORDER BY tr.executed_at ASC, tr.created_at ASC"#,
    )
    .bind(run_id)
    .bind(case_id)
    .fetch_all(pool)
    .await?;

    let mut comment_history = Vec::new();
    for h in history_rows {
        comment_history.push(RunResultCommentEntry {
            result_id: h.try_get("id")?,
            status: h.try_get("status")?,
            message: h.try_get("message")?,
            executed_at: h.try_get("executed_at")?,
            executed_by_name: h.try_get("executed_by_name")?,
        });
    }

    // Include mutable run_item comment if present and not already last message.
    if let Some(ri) = run_item_id {
        if let Some((comment, updated_at)) =
            sqlx::query_as::<_, (String, chrono::DateTime<chrono::Utc>)>(
                r#"SELECT comment, updated_at FROM run_items WHERE id = $1"#,
            )
            .bind(ri)
            .fetch_optional(pool)
            .await?
        {
            if !comment.is_empty()
                && comment_history.last().map(|c| c.message.as_str()) != Some(comment.as_str())
            {
                comment_history.push(RunResultCommentEntry {
                    result_id: result_id.unwrap_or(Uuid::nil()),
                    status: String::new(),
                    message: comment,
                    executed_at: updated_at,
                    executed_by_name: None,
                });
            }
        }
    }

    Ok(RunCaseResultExpand {
        case_id,
        run_item_id,
        result_id,
        steps,
        comment_history,
    })
}

pub async fn run_result_groups(
    pool: &PgPool,
    project_id: Uuid,
    run_id: Uuid,
    query: &RunGroupsQuery,
) -> AppResult<RunResultGroups> {
    let filters = Filters::from_groups(query);
    let rows = load_base_rows(pool, project_id, run_id, &filters).await?;
    let group_by = query
        .group_by
        .as_deref()
        .unwrap_or("folder")
        .to_ascii_lowercase();

    use std::collections::BTreeMap;
    let mut map: BTreeMap<String, StatusCounts> = BTreeMap::new();

    for row in &rows {
        let key = if group_by == "status" {
            let s: String = row.try_get("status")?;
            s
        } else {
            let s: String = row.try_get("folder_path")?;
            s
        };
        let status: String = row.try_get("status")?;
        let entry = map.entry(key).or_insert_with(empty_counts);
        // reuse accumulate via a one-row CountRow
        let cr = CountRow { status, count: 1 };
        let partial = from_rows(std::slice::from_ref(&cr));
        entry.total += partial.total;
        entry.passed += partial.passed;
        entry.failed += partial.failed;
        entry.skipped += partial.skipped;
        entry.blocked += partial.blocked;
        entry.error += partial.error;
        entry.untested += partial.untested;
        entry.retest += partial.retest;
        entry.other += partial.other;
    }

    let mut groups: Vec<RunResultGroup> = map
        .into_iter()
        .map(|(key, counts)| {
            let pass_rate = counts.pass_rate();
            RunResultGroup {
                label: key.clone(),
                key,
                counts,
                pass_rate,
            }
        })
        .collect();

    if group_by == "status" {
        groups.sort_by_key(|a| a.key.to_lowercase());
    } else {
        groups.sort_by(|a, b| b.counts.total.cmp(&a.counts.total).then(a.key.cmp(&b.key)));
    }

    Ok(RunResultGroups { group_by, groups })
}

pub async fn requirement_coverage_detail(
    pool: &PgPool,
    project_id: Uuid,
) -> AppResult<RequirementCoverageDetail> {
    #[derive(sqlx::FromRow)]
    struct Raw {
        requirement_id: Uuid,
        external_key: Option<String>,
        title: String,
        source: String,
        case_id: Option<Uuid>,
        case_key: Option<String>,
        case_title: Option<String>,
        latest_status: Option<String>,
    }

    let rows = sqlx::query_as::<_, Raw>(
        r#"
        WITH latest AS (
            SELECT DISTINCT ON (tr.case_id)
                   tr.case_id, tr.status
            FROM test_results tr
            JOIN test_runs run ON run.id = tr.run_id
            WHERE run.project_id = $1 AND tr.case_id IS NOT NULL
            ORDER BY tr.case_id, tr.executed_at DESC, tr.created_at DESC
        )
        SELECT r.id AS requirement_id,
               r.external_key,
               r.title,
               r.source,
               c.id AS case_id,
               c.key AS case_key,
               c.title AS case_title,
               latest.status AS latest_status
        FROM requirements r
        LEFT JOIN case_requirements cr ON cr.requirement_id = r.id
        LEFT JOIN test_cases c ON c.id = cr.case_id
        LEFT JOIN latest ON latest.case_id = c.id
        WHERE r.project_id = $1
        ORDER BY r.title, c.key
        "#,
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;

    // Preserve SQL order (title, key) while grouping requirement rows.
    let mut by_req: Vec<(Uuid, CoverageDetailRow)> = Vec::new();
    let mut index: std::collections::HashMap<Uuid, usize> = std::collections::HashMap::new();

    for row in rows {
        let idx = if let Some(&i) = index.get(&row.requirement_id) {
            i
        } else {
            let i = by_req.len();
            index.insert(row.requirement_id, i);
            by_req.push((
                row.requirement_id,
                CoverageDetailRow {
                    requirement_id: row.requirement_id,
                    external_key: row.external_key.clone(),
                    title: row.title.clone(),
                    source: row.source.clone(),
                    covered: false,
                    linked_cases: Vec::new(),
                    cases_passed: 0,
                    cases_failed: 0,
                    pass_percent: 0.0,
                },
            ));
            i
        };
        let entry = &mut by_req[idx].1;
        if let (Some(case_id), Some(case_key), Some(case_title)) =
            (row.case_id, row.case_key, row.case_title)
        {
            entry.covered = true;
            if let Some(ref st) = row.latest_status {
                let l = st.to_ascii_lowercase();
                if l == "passed" {
                    entry.cases_passed += 1;
                } else if l == "failed" || l == "error" {
                    entry.cases_failed += 1;
                }
            }
            entry.linked_cases.push(CoverageLinkedCase {
                case_id,
                case_key,
                title: case_title,
                latest_status: row.latest_status,
            });
        }
    }

    let mut covered = Vec::new();
    let mut uncovered = Vec::new();
    for (_, mut row) in by_req {
        let decided = row.cases_passed + row.cases_failed;
        row.pass_percent = if decided == 0 {
            0.0
        } else {
            (row.cases_passed as f64) / (decided as f64) * 100.0
        };
        if row.covered {
            covered.push(row);
        } else {
            uncovered.push(row);
        }
    }

    let total = (covered.len() + uncovered.len()) as i64;
    let covered_n = covered.len() as i64;
    let coverage_percent = if total == 0 {
        0.0
    } else {
        (covered_n as f64) / (total as f64) * 100.0
    };

    Ok(RequirementCoverageDetail {
        project_id,
        total_requirements: total,
        covered_requirements: covered_n,
        uncovered_requirements: uncovered.len() as i64,
        coverage_percent,
        covered,
        uncovered,
    })
}

pub fn run_results_to_csv(rows: &[RunCaseResultRow]) -> String {
    let mut out = String::from(
        "case_key,title,folder_path,priority,status,assignee,executed_by,duration_ms,comment,previous_status,comparison,attachment_count,defect_count,defects\n",
    );
    for r in rows {
        let defects = r
            .defects
            .iter()
            .map(|d| d.external_key.clone().unwrap_or_else(|| d.title.clone()))
            .collect::<Vec<_>>()
            .join("|");
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            csv_escape(&r.case_key),
            csv_escape(&r.case_title),
            csv_escape(&r.folder_path),
            csv_escape(&r.priority),
            csv_escape(&r.status),
            csv_escape(r.assignee.as_deref().unwrap_or("")),
            csv_escape(r.executed_by_name.as_deref().unwrap_or("")),
            r.duration_ms.map(|n| n.to_string()).unwrap_or_default(),
            csv_escape(&r.comment),
            csv_escape(r.previous_status.as_deref().unwrap_or("")),
            csv_escape(r.comparison.as_deref().unwrap_or("")),
            r.attachment_count,
            r.defect_count,
            csv_escape(&defects),
        ));
    }
    out
}

pub fn run_results_pdf_lines(detail: &RunReportDetail, rows: &[RunCaseResultRow]) -> Vec<String> {
    let mut lines = vec![
        format!(
            "Run: {} ({}) — {} — pass {:.1}%",
            detail.name, detail.source_kind, detail.status, detail.pass_rate
        ),
        format!(
            "Passed {} / Failed {} / Total {}",
            detail.counts.passed, detail.counts.failed, detail.counts.total
        ),
        "Case results:".into(),
    ];
    for r in rows.iter().take(80) {
        lines.push(format!(
            "  {} {} — {} — {}",
            r.case_key, r.case_title, r.status, r.folder_path
        ));
    }
    if rows.len() > 80 {
        lines.push(format!("  … and {} more", rows.len() - 80));
    }
    lines
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn comparison_detects_newly_failing_and_fixed() {
        assert_eq!(
            comparison(Some("Passed"), "Failed").as_deref(),
            Some("newly_failing")
        );
        assert_eq!(
            comparison(Some("failed"), "passed").as_deref(),
            Some("fixed")
        );
        assert_eq!(comparison(Some("passed"), "skipped").as_deref(), None);
        assert_eq!(
            comparison(Some("passed"), "passed").as_deref(),
            Some("unchanged")
        );
        assert_eq!(comparison(Some("blocked"), "passed").as_deref(), None);
        assert_eq!(comparison(Some("failed"), "blocked").as_deref(), None);
        assert_eq!(
            comparison(Some("passed"), "failed").as_deref(),
            Some("newly_failing")
        );
        // decided flip that isn't pass<->fail still marks flaky
        // blocked is not decided, so None — use failed->skipped? skipped not decided
        assert_eq!(
            comparison(Some("error"), "passed").as_deref(),
            Some("fixed")
        );
    }

    #[test]
    fn source_kind_mapping() {
        assert_eq!(source_kind("manual"), "manual");
        assert_eq!(source_kind("ci"), "ci");
        assert_eq!(source_kind("github-actions"), "ci");
        assert_eq!(source_kind("junit"), "automated");
    }
}
