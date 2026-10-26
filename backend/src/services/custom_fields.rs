use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::{
    is_allowed_field_type, needs_option_list, normalize_field_type, AddFieldToProject,
    CreateCustomFieldDefinition, CustomField, CustomFieldDefinition,
    CustomFieldDefinitionDetail, CustomFieldProjectOverride, CustomFieldValue,
    ScopeChangeWarning, UpdateCustomFieldDefinition, UpsertProjectFieldOverride,
};

const DEF_COLS: &str = "id, key, name, field_type, entity, option_list_id, default_value, required, help_text, sort_order, show_in_list, show_in_filters, active, scope, created_at, updated_at";

pub async fn list_definitions(pool: &PgPool) -> AppResult<Vec<CustomFieldDefinitionDetail>> {
    let rows = sqlx::query_as::<_, CustomFieldDefinition>(&format!(
        "SELECT {DEF_COLS} FROM custom_field_definitions ORDER BY sort_order, name"
    ))
    .fetch_all(pool)
    .await?;
    let mut out = Vec::with_capacity(rows.len());
    for field in rows {
        out.push(enrich_definition(pool, field).await?);
    }
    Ok(out)
}

pub async fn get_definition(pool: &PgPool, id: Uuid) -> AppResult<CustomFieldDefinitionDetail> {
    let field = sqlx::query_as::<_, CustomFieldDefinition>(&format!(
        "SELECT {DEF_COLS} FROM custom_field_definitions WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("custom field".into()))?;
    enrich_definition(pool, field).await
}

async fn enrich_definition(
    pool: &PgPool,
    field: CustomFieldDefinition,
) -> AppResult<CustomFieldDefinitionDetail> {
    let overrides = sqlx::query_as::<_, CustomFieldProjectOverride>(
        r#"SELECT field_id, project_id, required_override, default_value_override, enabled
           FROM custom_field_projects WHERE field_id = $1
           ORDER BY project_id"#,
    )
    .bind(field.id)
    .fetch_all(pool)
    .await?;
    let project_ids = overrides.iter().map(|o| o.project_id).collect();
    let in_use: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM custom_field_values WHERE field_id = $1)",
    )
    .bind(field.id)
    .fetch_one(pool)
    .await?;
    Ok(CustomFieldDefinitionDetail {
        field,
        project_ids,
        project_overrides: overrides,
        in_use,
    })
}

pub async fn create_definition(
    pool: &PgPool,
    body: CreateCustomFieldDefinition,
) -> AppResult<(CustomFieldDefinitionDetail, Option<ScopeChangeWarning>)> {
    let key = body.key.trim();
    let name = body.name.trim();
    if key.is_empty() || name.is_empty() {
        return Err(AppError::BadRequest("key and name are required".into()));
    }
    if !is_allowed_field_type(&body.field_type) {
        return Err(AppError::BadRequest("invalid field_type".into()));
    }
    let field_type = normalize_field_type(&body.field_type);
    let entity = body.entity.trim();
    if !matches!(entity, "test_case" | "requirement" | "both") {
        return Err(AppError::BadRequest("invalid entity".into()));
    }
    let scope = body.scope.trim();
    if !matches!(scope, "all" | "selected") {
        return Err(AppError::BadRequest("invalid scope".into()));
    }
    if needs_option_list(&field_type) && body.option_list_id.is_none() {
        return Err(AppError::BadRequest(
            "option_list_id required for dropdown/multi_select".into(),
        ));
    }
    if scope == "selected" && body.project_ids.is_empty() {
        return Err(AppError::BadRequest(
            "project_ids required when scope is selected".into(),
        ));
    }

    let mut tx = pool.begin().await?;
    let field = sqlx::query_as::<_, CustomFieldDefinition>(&format!(
        r#"INSERT INTO custom_field_definitions
             (key, name, field_type, entity, option_list_id, default_value, required,
              help_text, sort_order, show_in_list, show_in_filters, active, scope)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)
           RETURNING {DEF_COLS}"#
    ))
    .bind(key)
    .bind(name)
    .bind(&field_type)
    .bind(entity)
    .bind(body.option_list_id)
    .bind(&body.default_value)
    .bind(body.required)
    .bind(body.help_text.trim())
    .bind(body.sort_order)
    .bind(body.show_in_list)
    .bind(body.show_in_filters)
    .bind(body.active)
    .bind(scope)
    .fetch_one(&mut *tx)
    .await
    .map_err(map_unique)?;

    if scope == "selected" {
        for project_id in &body.project_ids {
            sqlx::query(
                r#"INSERT INTO custom_field_projects (field_id, project_id, enabled)
                   VALUES ($1, $2, TRUE)
                   ON CONFLICT DO NOTHING"#,
            )
            .bind(field.id)
            .bind(project_id)
            .execute(&mut *tx)
            .await?;
        }
    }
    tx.commit().await?;
    Ok((get_definition(pool, field.id).await?, None))
}

