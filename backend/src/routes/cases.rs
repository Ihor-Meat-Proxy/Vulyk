use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::{require_permission, OptionalAuthUser, Permission};
use crate::error::{AppError, AppResult};
use crate::models::{
    parse_custom_field_column_id, BulkCasesRequest, BulkCasesResponse, CaseResultsPage,
    CaseVersion, CaseVersionSummary, CreateCase, Page, PageQuery, SetCustomFieldValue, TestCase,
    TestCaseDetail, TestCaseListItem, UpdateCase,
};
use crate::routes::AppState;
use crate::services::case_export::{
    export_case_pdf_bytes, export_cases_bulk, resolve_case_globally, resolve_case_id_or_key,
    BulkExportRequest, ExportQuery,
};
use crate::services::case_keys::{
    allocate_case_key, find_case_by_key, is_key_retired, retire_case_key,
};
use crate::services::case_list::{
    fts_rank_prefix, load_custom_fields_for_cases, normalize_sort_field, order_by_sql,
    parse_cf_ids_param, parse_sort_direction, row_to_list_item,
};
use crate::services::cases::{
    add_case_tags, load_case_detail, remove_case_tags, replace_case_tags, resolve_tag_ids,
    snapshot_case,
};
use crate::services::results::{list_case_results, CaseResultsQuery};

// Re-exports for LLM/MCP routes that historically imported these helpers from here.
pub use crate::services::cases::{ensure_project, load_steps, replace_steps};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/projects/{project_id}/cases",
            get(list_cases).post(create_case),
        )
        .route("/api/v1/projects/{project_id}/cases/bulk", post(bulk_cases))
        .route(
            "/api/v1/projects/{project_id}/cases/export",
            post(export_cases_bulk_handler),
        )
        .route(
            "/api/v1/projects/{project_id}/cases/search",
            get(search_cases),
        )
        .route(
            "/api/v1/projects/{project_id}/cases/{id_or_key}/export.pdf",
            get(export_case_pdf_project),
        )
        .route(
            "/api/v1/projects/{project_id}/cases/{case_id}",
            get(get_case).put(update_case).delete(delete_case),
        )
        .route(
            "/api/v1/projects/{project_id}/cases/{case_id}/versions",
            get(list_versions),
        )
        .route(
            "/api/v1/projects/{project_id}/cases/{case_id}/versions/{version}",
            get(get_version),
        )
        .route(
            "/api/v1/projects/{project_id}/cases/{case_id}/results",
            get(list_case_results_project),
        )
        .route(
            "/api/v1/projects/{project_id}/cases/{case_id}/custom-fields",
            post(set_custom_fields),
        )
        .route(
            "/api/v1/cases/{id_or_key}/export.pdf",
            get(export_case_pdf_by_id_or_key),
        )
        .route(
            "/api/v1/cases/{case_id}/results",
            get(list_case_results_by_id),
        )
        .route("/api/v1/cases/by-key/{key}", get(get_case_by_key))
}

fn accept_language(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::ACCEPT_LANGUAGE)
        .and_then(|v| v.to_str().ok())
}

fn attachment_response(
    filename: &str,
    content_type: &str,
    body: Vec<u8>,
) -> AppResult<impl IntoResponse> {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(content_type)
            .map_err(|e| AppError::Internal(format!("content-type: {e}")))?,
    );
    let disposition = format!("attachment; filename=\"{filename}\"");
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&disposition)
            .map_err(|e| AppError::Internal(format!("content-disposition: {e}")))?,
    );
    Ok((StatusCode::OK, headers, body))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/cases/{id_or_key}/export.pdf",
    tag = "cases",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("id_or_key" = String, Path, description = "Case UUID or key (e.g. PLTK-2194)"),
        ("results" = Option<i64>, Query, description = "Latest mixed results to include (default 5, max 50)"),
        ("locale" = Option<String>, Query, description = "Label locale: en, uk, fr, es, de"),
    ),
    responses(
        (status = 200, description = "Case PDF export"),
        (status = 404, description = "Case not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn export_case_pdf_project(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, id_or_key)): Path<(Uuid, String)>,
    Query(query): Query<ExportQuery>,
    headers: HeaderMap,
) -> AppResult<impl IntoResponse> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ExportReports,
    )
    .await?;
    ensure_project(&state.pool, project_id).await?;
    let case_id = resolve_case_id_or_key(&state.pool, project_id, &id_or_key).await?;
    let locale = query.locale(accept_language(&headers));
    let (filename, bytes) = export_case_pdf_bytes(
        &state.pool,
        project_id,
        case_id,
        query.results_limit(),
        locale,
    )
    .await?;
    attachment_response(&filename, "application/pdf", bytes)
}

