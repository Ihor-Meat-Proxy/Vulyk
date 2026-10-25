use serde_json::json;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::{CreateStep, CustomFieldValue, Tag, TestCase, TestCaseDetail, TestStep};

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

pub async fn load_tags(pool: &sqlx::PgPool, case_id: Uuid) -> AppResult<Vec<Tag>> {
    let tags = sqlx::query_as::<_, Tag>(
        r#"SELECT t.id, t.project_id, t.name, t.color, t.created_at
           FROM tags t
           INNER JOIN case_tags ct ON ct.tag_id = t.id
           WHERE ct.case_id = $1
           ORDER BY t.name"#,
    )
    .bind(case_id)
    .fetch_all(pool)
    .await?;
    Ok(tags)
}

pub async fn load_custom_fields(
    pool: &sqlx::PgPool,
    case_id: Uuid,
) -> AppResult<Vec<CustomFieldValue>> {
    use sqlx::Row;
    let rows = sqlx::query(
        r#"SELECT cf.id AS field_id, cf.name, cf.field_type,
                  COALESCE(cfv.value, 'null'::jsonb) AS value
           FROM custom_fields cf
           INNER JOIN test_cases tc ON tc.project_id = cf.project_id
           LEFT JOIN custom_field_values cfv
             ON cfv.field_id = cf.id AND cfv.case_id = tc.id
           WHERE tc.id = $1
           ORDER BY cf.sort_order, cf.name"#,
    )
    .bind(case_id)
    .fetch_all(pool)
    .await?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(CustomFieldValue {
            field_id: row.try_get("field_id")?,
            name: row.try_get("name")?,
            field_type: row.try_get("field_type")?,
            value: row.try_get("value")?,
        });
    }
    Ok(out)
}

pub async fn load_case_detail(pool: &sqlx::PgPool, case: TestCase) -> AppResult<TestCaseDetail> {
    let steps = load_steps(pool, case.id).await?;
    let tags = load_tags(pool, case.id).await?;
    let custom_fields = load_custom_fields(pool, case.id).await?;
    Ok(TestCaseDetail {
        case,
        steps,
        tags,
        custom_fields,
    })
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

pub async fn resolve_tag_ids(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    project_id: Uuid,
    tag_ids: &[Uuid],
    tag_names: &[String],
) -> AppResult<Vec<Uuid>> {
    let mut ids = tag_ids.to_vec();
    for name in tag_names {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            continue;
        }
        let id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO tags (project_id, name)
               VALUES ($1, $2)
               ON CONFLICT (project_id, name) DO UPDATE SET name = EXCLUDED.name
               RETURNING id"#,
        )
        .bind(project_id)
        .bind(trimmed)
        .fetch_one(&mut **tx)
        .await?;
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    Ok(ids)
}

pub async fn replace_case_tags(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    case_id: Uuid,
    tag_ids: &[Uuid],
) -> AppResult<()> {
    sqlx::query("DELETE FROM case_tags WHERE case_id = $1")
        .bind(case_id)
        .execute(&mut **tx)
        .await?;
    for tag_id in tag_ids {
        sqlx::query(
            r#"INSERT INTO case_tags (case_id, tag_id) VALUES ($1, $2)
               ON CONFLICT DO NOTHING"#,
        )
        .bind(case_id)
        .bind(tag_id)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

pub async fn add_case_tags(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    case_id: Uuid,
    tag_ids: &[Uuid],
) -> AppResult<()> {
    for tag_id in tag_ids {
        sqlx::query(
            r#"INSERT INTO case_tags (case_id, tag_id) VALUES ($1, $2)
               ON CONFLICT DO NOTHING"#,
        )
        .bind(case_id)
        .bind(tag_id)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

pub async fn remove_case_tags(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    case_id: Uuid,
    tag_ids: &[Uuid],
) -> AppResult<()> {
    sqlx::query("DELETE FROM case_tags WHERE case_id = $1 AND tag_id = ANY($2)")
        .bind(case_id)
        .bind(tag_ids)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub async fn snapshot_case(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    case: &TestCase,
    change_summary: &str,
) -> AppResult<()> {
    let steps = sqlx::query_as::<_, TestStep>(
        r#"SELECT id, case_id, position, action, expected_result, data
           FROM test_steps WHERE case_id = $1 ORDER BY position"#,
    )
    .bind(case.id)
    .fetch_all(&mut **tx)
    .await?;

    let tags = sqlx::query_as::<_, Tag>(
        r#"SELECT t.id, t.project_id, t.name, t.color, t.created_at
           FROM tags t
           INNER JOIN case_tags ct ON ct.tag_id = t.id
           WHERE ct.case_id = $1
           ORDER BY t.name"#,
    )
    .bind(case.id)
    .fetch_all(&mut **tx)
    .await?;

    use sqlx::Row;
    let cf_rows = sqlx::query(
        r#"SELECT cf.id AS field_id, cf.name, cf.field_type,
                  COALESCE(cfv.value, 'null'::jsonb) AS value
           FROM custom_fields cf
           LEFT JOIN custom_field_values cfv
             ON cfv.field_id = cf.id AND cfv.case_id = $1
           WHERE cf.project_id = $2
           ORDER BY cf.sort_order, cf.name"#,
    )
    .bind(case.id)
    .bind(case.project_id)
    .fetch_all(&mut **tx)
    .await?;

    let mut custom_fields = Vec::with_capacity(cf_rows.len());
    for row in cf_rows {
        custom_fields.push(json!({
            "field_id": row.try_get::<Uuid, _>("field_id")?,
            "name": row.try_get::<String, _>("name")?,
            "field_type": row.try_get::<String, _>("field_type")?,
            "value": row.try_get::<serde_json::Value, _>("value")?,
        }));
    }

    let snapshot = json!({
        "case": case,
        "steps": steps,
        "tags": tags,
        "custom_fields": custom_fields,
    });

    sqlx::query(
        r#"INSERT INTO case_versions (case_id, version, snapshot, change_summary)
           VALUES ($1, $2, $3, $4)
           ON CONFLICT (case_id, version) DO NOTHING"#,
    )
    .bind(case.id)
    .bind(case.version)
    .bind(snapshot)
    .bind(change_summary)
    .execute(&mut **tx)
    .await?;

    Ok(())
}