pub async fn update_definition(
    pool: &PgPool,
    id: Uuid,
    body: UpdateCustomFieldDefinition,
) -> AppResult<(CustomFieldDefinitionDetail, Option<ScopeChangeWarning>)> {
    let existing = sqlx::query_as::<_, CustomFieldDefinition>(&format!(
        "SELECT {DEF_COLS} FROM custom_field_definitions WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("custom field".into()))?;

    let key = body
        .key
        .map(|k| k.trim().to_string())
        .unwrap_or(existing.key.clone());
    let name = body
        .name
        .map(|n| n.trim().to_string())
        .unwrap_or(existing.name.clone());
    if key.is_empty() || name.is_empty() {
        return Err(AppError::BadRequest("key and name are required".into()));
    }
    let field_type = body
        .field_type
        .as_ref()
        .map(|t| normalize_field_type(t))
        .unwrap_or_else(|| normalize_field_type(&existing.field_type));
    if !is_allowed_field_type(&field_type) {
        return Err(AppError::BadRequest("invalid field_type".into()));
    }
    let entity = body
        .entity
        .unwrap_or(existing.entity.clone());
    if !matches!(entity.as_str(), "test_case" | "requirement" | "both") {
        return Err(AppError::BadRequest("invalid entity".into()));
    }
    let scope = body.scope.clone().unwrap_or(existing.scope.clone());
    if !matches!(scope.as_str(), "all" | "selected") {
        return Err(AppError::BadRequest("invalid scope".into()));
    }
    let option_list_id = match &body.option_list_id {
        Some(v) => *v,
        None => existing.option_list_id,
    };
    if needs_option_list(&field_type) && option_list_id.is_none() {
        return Err(AppError::BadRequest(
            "option_list_id required for dropdown/multi_select".into(),
        ));
    }

    let mut warning = None;
    let mut tx = pool.begin().await?;

    sqlx::query_as::<_, CustomFieldDefinition>(&format!(
        r#"UPDATE custom_field_definitions SET
             key = $2, name = $3, field_type = $4, entity = $5, option_list_id = $6,
             default_value = $7, required = $8, help_text = $9, sort_order = $10,
             show_in_list = $11, show_in_filters = $12, active = $13, scope = $14,
             updated_at = NOW()
           WHERE id = $1
           RETURNING {DEF_COLS}"#
    ))
    .bind(id)
    .bind(&key)
    .bind(&name)
    .bind(&field_type)
    .bind(&entity)
    .bind(option_list_id)
    .bind(body.default_value.unwrap_or(existing.default_value))
    .bind(body.required.unwrap_or(existing.required))
    .bind(
        body.help_text
            .unwrap_or(existing.help_text)
            .trim()
            .to_string(),
    )
    .bind(body.sort_order.unwrap_or(existing.sort_order))
    .bind(body.show_in_list.unwrap_or(existing.show_in_list))
    .bind(body.show_in_filters.unwrap_or(existing.show_in_filters))
    .bind(body.active.unwrap_or(existing.active))
    .bind(&scope)
    .fetch_one(&mut *tx)
    .await
    .map_err(map_unique)?;

    let project_ids_update = body.project_ids.clone();
    if let Some(ref project_ids) = project_ids_update {
        if scope == "selected" {
            let previous: Vec<Uuid> = sqlx::query_scalar(
                "SELECT project_id FROM custom_field_projects WHERE field_id = $1",
            )
            .bind(id)
            .fetch_all(&mut *tx)
            .await?;

            let hidden: Vec<Uuid> = previous
                .iter()
                .copied()
                .filter(|p| !project_ids.contains(p))
                .collect();

            if !hidden.is_empty() {
                // Hide (disable) rather than delete project links so values stay.
                for pid in &hidden {
                    sqlx::query(
                        r#"UPDATE custom_field_projects
                           SET enabled = FALSE, updated_at = NOW()
                           WHERE field_id = $1 AND project_id = $2"#,
                    )
                    .bind(id)
                    .bind(pid)
                    .execute(&mut *tx)
                    .await?;
                }
                if !body.acknowledge_hide_values {
                    tx.rollback().await?;
                    return Err(AppError::BadRequest(
                        "Changing scope would hide this field from projects while retaining stored values. Pass acknowledge_hide_values=true to confirm.".into(),
                    ));
                }
                warning = Some(ScopeChangeWarning {
                    warning: "Field hidden from some projects; stored values retained.".into(),
                    hidden_project_ids: hidden,
                    values_retained: true,
                });
            }

            for pid in project_ids {
                sqlx::query(
                    r#"INSERT INTO custom_field_projects (field_id, project_id, enabled)
                       VALUES ($1, $2, TRUE)
                       ON CONFLICT (field_id, project_id) DO UPDATE
                       SET enabled = TRUE, updated_at = NOW()"#,
                )
                .bind(id)
                .bind(pid)
                .execute(&mut *tx)
                .await?;
            }
        }
    }

    if scope == "selected" && existing.scope == "all" {
        if project_ids_update.is_none() {
            tx.rollback().await?;
            return Err(AppError::BadRequest(
                "project_ids required when narrowing scope to selected".into(),
            ));
        }
        if !body.acknowledge_hide_values {
            tx.rollback().await?;
            return Err(AppError::BadRequest(
                "Narrowing scope from all projects hides the field elsewhere while retaining values. Pass acknowledge_hide_values=true.".into(),
            ));
        }
        warning = Some(ScopeChangeWarning {
            warning: "Scope narrowed to selected projects; values retained where hidden.".into(),
            hidden_project_ids: vec![],
            values_retained: true,
        });
    }

    tx.commit().await?;
    Ok((get_definition(pool, id).await?, warning))
}

pub async fn delete_or_deactivate(pool: &PgPool, id: Uuid) -> AppResult<&'static str> {
    let in_use: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM custom_field_values WHERE field_id = $1)",
    )
    .bind(id)
    .fetch_one(pool)
    .await?;
    if in_use {
        sqlx::query(
            "UPDATE custom_field_definitions SET active = FALSE, updated_at = NOW() WHERE id = $1",
        )
        .bind(id)
        .execute(pool)
        .await?;
        Ok("deactivated")
    } else {
        sqlx::query("DELETE FROM custom_field_projects WHERE field_id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        let res = sqlx::query("DELETE FROM custom_field_definitions WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        if res.rows_affected() == 0 {
            return Err(AppError::NotFound("custom field".into()));
        }
        Ok("deleted")
    }
}

