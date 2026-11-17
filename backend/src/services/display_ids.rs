//! Human-readable, never-reused display ids / URL slugs for campaigns, iterations, and runs.
//!
//! Scheme (per project `case_key_prefix`):
//! - Campaign: `{PREFIX}-C-{n}` (e.g. `VQA-C-3`)
//! - Iteration: `{PREFIX}-IT-{n}` (e.g. `VQA-IT-17`)
//! - Run display: `{PREFIX}-R-{n}` (e.g. `VQA-R-42`)
//! - Run URL slug: `{PREFIX}-R-{source}-{n}-{YYYYMMDDTHHMMSSZ}`
//!   e.g. `VQA-R-jenkins-42-20261116T163512Z`
//!
//! `source` ∈ `jenkins|gitlab|github|forgejo|gitea|bitbucket|manual|other`,
//! normalized from `ci_provider` / `source` / `lifecycle`.
//!
//! Counters live on `projects` (`next_*_number`) via `UPDATE … RETURNING` — same
//! pattern as case keys. Counters never decrement, so deleted entities do not
//! free sequence numbers (no tombstone table needed).

use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::error::{AppError, AppResult};

/// Normalize CI/manual provenance into a stable slug token.
pub fn normalize_run_source(
    ci_provider: Option<&str>,
    source: Option<&str>,
    lifecycle: Option<&str>,
) -> &'static str {
    let provider = ci_provider.unwrap_or("").trim().to_ascii_lowercase();
    match provider.as_str() {
        "jenkins" => return "jenkins",
        "gitlab" => return "gitlab",
        "github" => return "github",
        "forgejo" => return "forgejo",
        "gitea" => return "gitea",
        "bitbucket" => return "bitbucket",
        "other" => return "other",
        "manual" => return "manual",
        _ => {}
    }

    let src = source.unwrap_or("").trim().to_ascii_lowercase();
    if src.contains("jenkins") {
        return "jenkins";
    }
    if src.contains("gitlab") {
        return "gitlab";
    }
    if src.contains("forgejo") {
        return "forgejo";
    }
    if src.contains("gitea") {
        return "gitea";
    }
    if src.contains("github") {
        return "github";
    }
    if src.contains("bitbucket") {
        return "bitbucket";
    }

    let life = lifecycle.unwrap_or("").trim();
    if life == "ci" || src == "ci" || src.starts_with("ci:") || src.starts_with("ci/") {
        return "other";
    }
    if src.is_empty() || src == "manual" || src == "sample" || src == "api" {
        return "manual";
    }
    "other"
}

/// Allocate next campaign display id inside an open transaction.
pub async fn allocate_campaign_ids(
    tx: &mut Transaction<'_, Postgres>,
    project_id: Uuid,
) -> AppResult<(String, i32)> {
    let row: Option<(String, i32)> = sqlx::query_as(
        r#"UPDATE projects
           SET next_campaign_number = next_campaign_number + 1, updated_at = NOW()
           WHERE id = $1
           RETURNING case_key_prefix, next_campaign_number"#,
    )
    .bind(project_id)
    .fetch_optional(&mut **tx)
    .await?;

    let (prefix, number) = row.ok_or_else(|| AppError::NotFound("project".into()))?;
    let display_id = format!("{prefix}-C-{number}");
    Ok((display_id, number))
}

/// Allocate next iteration display id inside an open transaction.
pub async fn allocate_iteration_ids(
    tx: &mut Transaction<'_, Postgres>,
    project_id: Uuid,
) -> AppResult<(String, i32)> {
    let row: Option<(String, i32)> = sqlx::query_as(
        r#"UPDATE projects
           SET next_iteration_number = next_iteration_number + 1, updated_at = NOW()
           WHERE id = $1
           RETURNING case_key_prefix, next_iteration_number"#,
    )
    .bind(project_id)
    .fetch_optional(&mut **tx)
    .await?;

    let (prefix, number) = row.ok_or_else(|| AppError::NotFound("project".into()))?;
    let display_id = format!("{prefix}-IT-{number}");
    Ok((display_id, number))
}

