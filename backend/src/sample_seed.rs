//! Idempotent sample project seed (Kyiv Open-Air Concert 2026).
//!
//! Opt out with `VULYK_SEED_SAMPLE=0`. Default: seed when setup completes.

use std::collections::HashMap;

use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::normalize_result_status;

const SAMPLE_JSON: &str = include_str!("../seed/kyiv-concert-2026.json");
const SAMPLE_PROJECT_KEY: &str = "KYIV_CONCERT_2026";

#[derive(Debug, Default, Deserialize, Clone)]
struct Localized {
    #[serde(default)]
    en: String,
    #[serde(default)]
    uk: String,
}

impl Localized {
    fn pick(&self, locale: &str) -> &str {
        if locale.eq_ignore_ascii_case("uk") && !self.uk.is_empty() {
            &self.uk
        } else {
            &self.en
        }
    }
}

#[derive(Debug, Deserialize)]
struct SeedFile {
    project: SeedProject,
    suites: Vec<SeedSuite>,
    #[serde(default)]
    requirements: Vec<SeedRequirement>,
    #[serde(default)]
    runs: Vec<SeedRun>,
}

#[derive(Debug, Deserialize)]
struct SeedProject {
    key: String,
    name: Localized,
    description: Localized,
}

#[derive(Debug, Deserialize)]
struct SeedSuite {
    name: Localized,
    #[serde(default)]
    children: Vec<SeedSuite>,
    #[serde(default)]
    cases: Vec<SeedCase>,
}

#[derive(Debug, Deserialize)]
struct SeedCase {
    key: String,
    title: Localized,
    #[serde(default)]
    description: Localized,
    #[serde(default)]
    precondition: Localized,
    #[serde(default = "default_priority")]
    priority: String,
    #[serde(default = "default_case_status")]
    status: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    steps: Vec<SeedStep>,
}

fn default_priority() -> String {
    "medium".into()
}
fn default_case_status() -> String {
    "ready".into()
}

#[derive(Debug, Deserialize)]
struct SeedStep {
    action: Localized,
    expected_result: Localized,
    #[serde(default)]
    data: Localized,
}

#[derive(Debug, Deserialize)]
struct SeedRequirement {
    external_key: String,
    title: Localized,
    #[serde(default = "default_source")]
    source: String,
    #[serde(default)]
    case_keys: Vec<String>,
}

fn default_source() -> String {
    "sample".into()
}

#[derive(Debug, Deserialize)]
struct SeedRun {
    name: Localized,
    #[serde(default)]
    description: Localized,
    status: String,
    #[serde(default)]
    results: Vec<SeedResult>,
}

#[derive(Debug, Deserialize)]
struct SeedResult {
    case_key: String,
    status: String,
    #[serde(default)]
    comment: Localized,
    #[serde(default)]
    history: Vec<SeedHistory>,
}

#[derive(Debug, Deserialize)]
struct SeedHistory {
    status: String,
    #[serde(default)]
    message: Localized,
}

/// Whether sample seeding is enabled (default true).
pub fn sample_seed_enabled() -> bool {
    match std::env::var("VULYK_SEED_SAMPLE") {
        Ok(v) => !matches!(v.to_lowercase().as_str(), "0" | "false" | "no" | "off"),
        Err(_) => true,
    }
}