/// Fields applied to a project for a given entity (test_case / requirement).
pub async fn list_for_project(
    pool: &PgPool,
    project_id: Uuid,
    entity: Option<&str>,
) -> AppResult<Vec<CustomField>> {
    let rows = sqlx::query(
        r#"SELECT cfd.id, cfd.key, cfd.name, cfd.field_type, cfd.entity, cfd.option_list_id,
                  cfd.help_text, cfd.sort_order, cfd.show_in_list, cfd.show_in_filters,
                  cfd.active, cfd.scope, cfd.created_at,
                  COALESCE(cfp.required_override, cfd.required) AS required,
                  COALESCE(cfp.default_value_override, cfd.default_value) AS default_value
           FROM custom_field_definitions cfd
           LEFT JOIN custom_field_projects cfp
             ON cfp.field_id = cfd.id AND cfp.project_id = $1
           WHERE cfd.active
             AND (
               (cfd.scope = 'all')
               OR (cfd.scope = 'selected' AND cfp.enabled IS TRUE)
             )
             AND ($2::text IS NULL OR cfd.entity = $2 OR cfd.entity = 'both')
           ORDER BY cfd.sort_order, cfd.name"#,
    )
    .bind(project_id)
    .bind(entity)
    .fetch_all(pool)
    .await?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        use sqlx::Row;
        let option_list_id: Option<Uuid> = row.try_get("option_list_id")?;
        let options = options_for_list(pool, option_list_id).await?;
        out.push(CustomField {
            id: row.try_get("id")?,
            project_id,
            key: row.try_get("key")?,
            name: row.try_get("name")?,
            field_type: row.try_get("field_type")?,
            entity: row.try_get("entity")?,
            option_list_id,
            options,
            default_value: row.try_get("default_value")?,
            required: row.try_get("required")?,
            help_text: row.try_get("help_text")?,
            sort_order: row.try_get("sort_order")?,
            show_in_list: row.try_get("show_in_list")?,
            show_in_filters: row.try_get("show_in_filters")?,
            active: row.try_get("active")?,
            scope: row.try_get("scope")?,
            created_at: row.try_get("created_at")?,
        });
    }
    Ok(out)
}

