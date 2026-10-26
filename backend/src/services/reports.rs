use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::{
    FlakyCase, FlakyReport, PassFailTrends, ProjectDashboard, RequirementCoverageReport,
    RequirementCoverageRow, RunSummaryReport, StatusCounts, TrendPoint,
};

#[derive(sqlx::FromRow)]
pub struct CountRow {
    pub status: String,
    pub count: i64,
}

pub fn empty_counts() -> StatusCounts {
    StatusCounts {
        total: 0,
        passed: 0,
        failed: 0,
        skipped: 0,
        blocked: 0,
        error: 0,
        untested: 0,
        retest: 0,
        other: 0,
    }
}

pub fn accumulate(counts: &mut StatusCounts, status: &str, n: i64) {
    counts.total += n;
    match status.to_ascii_lowercase().as_str() {
        "passed" => counts.passed += n,
        "failed" => counts.failed += n,
        "skipped" => counts.skipped += n,
        "blocked" => counts.blocked += n,
        // Legacy; new ingest maps JUnit error → Failed.
        "error" => counts.error += n,
        "untested" => counts.untested += n,
        "retest" => counts.retest += n,
        _ => counts.other += n,
    }
}

pub fn from_rows(rows: &[CountRow]) -> StatusCounts {
    let mut counts = empty_counts();
    for row in rows {
        accumulate(&mut counts, &row.status, row.count);
    }
    counts
}

pub async fn run_summary(
    pool: &PgPool,
    project_id: Uuid,
    run_id: Uuid,
) -> AppResult<RunSummaryReport> {
    let run = sqlx::query_as::<
        _,
        (
            Uuid,
            Uuid,
            String,
            String,
            Option<chrono::DateTime<chrono::Utc>>,
            Option<chrono::DateTime<chrono::Utc>>,
        ),
    >(
        r#"SELECT id, project_id, name, status, started_at, finished_at
           FROM test_runs WHERE id = $1 AND project_id = $2"#,
    )
    .bind(run_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("run {run_id}")))?;

    // Prefer ingested test_results when present (CI / bulk upload). Otherwise use
    // run_items (manual execution progress, including untested placeholders).
    let result_rows = sqlx::query_as::<_, CountRow>(
        r#"SELECT status, COUNT(*)::bigint AS count
           FROM test_results WHERE run_id = $1
           GROUP BY status"#,
    )
    .bind(run_id)
    .fetch_all(pool)
    .await?;

    let counts = if result_rows.iter().any(|r| r.count > 0) {
        from_rows(&result_rows)
    } else {
        let item_rows = sqlx::query_as::<_, CountRow>(
            r#"SELECT status, COUNT(*)::bigint AS count
               FROM run_items WHERE run_id = $1
               GROUP BY status"#,
        )
        .bind(run_id)
        .fetch_all(pool)
        .await?;
        from_rows(&item_rows)
    };

    let pass_rate = counts.pass_rate();
    Ok(RunSummaryReport {
        run_id: run.0,
        project_id: run.1,
        name: run.2,
        status: run.3,
        started_at: run.4,
        finished_at: run.5,
        counts,
        pass_rate,
    })
}

