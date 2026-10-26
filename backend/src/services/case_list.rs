//! Case list query helpers: sort whitelist, enrichment, column preferences.

use serde_json::Value;
use sqlx::Row;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::{
    is_allowed_sort_field, parse_custom_field_column_id, CaseListColumnsConfig, CustomFieldValue,
    TestCaseListItem, CASE_LIST_SORT_FIELDS,
};

/// Resolve `order` query to ASC/DESC.
pub fn parse_sort_direction(order: Option<&str>) -> &'static str {
    match order.map(|o| o.trim().to_lowercase()).as_deref() {
        Some("desc") => "DESC",
        _ => "ASC",
    }
}

/// Validate sort field; empty / missing → `default`.
pub fn normalize_sort_field(sort: Option<&str>) -> AppResult<String> {
    let Some(raw) = sort.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok("default".into());
    };
    if !is_allowed_sort_field(raw) {
        return Err(AppError::BadRequest(format!(
            "invalid sort '{raw}'; expected one of {} or cf:<uuid>",
            CASE_LIST_SORT_FIELDS.join(", ")
        )));
    }
    Ok(raw.to_string())
}

/// Build ORDER BY fragment (no leading ORDER BY). Uses aliases from list SELECT.
/// `cf_sort_expr` is set when sorting by a custom field (bound as join).
pub fn order_by_sql(sort: &str, dir: &str) -> String {
    let nulls = "NULLS LAST";
    let key_numeric = format!(
        "CASE WHEN tc.key ~ '^[A-Z][A-Z0-9]*-[0-9]+$' \
         THEN (regexp_match(tc.key, '-([0-9]+)$'))[1]::bigint ELSE NULL END {dir} {nulls}, \
         tc.key {dir}"
    );
    let priority_rank = format!(
        "CASE lower(tc.priority) \
           WHEN 'critical' THEN 0 \
           WHEN 'high' THEN 1 \
           WHEN 'medium' THEN 2 \
           WHEN 'low' THEN 3 \
           ELSE 4 END {dir}, tc.priority {dir}"
    );

    let primary = match sort {
        "default" | "sort_order" => format!("tc.sort_order ASC, {key_numeric}"),
        "key" => key_numeric,
        "title" => format!("tc.title {dir}"),
        "priority" => priority_rank,
        "status" => format!("tc.status {dir}"),
        "updated_at" => format!("tc.updated_at {dir}"),
        "created_at" => format!("tc.created_at {dir}"),
        "automated" => format!("(tc.external_key IS NOT NULL) {dir}"),
        "last_run_status" => format!("last_run_status {dir} {nulls}"),
        "folder" => format!("folder_name {dir} {nulls}"),
        "tags" => format!("tag_names {dir} {nulls}"),
        s if parse_custom_field_column_id(s).is_some() => {
            format!("cf_sort_value {dir} {nulls}")
        }
        _ => format!("tc.sort_order ASC, {key_numeric}"),
    };

    // Stable tie-breaker.
    format!("{primary}, tc.id ASC")
}

/// FTS rank prefix when searching (always DESC relevance first).
pub fn fts_rank_prefix(has_q: bool) -> String {
    if has_q {
        "CASE WHEN $6::text IS NOT NULL \
           THEN ts_rank(tc.search_vector, plainto_tsquery('english', $6)) \
           ELSE 0 END DESC,"
            .into()
    } else {
        String::new()
    }
}