/// Selected-scope fields not yet enabled on the project ("Fields available to add").
pub async fn available_to_add(pool: &PgPool, project_id: Uuid) -> AppResult<Vec<CustomFieldDefinition>> {
    let rows = sqlx::query_as::<_, CustomFieldDefinition>(&format!(
        r#"SELECT {DEF_COLS}
           FROM custom_field_definitions cfd
           WHERE cfd.active
             AND cfd.scope = 'selected'
             AND NOT EXISTS (
               SELECT 1 FROM custom_field_projects cfp
               WHERE cfp.field_id = cfd.id AND cfp.project_id = $1 AND cfp.enabled
             )
           ORDER BY cfd.sort_order, cfd.name"#
    ))
    .bind(project_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn add_to_project(
    pool: &PgPool,
    project_id: Uuid,
    field_id: Uuid,
    body: AddFieldToProject,
) -> AppResult<CustomField> {
    let field = sqlx::query_as::<_, CustomFieldDefinition>(&format!(
        "SELECT {DEF_COLS} FROM custom_field_definitions WHERE id = $1"
    ))
    .bind(field_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("custom field".into()))?;
    if field.scope != "selected" {
        return Err(AppError::BadRequest(
            "only selected-scope fields can be added to a project manually".into(),
        ));
    }
    sqlx::query(
        r#"INSERT INTO custom_field_projects
             (field_id, project_id, required_override, default_value_override, enabled)
           VALUES ($1, $2, $3, $4, TRUE)
           ON CONFLICT (field_id, project_id) DO UPDATE SET
             required_override = EXCLUDED.required_override,
             default_value_override = EXCLUDED.default_value_override,
             enabled = TRUE,
             updated_at = NOW()"#,
    )
    .bind(field_id)
    .bind(project_id)
    .bind(body.required_override)
    .bind(body.default_value_override)
    .execute(pool)
    .await?;

    list_for_project(pool, project_id, None)
        .await?
        .into_iter()
        .find(|f| f.id == field_id)
        .ok_or_else(|| AppError::Internal("field not visible after add".into()))
}

pub async fn upsert_override(
    pool: &PgPool,
    project_id: Uuid,
    field_id: Uuid,
    body: UpsertProjectFieldOverride,
) -> AppResult<CustomFieldProjectOverride> {
    // Ensure field applies to project (all scope or selected link).
    let applies = field_applies(pool, project_id, field_id).await?;
    if !applies {
        return Err(AppError::NotFound("custom field not on project".into()));
    }

    let existing = sqlx::query_as::<_, CustomFieldProjectOverride>(
        r#"SELECT field_id, project_id, required_override, default_value_override, enabled
           FROM custom_field_projects WHERE field_id = $1 AND project_id = $2"#,
    )
    .bind(field_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await?;

    let required_override = match body.required_override {
        Some(v) => v,
        None => existing.as_ref().and_then(|e| e.required_override),
    };
    let default_value_override = match body.default_value_override {
        Some(v) => v,
        None => existing
            .as_ref()
            .and_then(|e| e.default_value_override.clone()),
    };
    let enabled = body
        .enabled
        .unwrap_or(existing.as_ref().map(|e| e.enabled).unwrap_or(true));

    let row = sqlx::query_as::<_, CustomFieldProjectOverride>(
        r#"INSERT INTO custom_field_projects
             (field_id, project_id, required_override, default_value_override, enabled)
           VALUES ($1, $2, $3, $4, $5)
           ON CONFLICT (field_id, project_id) DO UPDATE SET
             required_override = EXCLUDED.required_override,
             default_value_override = EXCLUDED.default_value_override,
             enabled = EXCLUDED.enabled,
             updated_at = NOW()
           RETURNING field_id, project_id, required_override, default_value_override, enabled"#,
    )
    .bind(field_id)
    .bind(project_id)
    .bind(required_override)
    .bind(default_value_override)
    .bind(enabled)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

pub async fn field_applies(pool: &PgPool, project_id: Uuid, field_id: Uuid) -> AppResult<bool> {
    let ok: bool = sqlx::query_scalar(
        r#"SELECT EXISTS(
             SELECT 1 FROM custom_field_definitions cfd
             LEFT JOIN custom_field_projects cfp
               ON cfp.field_id = cfd.id AND cfp.project_id = $2
             WHERE cfd.id = $1 AND cfd.active
               AND (
                 cfd.scope = 'all'
                 OR (cfd.scope = 'selected' AND cfp.enabled IS TRUE)
               )
           )"#,
    )
    .bind(field_id)
    .bind(project_id)
    .fetch_one(pool)
    .await?;
    Ok(ok)
}

pub async fn load_case_values(pool: &PgPool, case_id: Uuid) -> AppResult<Vec<CustomFieldValue>> {
    use sqlx::Row;
    let rows = sqlx::query(
        r#"SELECT cfd.id AS field_id, cfd.name, cfd.field_type, cfd.key,
                  COALESCE(cfv.value, cfd.default_value, 'null'::jsonb) AS value
           FROM test_cases tc
           JOIN custom_field_definitions cfd ON (
             cfd.active
             AND (cfd.entity = 'test_case' OR cfd.entity = 'both')
             AND (
               cfd.scope = 'all'
               OR EXISTS (
                 SELECT 1 FROM custom_field_projects cfp
                 WHERE cfp.field_id = cfd.id AND cfp.project_id = tc.project_id AND cfp.enabled
               )
             )
           )
           LEFT JOIN custom_field_values cfv
             ON cfv.field_id = cfd.id AND cfv.case_id = tc.id
           WHERE tc.id = $1
           ORDER BY cfd.sort_order, cfd.name"#,
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
            key: row.try_get("key")?,
            value: row.try_get("value")?,
        });
    }
    Ok(out)
}

pub async fn load_requirement_values(
    pool: &PgPool,
    requirement_id: Uuid,
) -> AppResult<Vec<CustomFieldValue>> {
    use sqlx::Row;
    let rows = sqlx::query(
        r#"SELECT cfd.id AS field_id, cfd.name, cfd.field_type, cfd.key,
                  COALESCE(cfv.value, cfd.default_value, 'null'::jsonb) AS value
           FROM requirements r
           JOIN custom_field_definitions cfd ON (
             cfd.active
             AND (cfd.entity = 'requirement' OR cfd.entity = 'both')
             AND (
               cfd.scope = 'all'
               OR EXISTS (
                 SELECT 1 FROM custom_field_projects cfp
                 WHERE cfp.field_id = cfd.id AND cfp.project_id = r.project_id AND cfp.enabled
               )
             )
           )
           LEFT JOIN custom_field_values cfv
             ON cfv.field_id = cfd.id AND cfv.requirement_id = r.id
           WHERE r.id = $1
           ORDER BY cfd.sort_order, cfd.name"#,
    )
    .bind(requirement_id)
    .fetch_all(pool)
    .await?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(CustomFieldValue {
            field_id: row.try_get("field_id")?,
            name: row.try_get("name")?,
            field_type: row.try_get("field_type")?,
            key: row.try_get("key")?,
            value: row.try_get("value")?,
        });
    }
    Ok(out)
}