#[utoipa::path(
    get,
    path = "/api/v1/cases/{id_or_key}/export.pdf",
    tag = "cases",
    params(
        ("id_or_key" = String, Path, description = "Case UUID or key"),
        ("results" = Option<i64>, Query, description = "Latest mixed results to include (default 5)"),
        ("locale" = Option<String>, Query, description = "Label locale: en, uk, fr, es, de"),
    ),
    responses(
        (status = 200, description = "Case PDF export"),
        (status = 404, description = "Case not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn export_case_pdf_by_id_or_key(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(id_or_key): Path<String>,
    Query(query): Query<ExportQuery>,
    headers: HeaderMap,
) -> AppResult<impl IntoResponse> {
    let (case_id, project_id) = resolve_case_globally(&state.pool, &id_or_key).await?;
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ExportReports,
    )
    .await?;
    let locale = query.locale(accept_language(&headers));
    let (filename, bytes) = export_case_pdf_bytes(
        &state.pool,
        project_id,
        case_id,
        query.results_limit(),
        locale,
    )
    .await?;
    attachment_response(&filename, "application/pdf", bytes)
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/cases/export",
    tag = "cases",
    params(("project_id" = Uuid, Path, description = "Project id")),
    request_body = BulkExportRequest,
    responses(
        (status = 200, description = "Single PDF or ZIP of PDFs"),
        (status = 400, description = "Bad request")
    ),
    security(("bearer_auth" = []))
)]
pub async fn export_cases_bulk_handler(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
    headers: HeaderMap,
    Json(body): Json<BulkExportRequest>,
) -> AppResult<impl IntoResponse> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ExportReports,
    )
    .await?;
    ensure_project(&state.pool, project_id).await?;
    let locale = body.locale(accept_language(&headers));
    let (filename, content_type, bytes) = export_cases_bulk(
        &state.pool,
        project_id,
        &body.case_ids,
        body.results_limit(),
        locale,
    )
    .await?;
    attachment_response(&filename, content_type, bytes)
}

#[derive(Debug, Deserialize)]
pub struct ListCasesQuery {
    pub suite_id: Option<Uuid>,
    /// When true with `suite_id`, only cases directly in that folder (no descendants).
    /// Default false: selecting a parent includes all descendant folders.
    #[serde(default)]
    pub direct_only: bool,
    /// When true, only cases with no folder (`suite_id IS NULL`). Ignores `suite_id`.
    #[serde(default)]
    pub unfiled_only: bool,
    pub q: Option<String>,
    pub tag: Option<String>,
    pub status: Option<String>,
    pub priority: Option<String>,
    /// Sort field: `default`, `key`, `title`, `priority`, `status`, `automated`,
    /// `last_run_status`, `updated_at`, `created_at`, `folder`, `tags`, `sort_order`,
    /// or `cf:<uuid>` for a custom field.
    pub sort: Option<String>,
    /// `asc` (default) or `desc`.
    pub order: Option<String>,
    /// Comma-separated custom field ids (bare uuid or `cf:<uuid>`) to include in each row.
    pub cf_ids: Option<String>,
    /// JSON object map of field_id (uuid string) → value to filter on.
    pub cf: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

impl ListCasesQuery {
    fn page(&self) -> PageQuery {
        PageQuery {
            limit: self.limit,
            offset: self.offset,
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/cases",
    tag = "cases",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("suite_id" = Option<Uuid>, Query, description = "Filter by suite/folder (includes descendants by default)"),
        ("direct_only" = Option<bool>, Query, description = "If true with suite_id, only direct cases in that folder"),
        ("unfiled_only" = Option<bool>, Query, description = "If true, only cases with no suite_id"),
        ("q" = Option<String>, Query, description = "Full-text / title search"),
        ("tag" = Option<String>, Query, description = "Filter by tag name"),
        ("status" = Option<String>, Query, description = "Filter by status"),
        ("priority" = Option<String>, Query, description = "Filter by priority"),
        ("sort" = Option<String>, Query, description = "Sort field (default|key|title|priority|status|automated|last_run_status|updated_at|created_at|folder|tags|sort_order|cf:<uuid>)"),
        ("order" = Option<String>, Query, description = "asc (default) or desc"),
        ("cf_ids" = Option<String>, Query, description = "Comma-separated custom field ids to include on each row"),
        ("cf" = Option<String>, Query, description = "JSON map of custom field_id → value"),
        ("limit" = Option<i64>, Query, description = "Page size (default 50, max 500)"),
        ("offset" = Option<i64>, Query, description = "Offset (default 0)")
    ),
    responses((status = 200, description = "Paginated cases", body = Page<TestCaseListItem>)),
    security(("bearer_auth" = []))
)]
pub async fn list_cases(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
    Query(query): Query<ListCasesQuery>,
) -> AppResult<Json<Page<TestCaseListItem>>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    ensure_project(&state.pool, project_id).await?;
    let page = query.page();
    let limit = page.limit();
    let offset = page.offset();

    let sort = normalize_sort_field(query.sort.as_deref())?;
    let dir = parse_sort_direction(query.order.as_deref());
    let cf_ids = parse_cf_ids_param(query.cf_ids.as_deref())?;
    let cf_sort_id = parse_custom_field_column_id(&sort);