pub async fn pass_fail_trends(
    pool: &PgPool,
    project_id: Uuid,
    granularity: &str,
    limit: i64,
) -> AppResult<PassFailTrends> {
    let limit = limit.clamp(1, 100);
    let mut points = Vec::new();

    if granularity == "day" {
        #[derive(sqlx::FromRow)]
        struct DayRow {
            bucket: chrono::NaiveDate,
            status: String,
            count: i64,
        }
        let rows = sqlx::query_as::<_, DayRow>(
            r#"SELECT (COALESCE(r.executed_at, r.created_at) AT TIME ZONE 'UTC')::date AS bucket,
                      r.status,
                      COUNT(*)::bigint AS count
               FROM test_results r
               JOIN test_runs tr ON tr.id = r.run_id
               WHERE tr.project_id = $1
               GROUP BY 1, 2
               ORDER BY 1 DESC
               LIMIT 500"#,
        )
        .bind(project_id)
        .fetch_all(pool)
        .await?;

        use std::collections::BTreeMap;
        let mut by_day: BTreeMap<chrono::NaiveDate, StatusCounts> = BTreeMap::new();
        for row in rows {
            let entry = by_day.entry(row.bucket).or_insert_with(empty_counts);
            accumulate(entry, &row.status, row.count);
        }
        for (day, counts) in by_day.into_iter().rev().take(limit as usize) {
            let pass_rate = counts.pass_rate();
            points.push(TrendPoint {
                bucket: day.to_string(),
                run_id: None,
                run_name: None,
                counts,
                pass_rate,
            });
        }
        points.reverse();
    } else {
        let runs = sqlx::query_as::<_, (Uuid, String, chrono::DateTime<chrono::Utc>)>(
            r#"SELECT id, name, created_at FROM test_runs
               WHERE project_id = $1
               ORDER BY created_at DESC
               LIMIT $2"#,
        )
        .bind(project_id)
        .bind(limit)
        .fetch_all(pool)
        .await?;

        for (run_id, name, created_at) in runs.into_iter().rev() {
            let summary = run_summary(pool, project_id, run_id).await?;
            points.push(TrendPoint {
                bucket: created_at.date_naive().to_string(),
                run_id: Some(run_id),
                run_name: Some(name),
                counts: summary.counts,
                pass_rate: summary.pass_rate,
            });
        }
    }

    Ok(PassFailTrends {
        project_id,
        granularity: granularity.to_string(),
        points,
    })
}

pub async fn requirement_coverage(
    pool: &PgPool,
    project_id: Uuid,
) -> AppResult<RequirementCoverageReport> {
    #[derive(sqlx::FromRow)]
    struct CovRow {
        requirement_id: Uuid,
        external_key: Option<String>,
        title: String,
        source: String,
        linked_cases: i64,
        cases_with_results: i64,
        cases_passed: i64,
        cases_failed: i64,
    }

    let rows = sqlx::query_as::<_, CovRow>(
        r#"
        WITH linked AS (
            SELECT r.id AS requirement_id,
                   r.external_key,
                   r.title,
                   r.source,
                   cr.case_id
            FROM requirements r
            LEFT JOIN case_requirements cr ON cr.requirement_id = r.id
            WHERE r.project_id = $1
        ),
        latest AS (
            SELECT DISTINCT ON (tr.case_id)
                   tr.case_id,
                   tr.status
            FROM test_results tr
            JOIN test_runs run ON run.id = tr.run_id
            WHERE run.project_id = $1 AND tr.case_id IS NOT NULL
            ORDER BY tr.case_id, tr.executed_at DESC, tr.created_at DESC
        )
        SELECT linked.requirement_id,
               linked.external_key,
               linked.title,
               linked.source,
               COUNT(DISTINCT linked.case_id) FILTER (WHERE linked.case_id IS NOT NULL)::bigint AS linked_cases,
               COUNT(DISTINCT linked.case_id) FILTER (WHERE latest.status IS NOT NULL)::bigint AS cases_with_results,
               COUNT(DISTINCT linked.case_id) FILTER (WHERE latest.status = 'Passed')::bigint AS cases_passed,
               COUNT(DISTINCT linked.case_id) FILTER (
                   WHERE latest.status IN ('Failed', 'failed', 'error')
               )::bigint AS cases_failed
        FROM linked
        LEFT JOIN latest ON latest.case_id = linked.case_id
        GROUP BY linked.requirement_id, linked.external_key, linked.title, linked.source
        ORDER BY linked.title
        "#,
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;

    let total_requirements = rows.len() as i64;
    let mut covered = 0i64;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        if row.linked_cases > 0 {
            covered += 1;
        }
        let coverage_percent = if row.linked_cases == 0 { 0.0 } else { 100.0 };
        let decided = row.cases_passed + row.cases_failed;
        let pass_percent = if decided == 0 {
            0.0
        } else {
            (row.cases_passed as f64) / (decided as f64) * 100.0
        };
        out.push(RequirementCoverageRow {
            requirement_id: row.requirement_id,
            external_key: row.external_key,
            title: row.title,
            source: row.source,
            linked_cases: row.linked_cases,
            cases_with_results: row.cases_with_results,
            cases_passed: row.cases_passed,
            cases_failed: row.cases_failed,
            coverage_percent,
            pass_percent,
        });
    }

    let coverage_percent = if total_requirements == 0 {
        0.0
    } else {
        (covered as f64) / (total_requirements as f64) * 100.0
    };

    Ok(RequirementCoverageReport {
        project_id,
        total_requirements,
        covered_requirements: covered,
        coverage_percent,
        rows: out,
    })
}