pub async fn set_case_values(
    pool: &PgPool,
    project_id: Uuid,
    case_id: Uuid,
    values: &[(Uuid, Value)],
) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    for (field_id, value) in values {
        if !field_applies(pool, project_id, *field_id).await? {
            return Err(AppError::BadRequest(format!(
                "field {field_id} not available on project"
            )));
        }
        validate_value(pool, *field_id, value).await?;
        let updated = sqlx::query(
            r#"UPDATE custom_field_values
               SET value = $3, updated_at = NOW()
               WHERE field_id = $1 AND case_id = $2"#,
        )
        .bind(field_id)
        .bind(case_id)
        .bind(value)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if updated == 0 {
            sqlx::query(
                r#"INSERT INTO custom_field_values (field_id, case_id, value, updated_at)
                   VALUES ($1, $2, $3, NOW())"#,
            )
            .bind(field_id)
            .bind(case_id)
            .bind(value)
            .execute(&mut *tx)
            .await?;
        }
    }
    tx.commit().await?;
    Ok(())
}

pub async fn set_requirement_values(
    pool: &PgPool,
    project_id: Uuid,
    requirement_id: Uuid,
    values: &[(Uuid, Value)],
) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    for (field_id, value) in values {
        if !field_applies(pool, project_id, *field_id).await? {
            return Err(AppError::BadRequest(format!(
                "field {field_id} not available on project"
            )));
        }
        validate_value(pool, *field_id, value).await?;
        let updated = sqlx::query(
            r#"UPDATE custom_field_values
               SET value = $3, updated_at = NOW()
               WHERE field_id = $1 AND requirement_id = $2"#,
        )
        .bind(field_id)
        .bind(requirement_id)
        .bind(value)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if updated == 0 {
            sqlx::query(
                r#"INSERT INTO custom_field_values (field_id, requirement_id, value, updated_at)
                   VALUES ($1, $2, $3, NOW())"#,
            )
            .bind(field_id)
            .bind(requirement_id)
            .bind(value)
            .execute(&mut *tx)
            .await?;
        }
    }
    tx.commit().await?;
    Ok(())
}