/// Seed the sample project if missing. Idempotent by project key.
pub async fn seed_sample_project(pool: &PgPool, locale: &str) -> AppResult<Option<Uuid>> {
    if !sample_seed_enabled() {
        tracing::info!("VULYK_SEED_SAMPLE disabled; skipping sample project");
        return Ok(None);
    }

    let existing: Option<Uuid> = sqlx::query_scalar("SELECT id FROM projects WHERE key = $1")
        .bind(SAMPLE_PROJECT_KEY)
        .fetch_optional(pool)
        .await?;
    if let Some(id) = existing {
        tracing::info!("Sample project {SAMPLE_PROJECT_KEY} already present ({id})");
        // Backfill executions tree when migrating older seeds.
        let _ = ensure_sample_executions(pool, id).await;
        return Ok(Some(id));
    }

    let seed: SeedFile = serde_json::from_str(SAMPLE_JSON)
        .map_err(|e| AppError::Internal(format!("sample seed JSON: {e}")))?;

    let project_id = insert_project(pool, &seed.project, locale).await?;
    let mut case_ids: HashMap<String, Uuid> = HashMap::new();

    for (idx, suite) in seed.suites.iter().enumerate() {
        insert_suite_tree(
            pool,
            project_id,
            None,
            suite,
            locale,
            idx as i32,
            &mut case_ids,
        )
        .await?;
    }

    for req in &seed.requirements {
        let req_id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO requirements (project_id, external_key, title, source)
               VALUES ($1, $2, $3, $4)
               RETURNING id"#,
        )
        .bind(project_id)
        .bind(&req.external_key)
        .bind(req.title.pick(locale))
        .bind(&req.source)
        .fetch_one(pool)
        .await?;

        for key in &req.case_keys {
            if let Some(case_id) = case_ids.get(key) {
                sqlx::query(
                    r#"INSERT INTO case_requirements (case_id, requirement_id)
                       VALUES ($1, $2)
                       ON CONFLICT DO NOTHING"#,
                )
                .bind(case_id)
                .bind(req_id)
                .execute(pool)
                .await?;
            }
        }
    }

    for run in &seed.runs {
        insert_run(pool, project_id, run, locale, &case_ids).await?;
    }

    ensure_sample_executions(pool, project_id).await?;

    // Align monotonic counter with highest seeded PREFIX-N number.
    sqlx::query(
        r#"UPDATE projects p
           SET next_case_number = COALESCE((
             SELECT MAX((regexp_match(tc.key, '^[A-Z][A-Z0-9]{1,7}-([0-9]+)$'))[1]::INT)
             FROM test_cases tc
             WHERE tc.project_id = p.id
               AND tc.key ~ ('^' || p.case_key_prefix || '-[0-9]+$')
           ), 0)
           WHERE p.id = $1"#,
    )
    .bind(project_id)
    .execute(pool)
    .await?;

    // Grant admin membership to all system admins so the sample is visible.
    sqlx::query(
        r#"INSERT INTO memberships (project_id, user_id, role)
           SELECT $1, id, 'admin'
           FROM users
           WHERE is_system_admin = TRUE AND disabled_at IS NULL
           ON CONFLICT (project_id, user_id) DO NOTHING"#,
    )
    .bind(project_id)
    .execute(pool)
    .await?;

    // Demo user gets test_automation_engineer on the sample project when present.
    sqlx::query(
        r#"INSERT INTO memberships (project_id, user_id, role)
           SELECT $1, id, 'test_automation_engineer'
           FROM users
           WHERE email = $2 AND disabled_at IS NULL
           ON CONFLICT (project_id, user_id) DO NOTHING"#,
    )
    .bind(project_id)
    .bind(
        std::env::var("VULYK_DEMO_EMAIL")
            .unwrap_or_else(|_| "user@vulyk.local".into())
            .to_lowercase(),
    )
    .execute(pool)
    .await?;

    tracing::info!(
        "Seeded sample project {SAMPLE_PROJECT_KEY} with {} cases",
        case_ids.len()
    );
    Ok(Some(project_id))
}

async fn insert_project(pool: &PgPool, project: &SeedProject, locale: &str) -> AppResult<Uuid> {
    use crate::services::case_keys::{derive_prefix_from_name, unique_prefix};

    let name = project.name.pick(locale);
    // Prefer prefix inferred from seeded case keys (KC-001 → KC) so sample stays stable;
    // otherwise derive initials from the display name (Kyiv Open-Air Concert → KOC).
    let preferred = seed_preferred_prefix().unwrap_or_else(|| derive_prefix_from_name(name));
    let prefix = unique_prefix(pool, &preferred, None).await?;

    let id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO projects (key, name, description, case_key_prefix, next_case_number)
           VALUES ($1, $2, $3, $4, 0)
           RETURNING id"#,
    )
    .bind(project.key.trim().to_uppercase())
    .bind(name)
    .bind(project.description.pick(locale))
    .bind(&prefix)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

fn seed_preferred_prefix() -> Option<String> {
    // Peek at first case key in the embedded JSON without full parse cost beyond serde.
    let seed: SeedFile = serde_json::from_str(SAMPLE_JSON).ok()?;
    for suite in &seed.suites {
        if let Some(p) = find_prefix_in_suite(suite) {
            return Some(p);
        }
    }
    None
}