pub async fn flaky_cases(
    pool: &PgPool,
    project_id: Uuid,
    lookback_runs: i64,
    limit: i64,
) -> AppResult<FlakyReport> {
    let lookback_runs = lookback_runs.clamp(2, 100);
    let limit = limit.clamp(1, 200);

    #[derive(sqlx::FromRow)]
    struct FlakeRow {
        case_id: Uuid,
        case_key: String,
        title: String,
        runs_seen: i64,
        passes: i64,
        failures: i64,
        last_statuses: Vec<String>,
    }

    let rows = sqlx::query_as::<_, FlakeRow>(
        r#"
        WITH recent_runs AS (
            SELECT id FROM test_runs
            WHERE project_id = $1
            ORDER BY created_at DESC
            LIMIT $2
        ),
        per_run AS (
            SELECT DISTINCT ON (tr.case_id, tr.run_id)
                   tr.case_id,
                   tr.run_id,
                   tr.status,
                   tr.executed_at
            FROM test_results tr
            JOIN recent_runs rr ON rr.id = tr.run_id
            WHERE tr.case_id IS NOT NULL
              AND tr.status IN ('Passed', 'Failed', 'passed', 'failed', 'error')
            ORDER BY tr.case_id, tr.run_id, tr.executed_at DESC, tr.created_at DESC
        ),
        agg AS (
            SELECT c.id AS case_id,
                   c.key AS case_key,
                   c.title,
                   COUNT(*)::bigint AS runs_seen,
                   COUNT(*) FILTER (WHERE lower(per_run.status) = 'passed')::bigint AS passes,
                   COUNT(*) FILTER (WHERE lower(per_run.status) IN ('failed', 'error'))::bigint AS failures,
                   ARRAY_AGG(per_run.status ORDER BY per_run.executed_at DESC) AS last_statuses
            FROM per_run
            JOIN test_cases c ON c.id = per_run.case_id
            GROUP BY c.id, c.key, c.title
            HAVING COUNT(*) FILTER (WHERE lower(per_run.status) = 'passed') > 0
               AND COUNT(*) FILTER (WHERE lower(per_run.status) IN ('failed', 'error')) > 0
        )
        SELECT case_id, case_key, title, runs_seen, passes, failures, last_statuses
        FROM agg
        ORDER BY LEAST(passes, failures)::float / NULLIF(runs_seen, 0) DESC, runs_seen DESC
        LIMIT $3
        "#,
    )
    .bind(project_id)
    .bind(lookback_runs)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    let cases = rows
        .into_iter()
        .map(|r| {
            let flake_score = if r.runs_seen == 0 {
                0.0
            } else {
                (r.passes.min(r.failures) as f64) / (r.runs_seen as f64)
            };
            FlakyCase {
                case_id: r.case_id,
                case_key: r.case_key,
                title: r.title,
                runs_seen: r.runs_seen,
                passes: r.passes,
                failures: r.failures,
                flake_score,
                last_statuses: r.last_statuses.into_iter().take(10).collect(),
            }
        })
        .collect();

    Ok(FlakyReport {
        project_id,
        lookback_runs,
        cases,
    })
}