    let cf_filter: Option<serde_json::Value> = match query.cf.as_deref() {
        None | Some("") => None,
        Some(raw) => Some(
            serde_json::from_str(raw)
                .map_err(|e| AppError::BadRequest(format!("invalid cf JSON: {e}")))?,
        ),
    };
    if let Some(v) = &cf_filter {
        if !v.is_object() {
            return Err(AppError::BadRequest(
                "cf must be a JSON object of field_id → value".into(),
            ));
        }
    }

    // Folder filter: by default include descendants; `direct_only` scopes to that suite only.
    // `unfiled_only` returns cases with no folder (used by the tree's lazy unfiled load).
    // Must not wrap in `$2 IS NULL OR …` — that would ignore `unfiled_only` when suite_id is absent.
    let suite_predicate = if query.unfiled_only {
        "tc.suite_id IS NULL".to_string()
    } else if query.suite_id.is_none() {
        "TRUE".to_string()
    } else if query.direct_only {
        "tc.suite_id = $2".to_string()
    } else {
        "(tc.suite_id = $2 OR tc.suite_id IN (SELECT id FROM suite_tree))".to_string()
    };

    // Param layout: $1..$6 filters, $7 cf JSON, $8 limit, $9 offset, $10 cf-sort field id.
    let cf_predicate = r#"($7::jsonb IS NULL OR NOT EXISTS (
               SELECT 1 FROM jsonb_each($7::jsonb) AS f(fid, fval)
               WHERE NOT EXISTS (
                 SELECT 1 FROM custom_field_values cfv
                 WHERE cfv.case_id = tc.id
                   AND cfv.field_id = (f.fid)::uuid
                   AND (cfv.value = f.fval OR cfv.value @> f.fval)
               )
             ))"#;

    let total_sql = format!(
        r#"WITH RECURSIVE suite_tree AS (
             SELECT id FROM suites WHERE id = $2
             UNION ALL
             SELECT s.id FROM suites s
             INNER JOIN suite_tree st ON s.parent_id = st.id
           )
           SELECT COUNT(DISTINCT tc.id)::bigint
           FROM test_cases tc
           LEFT JOIN case_tags ct ON ct.case_id = tc.id
           LEFT JOIN tags tg ON tg.id = ct.tag_id
           WHERE tc.project_id = $1
             AND ({suite_predicate})
             AND ($3::text IS NULL OR tc.status = $3)
             AND ($4::text IS NULL OR tc.priority = $4)
             AND ($5::text IS NULL OR tg.name = $5)
             AND (
               $6::text IS NULL OR
               tc.search_vector @@ plainto_tsquery('english', $6) OR
               tc.title ILIKE '%' || $6 || '%' OR
               tc.key ILIKE '%' || $6 || '%'
             )
             AND {cf_predicate}"#
    );

    let total: i64 = sqlx::query_scalar(&total_sql)
        .bind(project_id)
        .bind(query.suite_id)
        .bind(query.status.as_deref())
        .bind(query.priority.as_deref())
        .bind(query.tag.as_deref())
        .bind(query.q.as_deref())
        .bind(&cf_filter)
        .fetch_one(&state.pool)
        .await?;

    let has_q = query.q.as_deref().is_some_and(|q| !q.is_empty());
    let rank_prefix = fts_rank_prefix(has_q);
    let order_sql = order_by_sql(&sort, dir);

    // $10 = optional custom-field id when sorting by cf:<uuid>.
    let cf_join = if cf_sort_id.is_some() {
        "LEFT JOIN custom_field_values cf_sort \
           ON cf_sort.case_id = tc.id AND cf_sort.field_id = $10"
    } else {
        ""
    };
    let cf_sort_select = if cf_sort_id.is_some() {
        ", cf_sort.value #>> '{}' AS cf_sort_value"
    } else {
        ", NULL::text AS cf_sort_value"
    };

    let list_sql = format!(
        r#"WITH RECURSIVE suite_tree AS (
             SELECT id FROM suites WHERE id = $2
             UNION ALL
             SELECT s.id FROM suites s
             INNER JOIN suite_tree st ON s.parent_id = st.id
           )
           SELECT tc.id, tc.project_id, tc.suite_id, tc.key, tc.title, tc.description,
                  tc.precondition, tc.priority, tc.status, tc.version, tc.external_key,
                  tc.sort_order, tc.created_at, tc.updated_at,
                  (tc.external_key IS NOT NULL) AS automated,
                  su.name AS folder_name,
                  (
                    SELECT tr.status FROM test_results tr
                    WHERE tr.case_id = tc.id
                    ORDER BY tr.executed_at DESC NULLS LAST, tr.created_at DESC
                    LIMIT 1
                  ) AS last_run_status,
                  (
                    SELECT string_agg(tg2.name, ', ' ORDER BY tg2.name)
                    FROM case_tags ct2
                    INNER JOIN tags tg2 ON tg2.id = ct2.tag_id
                    WHERE ct2.case_id = tc.id
                  ) AS tag_names
                  {cf_sort_select}
           FROM test_cases tc
           LEFT JOIN suites su ON su.id = tc.suite_id
           LEFT JOIN case_tags ct ON ct.case_id = tc.id
           LEFT JOIN tags tg ON tg.id = ct.tag_id
           {cf_join}
           WHERE tc.project_id = $1
             AND ({suite_predicate})
             AND ($3::text IS NULL OR tc.status = $3)
             AND ($4::text IS NULL OR tc.priority = $4)
             AND ($5::text IS NULL OR tg.name = $5)
             AND (
               $6::text IS NULL OR
               tc.search_vector @@ plainto_tsquery('english', $6) OR
               tc.title ILIKE '%' || $6 || '%' OR
               tc.key ILIKE '%' || $6 || '%'
             )
             AND {cf_predicate}
           GROUP BY tc.id, su.name{group_cf}
           ORDER BY
             {rank_prefix}
             {order_sql}
           LIMIT $8 OFFSET $9"#,
        group_cf = if cf_sort_id.is_some() {
            ", cf_sort.value"
        } else {
            ""
        }
    );

    let rows = if let Some(field_id) = cf_sort_id {
        sqlx::query(&list_sql)
            .bind(project_id)
            .bind(query.suite_id)
            .bind(query.status.as_deref())
            .bind(query.priority.as_deref())
            .bind(query.tag.as_deref())
            .bind(query.q.as_deref())
            .bind(&cf_filter)
            .bind(limit)
            .bind(offset)
            .bind(field_id)
            .fetch_all(&state.pool)
            .await?
    } else {
        sqlx::query(&list_sql)
            .bind(project_id)
            .bind(query.suite_id)
            .bind(query.status.as_deref())
            .bind(query.priority.as_deref())
            .bind(query.tag.as_deref())
            .bind(query.q.as_deref())
            .bind(&cf_filter)
            .bind(limit)
            .bind(offset)
            .fetch_all(&state.pool)
            .await?
    };
    let mut items: Vec<TestCaseListItem> = rows
        .iter()
        .map(row_to_list_item)
        .collect::<AppResult<_>>()?;

    if !cf_ids.is_empty() && !items.is_empty() {
        let case_ids: Vec<Uuid> = items.iter().map(|c| c.id).collect();
        let by_case = load_custom_fields_for_cases(&state.pool, &case_ids, &cf_ids).await?;
        for item in &mut items {
            if let Some(fields) = by_case.get(&item.id) {
                item.custom_fields = fields.clone();
            }
        }
    } else if !items.is_empty() {
        // No explicit cf_ids: attach fields marked show_in_list (legacy list behaviour).
        attach_list_custom_fields(&state.pool, &mut items).await?;
    }

    Ok(Json(Page::new(items, total, limit, offset)))
}