fn find_prefix_in_suite(suite: &SeedSuite) -> Option<String> {
    for case in &suite.cases {
        if let Some((prefix, _)) = case.key.rsplit_once('-') {
            let p = prefix.trim().to_uppercase();
            if p.len() >= 2 {
                return Some(p);
            }
        }
    }
    for child in &suite.children {
        if let Some(p) = find_prefix_in_suite(child) {
            return Some(p);
        }
    }
    None
}

async fn insert_suite_tree(
    pool: &PgPool,
    project_id: Uuid,
    parent_id: Option<Uuid>,
    suite: &SeedSuite,
    locale: &str,
    sort_order: i32,
    case_ids: &mut HashMap<String, Uuid>,
) -> AppResult<Uuid> {
    let suite_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO suites (project_id, parent_id, name, sort_order)
           VALUES ($1, $2, $3, $4)
           RETURNING id"#,
    )
    .bind(project_id)
    .bind(parent_id)
    .bind(suite.name.pick(locale))
    .bind(sort_order)
    .fetch_one(pool)
    .await?;

    for (idx, child) in suite.children.iter().enumerate() {
        Box::pin(insert_suite_tree(
            pool,
            project_id,
            Some(suite_id),
            child,
            locale,
            idx as i32,
            case_ids,
        ))
        .await?;
    }

    for case in &suite.cases {
        let case_id = insert_case(pool, project_id, suite_id, case, locale).await?;
        case_ids.insert(case.key.clone(), case_id);
    }

    Ok(suite_id)
}

async fn insert_case(
    pool: &PgPool,
    project_id: Uuid,
    suite_id: Uuid,
    case: &SeedCase,
    locale: &str,
) -> AppResult<Uuid> {
    let case_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO test_cases
           (project_id, suite_id, key, title, description, precondition, priority, status)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
           RETURNING id"#,
    )
    .bind(project_id)
    .bind(suite_id)
    .bind(case.key.trim())
    .bind(case.title.pick(locale))
    .bind(case.description.pick(locale))
    .bind(case.precondition.pick(locale))
    .bind(&case.priority)
    .bind(&case.status)
    .fetch_one(pool)
    .await?;

    for (pos, step) in case.steps.iter().enumerate() {
        sqlx::query(
            r#"INSERT INTO test_steps (case_id, position, action, expected_result, data)
               VALUES ($1, $2, $3, $4, $5)"#,
        )
        .bind(case_id)
        .bind(pos as i32)
        .bind(step.action.pick(locale))
        .bind(step.expected_result.pick(locale))
        .bind(step.data.pick(locale))
        .execute(pool)
        .await?;
    }

    for tag_name in &case.tags {
        let tag = tag_name.trim();
        if tag.is_empty() {
            continue;
        }
        let tag_id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO tags (project_id, name)
               VALUES ($1, $2)
               ON CONFLICT (project_id, name) DO UPDATE SET name = EXCLUDED.name
               RETURNING id"#,
        )
        .bind(project_id)
        .bind(tag)
        .fetch_one(pool)
        .await?;
        sqlx::query(
            r#"INSERT INTO case_tags (case_id, tag_id)
               VALUES ($1, $2)
               ON CONFLICT DO NOTHING"#,
        )
        .bind(case_id)
        .bind(tag_id)
        .execute(pool)
        .await?;
    }

    Ok(case_id)
}