/// Allocate next run display id + URL slug inside an open transaction.
///
/// `source_token` should already be a normalized token from [`normalize_run_source`].
pub async fn allocate_run_ids(
    tx: &mut Transaction<'_, Postgres>,
    project_id: Uuid,
    source_token: &str,
    created_at: DateTime<Utc>,
) -> AppResult<(String, String, i32)> {
    let row: Option<(String, i32)> = sqlx::query_as(
        r#"UPDATE projects
           SET next_run_number = next_run_number + 1, updated_at = NOW()
           WHERE id = $1
           RETURNING case_key_prefix, next_run_number"#,
    )
    .bind(project_id)
    .fetch_optional(&mut **tx)
    .await?;

    let (prefix, number) = row.ok_or_else(|| AppError::NotFound("project".into()))?;
    let src = match source_token.trim().to_ascii_lowercase().as_str() {
        "jenkins" | "gitlab" | "github" | "forgejo" | "gitea" | "bitbucket" | "manual"
        | "other" => source_token.trim().to_ascii_lowercase(),
        _ => "other".to_string(),
    };
    let ts = created_at.format("%Y%m%dT%H%M%SZ");
    let display_id = format!("{prefix}-R-{number}");
    let slug = format!("{prefix}-R-{src}-{number}-{ts}");
    Ok((display_id, slug, number))
}

/// Resolve a run path param: UUID, `display_id`, or `slug`.
///
/// When `project_id` is `Some`, lookup is scoped to that project (except UUID
/// which still verifies the project ownership).
pub async fn resolve_run_id(
    pool: &PgPool,
    project_id: Option<Uuid>,
    id_or_slug: &str,
) -> AppResult<Uuid> {
    let raw = id_or_slug.trim();
    if raw.is_empty() {
        return Err(AppError::NotFound("run".into()));
    }

    if let Ok(id) = Uuid::parse_str(raw) {
        let found: Option<Uuid> = if let Some(pid) = project_id {
            sqlx::query_scalar("SELECT id FROM test_runs WHERE id = $1 AND project_id = $2")
                .bind(id)
                .bind(pid)
                .fetch_optional(pool)
                .await?
        } else {
            sqlx::query_scalar("SELECT id FROM test_runs WHERE id = $1")
                .bind(id)
                .fetch_optional(pool)
                .await?
        };
        return found.ok_or_else(|| AppError::NotFound("run".into()));
    }

    let needle = raw.to_ascii_uppercase();
    let found: Option<Uuid> = if let Some(pid) = project_id {
        sqlx::query_scalar(
            r#"SELECT id FROM test_runs
               WHERE project_id = $1
                 AND (upper(display_id) = $2 OR upper(slug) = $2)
               LIMIT 1"#,
        )
        .bind(pid)
        .bind(&needle)
        .fetch_optional(pool)
        .await?
    } else {
        sqlx::query_scalar(
            r#"SELECT id FROM test_runs
               WHERE upper(display_id) = $1 OR upper(slug) = $1
               LIMIT 1"#,
        )
        .bind(&needle)
        .fetch_optional(pool)
        .await?
    };
    found.ok_or_else(|| AppError::NotFound("run".into()))
}