async fn attach_list_custom_fields(
    pool: &sqlx::PgPool,
    cases: &mut [TestCaseListItem],
) -> AppResult<()> {
    if cases.is_empty() {
        return Ok(());
    }
    use sqlx::Row;
    let case_ids: Vec<Uuid> = cases.iter().map(|c| c.id).collect();
    let rows = sqlx::query(
        r#"SELECT cfv.case_id, cfd.id AS field_id, cfd.name, cfd.field_type, cfd.key,
                  COALESCE(cfv.value, cfd.default_value, 'null'::jsonb) AS value
           FROM custom_field_definitions cfd
           JOIN custom_field_values cfv ON cfv.field_id = cfd.id
           WHERE cfd.show_in_list
             AND cfd.active
             AND cfv.case_id = ANY($1)
           ORDER BY cfd.sort_order, cfd.name"#,
    )
    .bind(&case_ids)
    .fetch_all(pool)
    .await?;

    use std::collections::HashMap;
    let mut by_case: HashMap<Uuid, Vec<crate::models::CustomFieldValue>> = HashMap::new();
    for row in rows {
        let case_id: Uuid = row.try_get("case_id")?;
        by_case
            .entry(case_id)
            .or_default()
            .push(crate::models::CustomFieldValue {
                field_id: row.try_get("field_id")?,
                name: row.try_get("name")?,
                field_type: row.try_get("field_type")?,
                key: row.try_get("key")?,
                value: row.try_get("value")?,
            });
    }
    for case in cases.iter_mut() {
        case.custom_fields = by_case.remove(&case.id).unwrap_or_default();
    }
    Ok(())
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/cases/search",
    tag = "cases",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("q" = Option<String>, Query, description = "Search query"),
        ("limit" = Option<i64>, Query),
        ("offset" = Option<i64>, Query)
    ),
    responses((status = 200, description = "FTS search results", body = Page<TestCaseListItem>)),
    security(("bearer_auth" = []))
)]
pub async fn search_cases(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
    Query(query): Query<ListCasesQuery>,
) -> AppResult<Json<Page<TestCaseListItem>>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    list_cases(
        State(state),
        OptionalAuthUser(auth),
        Path(project_id),
        Query(query),
    )
    .await
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/cases",
    tag = "cases",
    params(("project_id" = Uuid, Path, description = "Project id")),
    request_body = CreateCase,
    responses((status = 200, description = "Created case", body = TestCaseDetail)),
    security(("bearer_auth" = []))
)]
pub async fn create_case(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<CreateCase>,
) -> AppResult<Json<TestCaseDetail>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::CreateEditCases,
    )
    .await?;
    ensure_project(&state.pool, project_id).await?;
    if body.title.trim().is_empty() {
        return Err(AppError::BadRequest("title is required".into()));
    }
    if let Some(suite_id) = body.suite_id {
        crate::services::suites::ensure_suite_in_project(&state.pool, project_id, suite_id).await?;
    }

    let mut tx = state.pool.begin().await?;
    let (key, _number) = allocate_case_key(&mut tx, project_id).await?;
    let sort_order = body.sort_order.unwrap_or(0);

    let case = sqlx::query_as::<_, TestCase>(
        r#"INSERT INTO test_cases
             (project_id, suite_id, key, title, description, precondition, priority, status,
              external_key, sort_order)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
           RETURNING id, project_id, suite_id, key, title, description, precondition,
                     priority, status, version, external_key, sort_order, created_at, updated_at"#,
    )
    .bind(project_id)
    .bind(body.suite_id)
    .bind(&key)
    .bind(body.title.trim())
    .bind(&body.description)
    .bind(&body.precondition)
    .bind(&body.priority)
    .bind(&body.status)
    .bind(body.external_key.as_deref())
    .bind(sort_order)
    .fetch_one(&mut *tx)
    .await?;

    let _steps = replace_steps(&mut tx, case.id, &body.steps).await?;
    let tag_ids = resolve_tag_ids(&mut tx, project_id, &body.tag_ids, &body.tag_names).await?;
    replace_case_tags(&mut tx, case.id, &tag_ids).await?;
    snapshot_case(&mut tx, &case, "created").await?;
    tx.commit().await?;

    Ok(Json(load_case_detail(&state.pool, case).await?))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/cases/{case_id}",
    tag = "cases",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("case_id" = Uuid, Path, description = "Case id")
    ),
    responses((status = 200, description = "Case detail", body = TestCaseDetail)),
    security(("bearer_auth" = []))
)]
pub async fn get_case(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, case_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<TestCaseDetail>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    let case = sqlx::query_as::<_, TestCase>(
        r#"SELECT id, project_id, suite_id, key, title, description, precondition,
                  priority, status, version, external_key, sort_order, created_at, updated_at
           FROM test_cases WHERE project_id = $1 AND id = $2"#,
    )
    .bind(project_id)
    .bind(case_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("case".into()))?;

    Ok(Json(load_case_detail(&state.pool, case).await?))
}