async fn insert_run(
    pool: &PgPool,
    project_id: Uuid,
    run: &SeedRun,
    locale: &str,
    case_ids: &HashMap<String, Uuid>,
) -> AppResult<()> {
    let is_completed = run.status == "completed" || run.status == "closed";
    let is_active = run.status == "in_progress" || run.status == "running";
    let started_at = if is_completed || is_active {
        Some(chrono::Utc::now() - chrono::Duration::hours(2))
    } else {
        None
    };
    let finished_at = if is_completed {
        Some(chrono::Utc::now() - chrono::Duration::minutes(30))
    } else {
        None
    };

    let run_id: Uuid = sqlx::query_scalar(
        r#"INSERT INTO test_runs
             (project_id, name, description, status, source, started_at, finished_at)
           VALUES ($1, $2, $3, $4, 'sample', $5, $6)
           RETURNING id"#,
    )
    .bind(project_id)
    .bind(run.name.pick(locale))
    .bind(run.description.pick(locale))
    .bind(&run.status)
    .bind(started_at)
    .bind(finished_at)
    .fetch_one(pool)
    .await?;

    for (sort, result) in run.results.iter().enumerate() {
        let Some(&case_id) = case_ids.get(&result.case_key) else {
            continue;
        };
        let status = normalize_result_status(&result.status);
        let executed_at = if status == "Untested" {
            None
        } else {
            Some(chrono::Utc::now() - chrono::Duration::minutes(90 - sort as i64))
        };

        let item_id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO run_items (run_id, case_id, status, sort_order, comment, executed_at)
               VALUES ($1, $2, $3, $4, $5, $6)
               RETURNING id"#,
        )
        .bind(run_id)
        .bind(case_id)
        .bind(&status)
        .bind(sort as i32)
        .bind(result.comment.pick(locale))
        .bind(executed_at)
        .fetch_one(pool)
        .await?;

        let history = if result.history.is_empty() {
            vec![SeedHistory {
                status: status.clone(),
                message: Localized {
                    en: result.comment.en.clone(),
                    uk: result.comment.uk.clone(),
                },
            }]
        } else {
            // Can't move out of result.history without clone — rebuild
            result
                .history
                .iter()
                .map(|h| SeedHistory {
                    status: h.status.clone(),
                    message: Localized {
                        en: h.message.en.clone(),
                        uk: h.message.uk.clone(),
                    },
                })
                .collect()
        };

        let case_version: i32 = sqlx::query_scalar("SELECT version FROM test_cases WHERE id = $1")
            .bind(case_id)
            .fetch_one(pool)
            .await
            .unwrap_or(1);

        for (hi, hist) in history.iter().enumerate() {
            let hist_status = normalize_result_status(&hist.status);
            if hist_status == "Untested" && hi == 0 && history.len() > 1 {
                // Skip pure queue markers if later entries exist — still insert for history demos
            }
            let when =
                chrono::Utc::now() - chrono::Duration::minutes(100 - (sort as i64) - (hi as i64));
            sqlx::query(
                r#"INSERT INTO test_results
                     (run_id, case_id, run_item_id, status, message, source, executed_at, case_version)
                   VALUES ($1, $2, $3, $4, $5, 'sample', $6, $7)"#,
            )
            .bind(run_id)
            .bind(case_id)
            .bind(item_id)
            .bind(&hist_status)
            .bind(hist.message.pick(locale))
            .bind(when)
            .bind(case_version)
            .execute(pool)
            .await?;
        }
    }

    Ok(())
}