/// Resolve an iteration path param: UUID or `display_id`.
pub async fn resolve_iteration_id(
    pool: &PgPool,
    project_id: Option<Uuid>,
    id_or_display: &str,
) -> AppResult<Uuid> {
    let raw = id_or_display.trim();
    if raw.is_empty() {
        return Err(AppError::NotFound("iteration".into()));
    }

    if let Ok(id) = Uuid::parse_str(raw) {
        let found: Option<Uuid> = if let Some(pid) = project_id {
            sqlx::query_scalar("SELECT id FROM iterations WHERE id = $1 AND project_id = $2")
                .bind(id)
                .bind(pid)
                .fetch_optional(pool)
                .await?
        } else {
            sqlx::query_scalar("SELECT id FROM iterations WHERE id = $1")
                .bind(id)
                .fetch_optional(pool)
                .await?
        };
        return found.ok_or_else(|| AppError::NotFound("iteration".into()));
    }

    let needle = raw.to_ascii_uppercase();
    let found: Option<Uuid> = if let Some(pid) = project_id {
        sqlx::query_scalar(
            r#"SELECT id FROM iterations
               WHERE project_id = $1 AND upper(display_id) = $2
               LIMIT 1"#,
        )
        .bind(pid)
        .bind(&needle)
        .fetch_optional(pool)
        .await?
    } else {
        sqlx::query_scalar(r#"SELECT id FROM iterations WHERE upper(display_id) = $1 LIMIT 1"#)
            .bind(&needle)
            .fetch_optional(pool)
            .await?
    };
    found.ok_or_else(|| AppError::NotFound("iteration".into()))
}

/// Resolve a campaign path param: UUID or `display_id`.
pub async fn resolve_campaign_id(
    pool: &PgPool,
    project_id: Option<Uuid>,
    id_or_display: &str,
) -> AppResult<Uuid> {
    let raw = id_or_display.trim();
    if raw.is_empty() {
        return Err(AppError::NotFound("campaign".into()));
    }

    if let Ok(id) = Uuid::parse_str(raw) {
        let found: Option<Uuid> = if let Some(pid) = project_id {
            sqlx::query_scalar("SELECT id FROM campaigns WHERE id = $1 AND project_id = $2")
                .bind(id)
                .bind(pid)
                .fetch_optional(pool)
                .await?
        } else {
            sqlx::query_scalar("SELECT id FROM campaigns WHERE id = $1")
                .bind(id)
                .fetch_optional(pool)
                .await?
        };
        return found.ok_or_else(|| AppError::NotFound("campaign".into()));
    }

    let needle = raw.to_ascii_uppercase();
    let found: Option<Uuid> = if let Some(pid) = project_id {
        sqlx::query_scalar(
            r#"SELECT id FROM campaigns
               WHERE project_id = $1 AND upper(display_id) = $2
               LIMIT 1"#,
        )
        .bind(pid)
        .bind(&needle)
        .fetch_optional(pool)
        .await?
    } else {
        sqlx::query_scalar(r#"SELECT id FROM campaigns WHERE upper(display_id) = $1 LIMIT 1"#)
            .bind(&needle)
            .fetch_optional(pool)
            .await?
    };
    found.ok_or_else(|| AppError::NotFound("campaign".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_known_providers() {
        assert_eq!(normalize_run_source(Some("Jenkins"), None, None), "jenkins");
        assert_eq!(normalize_run_source(Some("GITHUB"), None, None), "github");
        assert_eq!(normalize_run_source(Some("Forgejo"), None, None), "forgejo");
        assert_eq!(normalize_run_source(Some("gitea"), None, None), "gitea");
        assert_eq!(
            normalize_run_source(Some("bitbucket"), None, None),
            "bitbucket"
        );
    }

    #[test]
    fn normalize_from_source_substring() {
        assert_eq!(
            normalize_run_source(None, Some("ci:gitlab"), None),
            "gitlab"
        );
        assert_eq!(
            normalize_run_source(None, Some("ci:forgejo"), None),
            "forgejo"
        );
        assert_eq!(
            normalize_run_source(None, Some("Bitbucket Pipelines"), None),
            "bitbucket"
        );
    }

    #[test]
    fn normalize_ci_lifecycle_to_other() {
        assert_eq!(normalize_run_source(None, Some("ci"), Some("ci")), "other");
        assert_eq!(normalize_run_source(None, None, Some("ci")), "other");
    }

    #[test]
    fn normalize_manual_default() {
        assert_eq!(normalize_run_source(None, None, None), "manual");
        assert_eq!(
            normalize_run_source(None, Some("manual"), Some("legacy")),
            "manual"
        );
        assert_eq!(normalize_run_source(None, Some("sample"), None), "manual");
    }
}