#[utoipa::path(
    get,
    path = "/api/v1/cases/by-key/{key}",
    tag = "cases",
    params(
        ("key" = String, Path, description = "Human-readable case key (PREFIX-N), case-insensitive")
    ),
    responses(
        (status = 200, description = "Case detail", body = TestCaseDetail),
        (status = 404, description = "Unknown key"),
        (status = 410, description = "Case key deleted / retired")
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_case_by_key(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(key): Path<String>,
) -> AppResult<Json<TestCaseDetail>> {
    let case = find_case_by_key(&state.pool, &key).await?;
    if let Some(case) = case {
        require_permission(
            &state.pool,
            &auth,
            state.auth.required,
            case.project_id,
            Permission::ViewProject,
        )
        .await?;
        return Ok(Json(load_case_detail(&state.pool, case).await?));
    }

    if is_key_retired(&state.pool, &key).await? {
        return Err(AppError::Gone(format!("case key {key} deleted")));
    }

    Err(AppError::NotFound("case".into()))
}

#[utoipa::path(
    put,
    path = "/api/v1/projects/{project_id}/cases/{case_id}",
    tag = "cases",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("case_id" = Uuid, Path, description = "Case id")
    ),
    request_body = UpdateCase,
    responses((status = 200, description = "Updated case", body = TestCaseDetail)),
    security(("bearer_auth" = []))
)]
pub async fn update_case(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, case_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateCase>,
) -> AppResult<Json<TestCaseDetail>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::CreateEditCases,
    )
    .await?;
    let mut tx = state.pool.begin().await?;

    let existing = sqlx::query_as::<_, TestCase>(
        r#"SELECT id, project_id, suite_id, key, title, description, precondition,
                  priority, status, version, external_key, sort_order, created_at, updated_at
           FROM test_cases WHERE project_id = $1 AND id = $2"#,
    )
    .bind(project_id)
    .bind(case_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("case".into()))?;

    // Snapshot current version before mutating.
    snapshot_case(
        &mut tx,
        &existing,
        body.change_summary
            .as_deref()
            .unwrap_or("pre-update snapshot"),
    )
    .await?;

    let title = body.title.unwrap_or(existing.title);
    let suite_id = if body.clear_suite {
        None
    } else {
        body.suite_id.or(existing.suite_id)
    };
    // Only re-check when the client explicitly sets a new suite_id.
    if !body.clear_suite {
        if let Some(sid) = body.suite_id {
            crate::services::suites::ensure_suite_in_project(&state.pool, project_id, sid).await?;
        }
    }
    let description = body.description.unwrap_or(existing.description);
    let precondition = body.precondition.unwrap_or(existing.precondition);
    let priority = body.priority.unwrap_or(existing.priority);
    let status = body.status.unwrap_or(existing.status);
    let sort_order = body.sort_order.unwrap_or(existing.sort_order);
    let external_key = body.external_key.or(existing.external_key);

    let case = sqlx::query_as::<_, TestCase>(
        r#"UPDATE test_cases
           SET title = $3, suite_id = $4, description = $5, precondition = $6,
               priority = $7, status = $8, sort_order = $9, external_key = $10,
               version = version + 1, updated_at = NOW()
           WHERE project_id = $1 AND id = $2
           RETURNING id, project_id, suite_id, key, title, description, precondition,
                     priority, status, version, external_key, sort_order, created_at, updated_at"#,
    )
    .bind(project_id)
    .bind(case_id)
    .bind(title)
    .bind(suite_id)
    .bind(description)
    .bind(precondition)
    .bind(priority)
    .bind(status)
    .bind(sort_order)
    .bind(external_key)
    .fetch_one(&mut *tx)
    .await?;

    if let Some(ref steps) = body.steps {
        replace_steps(&mut tx, case.id, steps).await?;
    }

    if body.tag_ids.is_some() || body.tag_names.is_some() {
        let tag_ids = resolve_tag_ids(
            &mut tx,
            project_id,
            body.tag_ids.as_deref().unwrap_or(&[]),
            body.tag_names.as_deref().unwrap_or(&[]),
        )
        .await?;
        replace_case_tags(&mut tx, case.id, &tag_ids).await?;
    }

    tx.commit().await?;
    Ok(Json(load_case_detail(&state.pool, case).await?))
}