pub async fn validate_value(pool: &PgPool, field_id: Uuid, value: &Value) -> AppResult<()> {
    let field = sqlx::query_as::<_, CustomFieldDefinition>(&format!(
        "SELECT {DEF_COLS} FROM custom_field_definitions WHERE id = $1"
    ))
    .bind(field_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("custom field".into()))?;

    let ft = normalize_field_type(&field.field_type);
    match ft.as_str() {
        "checkbox" => {
            if !value.is_null() && !value.is_boolean() {
                return Err(AppError::BadRequest("checkbox expects boolean".into()));
            }
        }
        "number" => {
            if !value.is_null() && !value.is_number() {
                return Err(AppError::BadRequest("number expects numeric value".into()));
            }
        }
        "text" | "long_text" | "url" | "date" | "dropdown" => {
            if !value.is_null() && !value.is_string() {
                return Err(AppError::BadRequest(format!("{ft} expects string")));
            }
            if ft == "url" {
                if let Some(s) = value.as_str() {
                    if !s.is_empty() && !(s.starts_with("http://") || s.starts_with("https://")) {
                        return Err(AppError::BadRequest("url must start with http(s)://".into()));
                    }
                }
            }
        }
        "multi_select" => {
            if !value.is_null() && !value.is_array() {
                return Err(AppError::BadRequest("multi_select expects array".into()));
            }
        }
        _ => {}
    }
    Ok(())
}

async fn options_for_list(pool: &PgPool, option_list_id: Option<Uuid>) -> AppResult<Value> {
    let Some(list_id) = option_list_id else {
        return Ok(json!([]));
    };
    let items = crate::services::option_lists::list_items(pool, list_id).await?;
    let arr: Vec<Value> = items
        .into_iter()
        .filter(|i| i.active && !i.deprecated)
        .map(|i| {
            json!({
                "id": i.id,
                "key": i.key,
                "label": i.label,
                "value": i.value.unwrap_or_else(|| i.label.clone()),
                "colour": i.colour,
            })
        })
        .collect();
    Ok(Value::Array(arr))
}

/// Snapshot helper for case versions / chat context.
pub async fn values_as_json(pool: &PgPool, case_id: Uuid) -> AppResult<Vec<Value>> {
    let values = load_case_values(pool, case_id).await?;
    Ok(values
        .into_iter()
        .map(|v| {
            json!({
                "field_id": v.field_id,
                "key": v.key,
                "name": v.name,
                "field_type": v.field_type,
                "value": v.value,
            })
        })
        .collect())
}

fn map_unique(err: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(db) = &err {
        if db.constraint().is_some() {
            return AppError::Conflict("custom field key already exists".into());
        }
    }
    AppError::from(err)
}
