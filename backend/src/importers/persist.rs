use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppResult;
use crate::models::{ImportCaseDraft, ImportReport};
use crate::services::case_keys::allocate_case_key;

/// Persist preview drafts into suites + cases + steps for a project.
/// Case keys are always allocated from the project counter; original keys are stored as external_key.
pub async fn persist_cases(
    pool: &PgPool,
    project_id: Uuid,
    importer_id: &str,
    drafts: &[ImportCaseDraft],
) -> AppResult<ImportReport> {
    let mut created_cases = 0usize;
    let mut created_suites = 0usize;
    let mut skipped = 0usize;
    let mut warnings = Vec::new();
    let mut case_ids = Vec::new();

    for (idx, draft) in drafts.iter().enumerate() {
        if draft.title.trim().is_empty() {
            skipped += 1;
            warnings.push(format!("row {idx}: skipped empty title"));
            continue;
        }

        let suite_id = if let Some(name) =
            draft.suite_name.as_ref().filter(|s| !s.trim().is_empty())
        {
            let existing: Option<(Uuid,)> =
                sqlx::query_as("SELECT id FROM suites WHERE project_id = $1 AND name = $2 LIMIT 1")
                    .bind(project_id)
                    .bind(name.trim())
                    .fetch_optional(pool)
                    .await?;
            if let Some((id,)) = existing {
                Some(id)
            } else {
                let (id,): (Uuid,) = sqlx::query_as(
                    r#"INSERT INTO suites (project_id, name, sort_order)
                       VALUES ($1, $2, 0) RETURNING id"#,
                )
                .bind(project_id)
                .bind(name.trim())
                .fetch_one(pool)
                .await?;
                created_suites += 1;
                Some(id)
            }
        } else {
            None
        };

        let external_key = draft
            .external_id
            .clone()
            .or_else(|| draft.key.clone())
            .filter(|k| !k.trim().is_empty());

        let mut tx = pool.begin().await?;
        let (key, _) = allocate_case_key(&mut tx, project_id).await?;

        let inserted: Result<(Uuid,), sqlx::Error> = sqlx::query_as(
            r#"INSERT INTO test_cases
               (project_id, suite_id, key, title, description, precondition, priority, status, external_key)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
               RETURNING id"#,
        )
        .bind(project_id)
        .bind(suite_id)
        .bind(&key)
        .bind(draft.title.trim())
        .bind(&draft.description)
        .bind(&draft.precondition)
        .bind(if draft.priority.is_empty() {
            "medium"
        } else {
            draft.priority.as_str()
        })
        .bind(if draft.status.is_empty() {
            "draft"
        } else {
            draft.status.as_str()
        })
        .bind(external_key.as_deref())
        .fetch_one(&mut *tx)
        .await;

        let case_id = match inserted {
            Ok((id,)) => id,
            Err(sqlx::Error::Database(err)) if err.constraint().is_some() => {
                tx.rollback().await.ok();
                skipped += 1;
                warnings.push(format!(
                    "row {idx}: insert conflict for allocated key {key}, skipped"
                ));
                continue;
            }
            Err(e) => return Err(e.into()),
        };

        for (pos, step) in draft.steps.iter().enumerate() {
            sqlx::query(
                r#"INSERT INTO test_steps (case_id, position, action, expected_result, data)
                   VALUES ($1, $2, $3, $4, $5)"#,
            )
            .bind(case_id)
            .bind(pos as i32)
            .bind(&step.action)
            .bind(&step.expected_result)
            .bind(&step.data)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        created_cases += 1;
        case_ids.push(case_id);
    }

    Ok(ImportReport {
        importer_id: importer_id.to_string(),
        created_cases,
        created_suites,
        skipped,
        warnings,
        case_ids,
    })
}