#[utoipa::path(
    delete,
    path = "/api/v1/projects/{project_id}/cases/{case_id}",
    tag = "cases",
    params(
        ("project_id" = Uuid, Path, description = "Project id"),
        ("case_id" = Uuid, Path, description = "Case id")
    ),
    responses((status = 204, description = "Deleted")),
    security(("bearer_auth" = []))
)]
pub async fn delete_case(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, case_id)): Path<(Uuid, Uuid)>,
) -> AppResult<axum::http::StatusCode> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::DeleteCases,
    )
    .await?;
    let mut tx = state.pool.begin().await?;
    let key: Option<String> =
        sqlx::query_scalar("SELECT key FROM test_cases WHERE project_id = $1 AND id = $2")
            .bind(project_id)
            .bind(case_id)
            .fetch_optional(&mut *tx)
            .await?;
    let Some(key) = key else {
        return Err(AppError::NotFound("case".into()));
    };
    retire_case_key(&mut tx, project_id, &key).await?;
    sqlx::query("DELETE FROM test_cases WHERE project_id = $1 AND id = $2")
        .bind(project_id)
        .bind(case_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/cases/bulk",
    tag = "cases",
    params(("project_id" = Uuid, Path)),
    request_body = BulkCasesRequest,
    responses((status = 200, description = "Bulk result", body = BulkCasesResponse)),
    security(("bearer_auth" = []))
)]
pub async fn bulk_cases(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<BulkCasesRequest>,
) -> AppResult<Json<BulkCasesResponse>> {
    let permission = if body.action.as_str() == "delete" {
        Permission::DeleteCases
    } else {
        Permission::CreateEditCases
    };
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        permission,
    )
    .await?;
    ensure_project(&state.pool, project_id).await?;
    if body.case_ids.is_empty() {
        return Err(AppError::BadRequest("case_ids required".into()));
    }

    let mut tx = state.pool.begin().await?;
    let action = body.action.as_str();
    let affected = match action {
        "delete" => {
            let keys: Vec<(String,)> =
                sqlx::query_as("SELECT key FROM test_cases WHERE project_id = $1 AND id = ANY($2)")
                    .bind(project_id)
                    .bind(&body.case_ids)
                    .fetch_all(&mut *tx)
                    .await?;
            for (key,) in &keys {
                retire_case_key(&mut tx, project_id, key).await?;
            }
            let res = sqlx::query("DELETE FROM test_cases WHERE project_id = $1 AND id = ANY($2)")
                .bind(project_id)
                .bind(&body.case_ids)
                .execute(&mut *tx)
                .await?;
            res.rows_affected() as i64
        }
        "set_status" => {
            let status = body
                .status
                .as_deref()
                .ok_or_else(|| AppError::BadRequest("status required".into()))?;
            let res = sqlx::query(
                r#"UPDATE test_cases
                   SET status = $3, version = version + 1, updated_at = NOW()
                   WHERE project_id = $1 AND id = ANY($2)"#,
            )
            .bind(project_id)
            .bind(&body.case_ids)
            .bind(status)
            .execute(&mut *tx)
            .await?;
            res.rows_affected() as i64
        }
        "set_priority" => {
            let priority = body
                .priority
                .as_deref()
                .ok_or_else(|| AppError::BadRequest("priority required".into()))?;
            let res = sqlx::query(
                r#"UPDATE test_cases
                   SET priority = $3, version = version + 1, updated_at = NOW()
                   WHERE project_id = $1 AND id = ANY($2)"#,
            )
            .bind(project_id)
            .bind(&body.case_ids)
            .bind(priority)
            .execute(&mut *tx)
            .await?;
            res.rows_affected() as i64
        }
        "move_suite" => {
            let suite_id = if body.clear_suite {
                None
            } else {
                body.suite_id
            };
            if let Some(sid) = suite_id {
                // Validate before mutating — foreign suite_ids would hang cases
                // under another project's folder (review #38).
                crate::services::suites::ensure_suite_in_project(&state.pool, project_id, sid)
                    .await?;
            }
            let res = sqlx::query(
                r#"UPDATE test_cases
                   SET suite_id = $3, version = version + 1, updated_at = NOW()
                   WHERE project_id = $1 AND id = ANY($2)"#,
            )
            .bind(project_id)
            .bind(&body.case_ids)
            .bind(suite_id)
            .execute(&mut *tx)
            .await?;
            res.rows_affected() as i64
        }
        "add_tags" | "remove_tags" | "set_tags" => {
            // Only count / mutate cases that belong to this project (review #39).
            let case_ids = crate::services::cases::filter_case_ids_in_project(
                &state.pool,
                project_id,
                &body.case_ids,
            )
            .await?;
            let tag_ids =
                resolve_tag_ids(&mut tx, project_id, &body.tag_ids, &body.tag_names).await?;
            for case_id in &case_ids {
                match action {
                    "add_tags" => add_case_tags(&mut tx, *case_id, &tag_ids).await?,
                    "remove_tags" => remove_case_tags(&mut tx, *case_id, &tag_ids).await?,
                    _ => replace_case_tags(&mut tx, *case_id, &tag_ids).await?,
                }
            }
            case_ids.len() as i64
        }
        "set_custom_fields" => {
            if body.custom_field_values.is_empty() {
                return Err(AppError::BadRequest("custom_field_values required".into()));
            }
            // Apply outside this tx — set_case_values manages its own transaction.
            tx.rollback().await?;
            let pairs: Vec<(Uuid, serde_json::Value)> = body
                .custom_field_values
                .iter()
                .map(|i| (i.field_id, i.value.clone()))
                .collect();
            for case_id in &body.case_ids {
                crate::services::custom_fields::set_case_values(
                    &state.pool,
                    project_id,
                    *case_id,
                    &pairs,
                )
                .await?;
            }
            return Ok(Json(BulkCasesResponse {
                affected: body.case_ids.len() as i64,
                action: body.action,
            }));
        }
        other => {
            return Err(AppError::BadRequest(format!("unknown action: {other}")));
        }
    };

    tx.commit().await?;
    Ok(Json(BulkCasesResponse {
        affected,
        action: body.action,
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/cases/{case_id}/versions",
    tag = "cases",
    params(
        ("project_id" = Uuid, Path),
        ("case_id" = Uuid, Path)
    ),
    responses((status = 200, description = "Version history", body = [CaseVersionSummary])),
    security(("bearer_auth" = []))
)]
pub async fn list_versions(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, case_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<Vec<CaseVersionSummary>>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM test_cases WHERE project_id = $1 AND id = $2)",
    )
    .bind(project_id)
    .bind(case_id)
    .fetch_one(&state.pool)
    .await?;
    if !exists {
        return Err(AppError::NotFound("case".into()));
    }

    let rows = sqlx::query_as::<_, CaseVersion>(
        r#"SELECT id, case_id, version, snapshot, change_summary, created_at
           FROM case_versions WHERE case_id = $1 ORDER BY version DESC"#,
    )
    .bind(case_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(
        rows.into_iter()
            .map(|v| CaseVersionSummary {
                id: v.id,
                case_id: v.case_id,
                version: v.version,
                change_summary: v.change_summary,
                created_at: v.created_at,
            })
            .collect(),
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/cases/{case_id}/versions/{version}",
    tag = "cases",
    params(
        ("project_id" = Uuid, Path),
        ("case_id" = Uuid, Path),
        ("version" = i32, Path)
    ),
    responses((status = 200, description = "Version snapshot", body = CaseVersion)),
    security(("bearer_auth" = []))
)]
pub async fn get_version(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, case_id, version)): Path<(Uuid, Uuid, i32)>,
) -> AppResult<Json<CaseVersion>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM test_cases WHERE project_id = $1 AND id = $2)",
    )
    .bind(project_id)
    .bind(case_id)
    .fetch_one(&state.pool)
    .await?;
    if !exists {
        return Err(AppError::NotFound("case".into()));
    }

    let row = sqlx::query_as::<_, CaseVersion>(
        r#"SELECT id, case_id, version, snapshot, change_summary, created_at
           FROM case_versions WHERE case_id = $1 AND version = $2"#,
    )
    .bind(case_id)
    .bind(version)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("version".into()))?;

    Ok(Json(row))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/cases/{case_id}/custom-fields",
    tag = "cases",
    params(
        ("project_id" = Uuid, Path),
        ("case_id" = Uuid, Path)
    ),
    request_body = Vec<SetCustomFieldValue>,
    responses((status = 200, description = "Updated case", body = TestCaseDetail)),
    security(("bearer_auth" = []))
)]
pub async fn set_custom_fields(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, case_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<Vec<SetCustomFieldValue>>,
) -> AppResult<Json<TestCaseDetail>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::CreateEditCases,
    )
    .await?;
    let case = sqlx::query_as::<_, TestCase>(
        r#"SELECT id, project_id, suite_id, key, title, description, precondition,
                  priority, status, version, external_key, sort_order, created_at, updated_at
           FROM test_cases WHERE project_id = $1 AND id = $2"#,
    )
    .bind(project_id)
    .bind(case_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("case".into()))?;

    let pairs: Vec<(Uuid, serde_json::Value)> =
        body.iter().map(|i| (i.field_id, i.value.clone())).collect();
    crate::services::custom_fields::set_case_values(&state.pool, project_id, case_id, &pairs)
        .await?;

    let mut tx = state.pool.begin().await?;
    snapshot_case(&mut tx, &case, "custom fields update").await?;

    let case = sqlx::query_as::<_, TestCase>(
        r#"UPDATE test_cases SET version = version + 1, updated_at = NOW()
           WHERE id = $1
           RETURNING id, project_id, suite_id, key, title, description, precondition,
                     priority, status, version, external_key, sort_order, created_at, updated_at"#,
    )
    .bind(case_id)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(Json(load_case_detail(&state.pool, case).await?))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/cases/{case_id}/results",
    tag = "cases",
    params(
        ("project_id" = Uuid, Path),
        ("case_id" = Uuid, Path),
        ("run_id" = Option<Uuid>, Query),
        ("status" = Option<String>, Query),
        ("user_id" = Option<Uuid>, Query),
        ("from" = Option<String>, Query),
        ("to" = Option<String>, Query),
        ("limit" = Option<i64>, Query),
        ("offset" = Option<i64>, Query),
        ("include_steps" = Option<bool>, Query)
    ),
    responses((status = 200, description = "Case result history across runs", body = CaseResultsPage)),
    security(("bearer_auth" = []))
)]
pub async fn list_case_results_project(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, case_id)): Path<(Uuid, Uuid)>,
    Query(query): Query<CaseResultsQuery>,
) -> AppResult<Json<CaseResultsPage>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM test_cases WHERE project_id = $1 AND id = $2)",
    )
    .bind(project_id)
    .bind(case_id)
    .fetch_one(&state.pool)
    .await?;
    if !exists {
        return Err(AppError::NotFound("case".into()));
    }
    Ok(Json(list_case_results(&state.pool, case_id, &query).await?))
}

#[utoipa::path(
    get,
    path = "/api/v1/cases/{case_id}/results",
    tag = "cases",
    params(
        ("case_id" = Uuid, Path),
        ("run_id" = Option<Uuid>, Query),
        ("status" = Option<String>, Query),
        ("user_id" = Option<Uuid>, Query),
        ("from" = Option<String>, Query),
        ("to" = Option<String>, Query),
        ("limit" = Option<i64>, Query),
        ("offset" = Option<i64>, Query),
        ("include_steps" = Option<bool>, Query)
    ),
    responses((status = 200, description = "Case result history across runs", body = CaseResultsPage)),
    security(("bearer_auth" = []))
)]
pub async fn list_case_results_by_id(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(case_id): Path<Uuid>,
    Query(query): Query<CaseResultsQuery>,
) -> AppResult<Json<CaseResultsPage>> {
    let project_id: Option<Uuid> =
        sqlx::query_scalar("SELECT project_id FROM test_cases WHERE id = $1")
            .bind(case_id)
            .fetch_optional(&state.pool)
            .await?;
    let Some(project_id) = project_id else {
        return Err(AppError::NotFound("case".into()));
    };
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    Ok(Json(list_case_results(&state.pool, case_id, &query).await?))
}
