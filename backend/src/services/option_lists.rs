use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::models::{
    CreateOptionList, CreateOptionListItem, OptionList, OptionListDetail, OptionListItem,
    UpdateOptionList, UpdateOptionListItem,
};

const LIST_COLS: &str = "id, key, name, description, active, created_at, updated_at";
const ITEM_COLS: &str = "id, list_id, key, label, value, colour, description, sort_order, active, deprecated, created_at, updated_at";

pub async fn list_all(pool: &PgPool) -> AppResult<Vec<OptionList>> {
    let rows = sqlx::query_as::<_, OptionList>(&format!(
        "SELECT {LIST_COLS} FROM option_lists ORDER BY name"
    ))
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn get_detail(pool: &PgPool, id: Uuid) -> AppResult<OptionListDetail> {
    let list = sqlx::query_as::<_, OptionList>(&format!(
        "SELECT {LIST_COLS} FROM option_lists WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("option list".into()))?;
    let items = list_items(pool, id).await?;
    Ok(OptionListDetail { list, items })
}

pub async fn get_by_key(pool: &PgPool, key: &str) -> AppResult<Option<OptionList>> {
    let row = sqlx::query_as::<_, OptionList>(&format!(
        "SELECT {LIST_COLS} FROM option_lists WHERE key = $1"
    ))
    .bind(key)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn list_items(pool: &PgPool, list_id: Uuid) -> AppResult<Vec<OptionListItem>> {
    let rows = sqlx::query_as::<_, OptionListItem>(&format!(
        "SELECT {ITEM_COLS} FROM option_list_items WHERE list_id = $1 ORDER BY sort_order, label"
    ))
    .bind(list_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

fn slug_key(raw: &str) -> String {
    let s: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    let s = s.trim_matches('_').to_string();
    if s.is_empty() {
        "list".into()
    } else {
        s
    }
}

pub async fn create(pool: &PgPool, body: CreateOptionList) -> AppResult<OptionListDetail> {
    let key = body.key.trim();
    let name = body.name.trim();
    if key.is_empty() || name.is_empty() {
        return Err(AppError::BadRequest("key and name are required".into()));
    }
    let mut tx = pool.begin().await?;
    let list = sqlx::query_as::<_, OptionList>(&format!(
        r#"INSERT INTO option_lists (key, name, description, active)
           VALUES ($1, $2, $3, $4)
           RETURNING {LIST_COLS}"#
    ))
    .bind(key)
    .bind(name)
    .bind(body.description.trim())
    .bind(body.active)
    .fetch_one(&mut *tx)
    .await
    .map_err(map_unique)?;

    for (idx, item) in body.items.into_iter().enumerate() {
        insert_item_tx(&mut tx, list.id, item, Some(idx as i32)).await?;
    }
    tx.commit().await?;
    get_detail(pool, list.id).await
}

pub async fn update(pool: &PgPool, id: Uuid, body: UpdateOptionList) -> AppResult<OptionList> {
    let existing = sqlx::query_as::<_, OptionList>(&format!(
        "SELECT {LIST_COLS} FROM option_lists WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("option list".into()))?;

    let key = body
        .key
        .map(|k| k.trim().to_string())
        .unwrap_or(existing.key);
    let name = body
        .name
        .map(|n| n.trim().to_string())
        .unwrap_or(existing.name);
    if key.is_empty() || name.is_empty() {
        return Err(AppError::BadRequest("key and name are required".into()));
    }
    let description = body.description.unwrap_or(existing.description);
    let active = body.active.unwrap_or(existing.active);

    let row = sqlx::query_as::<_, OptionList>(&format!(
        r#"UPDATE option_lists
           SET key = $2, name = $3, description = $4, active = $5, updated_at = NOW()
           WHERE id = $1
           RETURNING {LIST_COLS}"#
    ))
    .bind(id)
    .bind(key)
    .bind(name)
    .bind(description)
    .bind(active)
    .fetch_one(pool)
    .await
    .map_err(map_unique)?;
    Ok(row)
}

pub async fn delete_or_deactivate(pool: &PgPool, id: Uuid) -> AppResult<&'static str> {
    let in_use: bool = sqlx::query_scalar(
        r#"SELECT EXISTS(
             SELECT 1 FROM custom_field_definitions WHERE option_list_id = $1
           )"#,
    )
    .bind(id)
    .fetch_one(pool)
    .await?;

    if in_use {
        sqlx::query("UPDATE option_lists SET active = FALSE, updated_at = NOW() WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        Ok("deactivated")
    } else {
        let res = sqlx::query("DELETE FROM option_lists WHERE id = $1")
            .bind(id)
            .execute(pool)
            .await?;
        if res.rows_affected() == 0 {
            return Err(AppError::NotFound("option list".into()));
        }
        Ok("deleted")
    }
}

pub async fn add_item(
    pool: &PgPool,
    list_id: Uuid,
    body: CreateOptionListItem,
) -> AppResult<OptionListItem> {
    ensure_list(pool, list_id).await?;
    let mut tx = pool.begin().await?;
    let item = insert_item_tx(&mut tx, list_id, body, None).await?;
    tx.commit().await?;
    Ok(item)
}

async fn insert_item_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    list_id: Uuid,
    body: CreateOptionListItem,
    sort_override: Option<i32>,
) -> AppResult<OptionListItem> {
    let label = body.label.trim();
    if label.is_empty() {
        return Err(AppError::BadRequest("label is required".into()));
    }
    let key = body
        .key
        .as_ref()
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty());
    let value = body
        .value
        .as_ref()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty());
    let sort_order = sort_override.unwrap_or(body.sort_order);

    let item = sqlx::query_as::<_, OptionListItem>(&format!(
        r#"INSERT INTO option_list_items
             (list_id, key, label, value, colour, description, sort_order, active, deprecated)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
           RETURNING {ITEM_COLS}"#
    ))
    .bind(list_id)
    .bind(key)
    .bind(label)
    .bind(value)
    .bind(body.colour)
    .bind(body.description.trim())
    .bind(sort_order)
    .bind(body.active)
    .bind(body.deprecated)
    .fetch_one(&mut **tx)
    .await
    .map_err(map_unique)?;
    Ok(item)
}

pub async fn update_item(
    pool: &PgPool,
    list_id: Uuid,
    item_id: Uuid,
    body: UpdateOptionListItem,
) -> AppResult<OptionListItem> {
    let existing = sqlx::query_as::<_, OptionListItem>(&format!(
        "SELECT {ITEM_COLS} FROM option_list_items WHERE id = $1 AND list_id = $2"
    ))
    .bind(item_id)
    .bind(list_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("option list item".into()))?;

    let label = body
        .label
        .map(|l| l.trim().to_string())
        .unwrap_or(existing.label);
    if label.is_empty() {
        return Err(AppError::BadRequest("label is required".into()));
    }
    let key = body.key.or(existing.key);
    let value = body.value.or(existing.value);
    let colour = body.colour.or(existing.colour);
    let description = body.description.unwrap_or(existing.description);
    let sort_order = body.sort_order.unwrap_or(existing.sort_order);
    let active = body.active.unwrap_or(existing.active);
    let deprecated = body.deprecated.unwrap_or(existing.deprecated);

    let item = sqlx::query_as::<_, OptionListItem>(&format!(
        r#"UPDATE option_list_items
           SET key = $3, label = $4, value = $5, colour = $6, description = $7,
               sort_order = $8, active = $9, deprecated = $10, updated_at = NOW()
           WHERE id = $1 AND list_id = $2
           RETURNING {ITEM_COLS}"#
    ))
    .bind(item_id)
    .bind(list_id)
    .bind(key)
    .bind(label)
    .bind(value)
    .bind(colour)
    .bind(description)
    .bind(sort_order)
    .bind(active)
    .bind(deprecated)
    .fetch_one(pool)
    .await
    .map_err(map_unique)?;
    Ok(item)
}

pub async fn delete_or_deprecate_item(
    pool: &PgPool,
    list_id: Uuid,
    item_id: Uuid,
) -> AppResult<&'static str> {
    let existing = sqlx::query_as::<_, OptionListItem>(&format!(
        "SELECT {ITEM_COLS} FROM option_list_items WHERE id = $1 AND list_id = $2"
    ))
    .bind(item_id)
    .bind(list_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("option list item".into()))?;

    let needle = existing
        .key
        .clone()
        .or_else(|| existing.value.clone())
        .unwrap_or_else(|| existing.label.clone());

    let in_use: bool = sqlx::query_scalar(
        r#"SELECT EXISTS(
             SELECT 1 FROM custom_field_values cfv
             JOIN custom_field_definitions cfd ON cfd.id = cfv.field_id
             WHERE cfd.option_list_id = $1
               AND (
                 cfv.value #>> '{}' = $2
                 OR cfv.value @> to_jsonb($2::text)
                 OR cfv.value @> jsonb_build_array($2::text)
               )
           )"#,
    )
    .bind(list_id)
    .bind(&needle)
    .fetch_one(pool)
    .await?;

    if in_use {
        sqlx::query(
            r#"UPDATE option_list_items
               SET deprecated = TRUE, active = FALSE, updated_at = NOW()
               WHERE id = $1"#,
        )
        .bind(item_id)
        .execute(pool)
        .await?;
        Ok("deprecated")
    } else {
        sqlx::query("DELETE FROM option_list_items WHERE id = $1")
            .bind(item_id)
            .execute(pool)
            .await?;
        Ok("deleted")
    }
}

pub async fn reorder_items(
    pool: &PgPool,
    list_id: Uuid,
    item_ids: &[Uuid],
) -> AppResult<Vec<OptionListItem>> {
    ensure_list(pool, list_id).await?;
    let mut tx = pool.begin().await?;
    for (idx, item_id) in item_ids.iter().enumerate() {
        let res = sqlx::query(
            r#"UPDATE option_list_items
               SET sort_order = $3, updated_at = NOW()
               WHERE id = $1 AND list_id = $2"#,
        )
        .bind(item_id)
        .bind(list_id)
        .bind(idx as i32)
        .execute(&mut *tx)
        .await?;
        if res.rows_affected() == 0 {
            return Err(AppError::BadRequest(format!("item {item_id} not in list")));
        }
    }
    tx.commit().await?;
    list_items(pool, list_id).await
}

pub async fn ensure_list(pool: &PgPool, id: Uuid) -> AppResult<()> {
    let exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM option_lists WHERE id = $1)")
            .bind(id)
            .fetch_one(pool)
            .await?;
    if !exists {
        return Err(AppError::NotFound("option list".into()));
    }
    Ok(())
}

pub fn suggest_key(name: &str) -> String {
    slug_key(name)
}

fn map_unique(err: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(db) = &err {
        if db.constraint().is_some() {
            return AppError::Conflict("option list key already exists".into());
        }
    }
    AppError::from(err)
}