/// Seed Integration_Automation campaign with daily + manual iterations and ~15 CI suites.
async fn ensure_sample_executions(pool: &PgPool, project_id: Uuid) -> AppResult<()> {
    // Skip if campaigns already exist for this project.
    let existing: i64 =
        sqlx::query_scalar("SELECT COUNT(*)::bigint FROM campaigns WHERE project_id = $1")
            .bind(project_id)
            .fetch_one(pool)
            .await?;
    if existing > 0 {
        return Ok(());
    }

    use crate::models::{CreateCampaign, CreateIteration};
    use crate::services::campaigns::{create_campaign, create_iteration, seed_historical_suite};
    use serde_json::json;

    let case_rows: Vec<(Uuid, String)> =
        sqlx::query_as("SELECT id, key FROM test_cases WHERE project_id = $1 ORDER BY key")
            .bind(project_id)
            .fetch_all(pool)
            .await?;
    if case_rows.len() < 8 {
        return Ok(());
    }

    let integration = create_campaign(
        pool,
        project_id,
        &CreateCampaign {
            name: "Integration_Automation".into(),
            description: "Nightly and on-push automation for concert stack".into(),
            parent_id: None,
            sort_order: 0,
        },
    )
    .await?;
    let _major = create_campaign(
        pool,
        project_id,
        &CreateCampaign {
            name: "Major release testing (.0)".into(),
            description: String::new(),
            parent_id: None,
            sort_order: 1,
        },
    )
    .await?;
    let non_major = create_campaign(
        pool,
        project_id,
        &CreateCampaign {
            name: "Non-major release testing (.x)".into(),
            description: String::new(),
            parent_id: None,
            sort_order: 2,
        },
    )
    .await?;

    let case_ids: Vec<Uuid> = case_rows.iter().map(|(id, _)| *id).collect();
    let daily = create_iteration(
        pool,
        project_id,
        integration.id,
        &CreateIteration {
            name: "Daily - dev (17.8) + NC 34".into(),
            reference: "Daily".into(),
            description: "Automated smoke + regression for current NC build".into(),
            status: "Running".into(),
            mode: "automated".into(),
            cadence: "daily".into(),
            scheduled_start: Some(chrono::Utc::now() - chrono::Duration::hours(6)),
            scheduled_end: Some(chrono::Utc::now() + chrono::Duration::hours(2)),
            case_ids: case_ids.clone(),
            suite_ids: vec![],
        },
    )
    .await?;
    let _older = create_iteration(
        pool,
        project_id,
        integration.id,
        &CreateIteration {
            name: "Daily - 17.7 NC33".into(),
            reference: "Daily".into(),
            description: String::new(),
            status: "Completed".into(),
            mode: "automated".into(),
            cadence: "daily".into(),
            scheduled_start: None,
            scheduled_end: None,
            case_ids: case_ids[..12.min(case_ids.len())].to_vec(),
            suite_ids: vec![],
        },
    )
    .await?;
    let _manual = create_iteration(
        pool,
        project_id,
        integration.id,
        &CreateIteration {
            name: "Manual Run".into(),
            reference: "Manual".into(),
            description: "Ad-hoc exploratory pass".into(),
            status: "Ready".into(),
            mode: "manual".into(),
            cadence: "manual".into(),
            scheduled_start: None,
            scheduled_end: None,
            case_ids: case_ids[..6.min(case_ids.len())].to_vec(),
            suite_ids: vec![],
        },
    )
    .await?;
    let _rel = create_iteration(
        pool,
        project_id,
        non_major.id,
        &CreateIteration {
            name: "17.8".into(),
            reference: "Release".into(),
            description: String::new(),
            status: "Completed".into(),
            mode: "mixed".into(),
            cadence: "on-tag".into(),
            scheduled_start: None,
            scheduled_end: None,
            case_ids: case_ids[..10.min(case_ids.len())].to_vec(),
            suite_ids: vec![],
        },
    )
    .await?;

    let statuses_rot = [
        "Passed",
        "Passed",
        "Passed",
        "Failed",
        "Skipped",
        "Passed",
        "Blocked",
        "Passed",
        "Passed",
        "Failed",
        "Passed",
        "Untestable",
        "Passed",
        "Passed",
        "Running",
    ];
    let env = json!({"runner": "Playwright", "browser": "chromium", "os": "linux"});
    let artifacts = json!([
        {"kind": "report", "label": "Playwright HTML", "url": "https://example.invalid/report/index.html"},
        {"kind": "trace", "label": "Trace", "url": "https://example.invalid/trace.zip"},
        {"kind": "video", "label": "Video", "url": "https://example.invalid/video.webm"},
        {"kind": "log", "label": "Job log", "url": "https://example.invalid/job.log"}
    ]);

    for day in 0..15 {
        let mut results: Vec<(Uuid, &str, i32)> = Vec::new();
        let take = 10.min(case_ids.len());
        for (i, case_id) in case_ids.iter().take(take).enumerate() {
            // Flip a couple of cases across days to seed flaky detection.
            let mut status = statuses_rot[(i + day) % statuses_rot.len()];
            if i == 3 && day % 2 == 0 {
                status = "Failed";
            } else if i == 3 {
                status = "Passed";
            }
            if status == "Running" {
                status = "Ready";
            }
            let ms = 800 + ((i * 337 + day * 110) % 170_000) as i32;
            results.push((*case_id, status, ms));
        }
        let name = format!(
            "CI #{} — wall-e {}",
            80 - day,
            (chrono::Utc::now() - chrono::Duration::days(day as i64)).format("%m/%d %H:%M")
        );
        seed_historical_suite(
            pool,
            project_id,
            daily.id,
            &name,
            "WALL-E-rror",
            day as i64,
            &results,
            env.clone(),
            artifacts.clone(),
        )
        .await?;
    }

    tracing::info!("Seeded sample executions tree with 15 automated suites");
    Ok(())
}