pub async fn project_dashboard(pool: &PgPool, project_id: Uuid) -> AppResult<ProjectDashboard> {
    let project = sqlx::query_as::<_, (Uuid, String, String)>(
        r#"SELECT id, key, name FROM projects WHERE id = $1"#,
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("project {project_id}")))?;

    let total_cases: (i64,) =
        sqlx::query_as(r#"SELECT COUNT(*)::bigint FROM test_cases WHERE project_id = $1"#)
            .bind(project_id)
            .fetch_one(pool)
            .await?;
    let total_runs: (i64,) =
        sqlx::query_as(r#"SELECT COUNT(*)::bigint FROM test_runs WHERE project_id = $1"#)
            .bind(project_id)
            .fetch_one(pool)
            .await?;
    let total_requirements: (i64,) =
        sqlx::query_as(r#"SELECT COUNT(*)::bigint FROM requirements WHERE project_id = $1"#)
            .bind(project_id)
            .fetch_one(pool)
            .await?;

    let recent_ids: Vec<(Uuid,)> = sqlx::query_as(
        r#"SELECT id FROM test_runs WHERE project_id = $1 ORDER BY created_at DESC LIMIT 5"#,
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?;

    let mut recent_run_summaries = Vec::new();
    for (id,) in recent_ids {
        recent_run_summaries.push(run_summary(pool, project_id, id).await?);
    }

    let trends = pass_fail_trends(pool, project_id, "run", 10).await?;
    let coverage = requirement_coverage(pool, project_id).await?;
    let flaky = flaky_cases(pool, project_id, 20, 20).await?;

    Ok(ProjectDashboard {
        project_id: project.0,
        project_key: project.1,
        project_name: project.2,
        total_cases: total_cases.0,
        total_runs: total_runs.0,
        total_requirements: total_requirements.0,
        recent_run_summaries,
        trends,
        coverage,
        flaky,
    })
}

/// Build a CSV export for the selected report kind.
pub fn dashboard_to_csv(dash: &ProjectDashboard) -> String {
    let mut out = String::from("section,metric,value\n");
    out.push_str(&format!("project,key,{}\n", csv_escape(&dash.project_key)));
    out.push_str(&format!(
        "project,name,{}\n",
        csv_escape(&dash.project_name)
    ));
    out.push_str(&format!("project,total_cases,{}\n", dash.total_cases));
    out.push_str(&format!("project,total_runs,{}\n", dash.total_runs));
    out.push_str(&format!(
        "project,total_requirements,{}\n",
        dash.total_requirements
    ));
    out.push_str(&format!(
        "coverage,percent,{:.2}\n",
        dash.coverage.coverage_percent
    ));
    out.push_str("run_id,name,status,passed,failed,pass_rate\n");
    for s in &dash.recent_run_summaries {
        out.push_str(&format!(
            "{},{},{},{},{},{:.2}\n",
            s.run_id,
            csv_escape(&s.name),
            csv_escape(&s.status),
            s.counts.passed,
            s.counts.failed,
            s.pass_rate
        ));
    }
    out.push_str("requirement_id,title,linked_cases,cases_passed,cases_failed,pass_percent\n");
    for r in &dash.coverage.rows {
        out.push_str(&format!(
            "{},{},{},{},{},{:.2}\n",
            r.requirement_id,
            csv_escape(&r.title),
            r.linked_cases,
            r.cases_passed,
            r.cases_failed,
            r.pass_percent
        ));
    }
    out.push_str("case_key,title,runs_seen,passes,failures,flake_score\n");
    for f in &dash.flaky.cases {
        out.push_str(&format!(
            "{},{},{},{},{},{:.3}\n",
            csv_escape(&f.case_key),
            csv_escape(&f.title),
            f.runs_seen,
            f.passes,
            f.failures,
            f.flake_score
        ));
    }
    out
}

pub fn coverage_to_csv(report: &RequirementCoverageReport) -> String {
    let mut out = String::from(
        "requirement_id,external_key,title,source,linked_cases,cases_with_results,cases_passed,cases_failed,coverage_percent,pass_percent\n",
    );
    for r in &report.rows {
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{:.2},{:.2}\n",
            r.requirement_id,
            csv_escape(r.external_key.as_deref().unwrap_or("")),
            csv_escape(&r.title),
            csv_escape(&r.source),
            r.linked_cases,
            r.cases_with_results,
            r.cases_passed,
            r.cases_failed,
            r.coverage_percent,
            r.pass_percent
        ));
    }
    out
}

pub fn trends_to_csv(report: &PassFailTrends) -> String {
    let mut out = String::from("bucket,run_id,run_name,passed,failed,skipped,pass_rate\n");
    for p in &report.points {
        out.push_str(&format!(
            "{},{},{},{},{},{},{:.2}\n",
            csv_escape(&p.bucket),
            p.run_id.map(|id| id.to_string()).unwrap_or_default(),
            csv_escape(p.run_name.as_deref().unwrap_or("")),
            p.counts.passed,
            p.counts.failed,
            p.counts.skipped,
            p.pass_rate
        ));
    }
    out
}

pub fn flaky_to_csv(report: &FlakyReport) -> String {
    let mut out = String::from(
        "case_id,case_key,title,runs_seen,passes,failures,flake_score,last_statuses\n",
    );
    for f in &report.cases {
        out.push_str(&format!(
            "{},{},{},{},{},{},{:.3},{}\n",
            f.case_id,
            csv_escape(&f.case_key),
            csv_escape(&f.title),
            f.runs_seen,
            f.passes,
            f.failures,
            f.flake_score,
            csv_escape(&f.last_statuses.join("|"))
        ));
    }
    out
}

pub fn run_summary_to_csv(s: &RunSummaryReport) -> String {
    format!(
        "run_id,name,status,total,passed,failed,skipped,blocked,error,pass_rate\n{},{},{},{},{},{},{},{},{},{:.2}\n",
        s.run_id,
        csv_escape(&s.name),
        csv_escape(&s.status),
        s.counts.total,
        s.counts.passed,
        s.counts.failed,
        s.counts.skipped,
        s.counts.blocked,
        s.counts.error,
        s.pass_rate
    )
}

/// Minimal single-page PDF with report title + body lines (no external crate).
pub fn text_report_to_pdf(title: &str, lines: &[String]) -> Vec<u8> {
    let mut content = String::from("BT /F1 14 Tf 50 780 Td (");
    content.push_str(&pdf_escape(title));
    content.push_str(") Tj 0 -24 Td /F1 10 Tf ");
    for (i, line) in lines.iter().take(60).enumerate() {
        if i > 0 {
            content.push_str("0 -14 Td ");
        }
        content.push('(');
        content.push_str(&pdf_escape(line));
        content.push_str(") Tj ");
    }
    content.push_str("ET");

    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>".to_string(),
        format!("<< /Length {} >>\nstream\n{}\nendstream", content.len(), content),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
    ];

    let mut pdf = String::from("%PDF-1.4\n");
    let mut offsets = Vec::new();
    for (i, obj) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.push_str(&format!("{} 0 obj\n{}\nendobj\n", i + 1, obj));
    }
    let xref_at = pdf.len();
    pdf.push_str(&format!("xref\n0 {}\n", objects.len() + 1));
    pdf.push_str("0000000000 65535 f \n");
    for off in offsets {
        pdf.push_str(&format!("{off:010} 00000 n \n"));
    }
    pdf.push_str(&format!(
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
        objects.len() + 1,
        xref_at
    ));
    pdf.into_bytes()
}