pub async fn load_case_list_columns(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    project_id: Uuid,
) -> AppResult<CaseListColumnsConfig> {
    let row: Option<(Value,)> = sqlx::query_as(
        r#"SELECT case_list_columns
           FROM user_project_preferences
           WHERE user_id = $1 AND project_id = $2"#,
    )
    .bind(user_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await?;

    Ok(match row {
        Some((value,)) => CaseListColumnsConfig::from_value(&value),
        None => CaseListColumnsConfig::default_config(),
    })
}

pub async fn save_case_list_columns(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    project_id: Uuid,
    config: &CaseListColumnsConfig,
) -> AppResult<CaseListColumnsConfig> {
    let validated = CaseListColumnsConfig::validate(config).map_err(AppError::BadRequest)?;
    let value = validated.to_value();
    sqlx::query(
        r#"INSERT INTO user_project_preferences (user_id, project_id, case_list_columns, updated_at)
           VALUES ($1, $2, $3, NOW())
           ON CONFLICT (user_id, project_id) DO UPDATE
             SET case_list_columns = EXCLUDED.case_list_columns,
                 updated_at = NOW()"#,
    )
    .bind(user_id)
    .bind(project_id)
    .bind(value)
    .execute(pool)
    .await?;
    Ok(validated)
}

pub async fn reset_case_list_columns(
    pool: &sqlx::PgPool,
    user_id: Uuid,
    project_id: Uuid,
) -> AppResult<CaseListColumnsConfig> {
    let defaults = CaseListColumnsConfig::default_config();
    // Delete row (or reset column) so GET returns defaults.
    sqlx::query(
        r#"DELETE FROM user_project_preferences
           WHERE user_id = $1 AND project_id = $2"#,
    )
    .bind(user_id)
    .bind(project_id)
    .execute(pool)
    .await?;
    Ok(defaults)
}

pub async fn load_custom_fields_for_cases(
    pool: &sqlx::PgPool,
    case_ids: &[Uuid],
    field_ids: &[Uuid],
) -> AppResult<std::collections::HashMap<Uuid, Vec<CustomFieldValue>>> {
    let mut map: std::collections::HashMap<Uuid, Vec<CustomFieldValue>> =
        std::collections::HashMap::new();
    if case_ids.is_empty() || field_ids.is_empty() {
        return Ok(map);
    }

    let rows = sqlx::query(
        r#"SELECT cfv.case_id, cfv.field_id, cf.name, cf.field_type, cfv.value
           FROM custom_field_values cfv
           INNER JOIN custom_fields cf ON cf.id = cfv.field_id
           WHERE cfv.case_id = ANY($1) AND cfv.field_id = ANY($2)
           ORDER BY cf.sort_order, cf.name"#,
    )
    .bind(case_ids)
    .bind(field_ids)
    .fetch_all(pool)
    .await?;

    for row in rows {
        let case_id: Uuid = row.try_get("case_id")?;
        map.entry(case_id).or_default().push(CustomFieldValue {
            field_id: row.try_get("field_id")?,
            name: row.try_get("name")?,
            field_type: row.try_get("field_type")?,
            value: row.try_get("value")?,
        });
    }
    Ok(map)
}

pub fn parse_cf_ids_param(raw: Option<&str>) -> AppResult<Vec<Uuid>> {
    let Some(raw) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(vec![]);
    };
    let mut out = Vec::new();
    for part in raw.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        // Accept bare uuid or cf:<uuid>
        let id = parse_custom_field_column_id(part)
            .or_else(|| Uuid::parse_str(part).ok())
            .ok_or_else(|| AppError::BadRequest(format!("invalid cf_ids entry '{part}'")))?;
        if !out.contains(&id) {
            out.push(id);
        }
    }
    Ok(out)
}

/// Map a sqlx row from the enriched list query into a list item (without custom_fields).
pub fn row_to_list_item(row: &sqlx::postgres::PgRow) -> AppResult<TestCaseListItem> {
    let tag_names_raw: Option<String> = row.try_get("tag_names")?;
    let tag_names = tag_names_raw
        .filter(|s| !s.is_empty())
        .map(|s| s.split(", ").map(|t| t.to_string()).collect())
        .unwrap_or_default();

    Ok(TestCaseListItem {
        id: row.try_get("id")?,
        project_id: row.try_get("project_id")?,
        suite_id: row.try_get("suite_id")?,
        key: row.try_get("key")?,
        title: row.try_get("title")?,
        description: row.try_get("description")?,
        precondition: row.try_get("precondition")?,
        priority: row.try_get("priority")?,
        status: row.try_get("status")?,
        version: row.try_get("version")?,
        external_key: row.try_get("external_key")?,
        sort_order: row.try_get("sort_order")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
        automated: row.try_get("automated")?,
        last_run_status: row.try_get("last_run_status")?,
        folder_name: row.try_get("folder_name")?,
        tag_names,
        custom_fields: vec![],
    })
}