pub fn dashboard_pdf_lines(dash: &ProjectDashboard) -> Vec<String> {
    let mut lines = vec![
        format!(
            "Project {} ({}) — cases {} runs {} requirements {}",
            dash.project_name,
            dash.project_key,
            dash.total_cases,
            dash.total_runs,
            dash.total_requirements
        ),
        format!(
            "Requirement coverage: {:.1}% ({}/{})",
            dash.coverage.coverage_percent,
            dash.coverage.covered_requirements,
            dash.coverage.total_requirements
        ),
        "Recent runs:".into(),
    ];
    for s in &dash.recent_run_summaries {
        lines.push(format!(
            "  {} — pass {:.1}% ({} passed / {} failed)",
            s.name, s.pass_rate, s.counts.passed, s.counts.failed
        ));
    }
    if !dash.flaky.cases.is_empty() {
        lines.push("Flaky cases:".into());
        for f in dash.flaky.cases.iter().take(15) {
            lines.push(format!(
                "  {} {} — flake {:.2} ({}P/{}F over {} runs)",
                f.case_key, f.title, f.flake_score, f.passes, f.failures, f.runs_seen
            ));
        }
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

fn pdf_escape(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\\' => "\\\\".to_string(),
            '(' => "\\(".to_string(),
            ')' => "\\)".to_string(),
            c if c.is_ascii() && !c.is_control() => c.to_string(),
            _ => "?".to_string(),
        })
        .collect()
}
