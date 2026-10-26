use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::{require_permission, OptionalAuthUser, Permission};
use crate::error::{AppError, AppResult};
use crate::models::{
    BulkCasesRequest, BulkCasesResponse, CaseResultsPage, CaseVersion, CaseVersionSummary,
    CreateCase, Page, PageQuery, SetCustomFieldValue, TestCase, TestCaseDetail, UpdateCase,
};
use crate::routes::AppState;
use crate::services::case_keys::{allocate_case_key, retire_case_key};
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
            "/api/v1/projects/{project_id}/cases/search",
            get(search_cases),
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
            "/api/v1/cases/{case_id}/results",
            get(list_case_results_by_id),
        )
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
        ("limit" = Option<i64>, Query, description = "Page size (default 50, max 500)"),
        ("offset" = Option<i64>, Query, description = "Offset (default 0)")
    ),
    responses((status = 200, description = "Paginated cases", body = Page<TestCase>)),
    security(("bearer_auth" = []))
)]
pub async fn list_cases(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
    Query(query): Query<ListCasesQuery>,
) -> AppResult<Json<Page<TestCase>>> {
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
             )"#
    );

    let total: i64 = sqlx::query_scalar(&total_sql)
        .bind(project_id)
        .bind(query.suite_id)
        .bind(query.status.as_deref())
        .bind(query.priority.as_deref())
        .bind(query.tag.as_deref())
        .bind(query.q.as_deref())
        .fetch_one(&state.pool)
        .await?;

    // Numeric PREFIX-N ordering (PLTK-2 before PLTK-10), then key as tie-breaker.
    let list_sql = format!(
        r#"WITH RECURSIVE suite_tree AS (
             SELECT id FROM suites WHERE id = $2
             UNION ALL
             SELECT s.id FROM suites s
             INNER JOIN suite_tree st ON s.parent_id = st.id
           )
           SELECT tc.id, tc.project_id, tc.suite_id, tc.key, tc.title, tc.description,
                  tc.precondition, tc.priority, tc.status, tc.version, tc.external_key, tc.sort_order, tc.created_at, tc.updated_at
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
           GROUP BY tc.id
           ORDER BY
             CASE WHEN $6::text IS NOT NULL
               THEN ts_rank(tc.search_vector, plainto_tsquery('english', $6))
               ELSE 0 END DESC,
             tc.sort_order,
             CASE
               WHEN tc.key ~ '^[A-Z][A-Z0-9]*-[0-9]+$'
               THEN (regexp_match(tc.key, '-([0-9]+)$'))[1]::bigint
               ELSE NULL
             END ASC NULLS LAST,
             tc.key
           LIMIT $7 OFFSET $8"#
    );

    let cases = sqlx::query_as::<_, TestCase>(&list_sql)
        .bind(project_id)
        .bind(query.suite_id)
        .bind(query.status.as_deref())
        .bind(query.priority.as_deref())
        .bind(query.tag.as_deref())
        .bind(query.q.as_deref())
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.pool)
        .await?;

    Ok(Json(Page::new(cases, total, limit, offset)))
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
    responses((status = 200, description = "FTS search results", body = Page<TestCase>)),
    security(("bearer_auth" = []))
)]
pub async fn search_cases(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
    Query(query): Query<ListCasesQuery>,
) -> AppResult<Json<Page<TestCase>>> {
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
            let tag_ids =
                resolve_tag_ids(&mut tx, project_id, &body.tag_ids, &body.tag_names).await?;
            for case_id in &body.case_ids {
                match action {
                    "add_tags" => add_case_tags(&mut tx, *case_id, &tag_ids).await?,
                    "remove_tags" => remove_case_tags(&mut tx, *case_id, &tag_ids).await?,
                    _ => replace_case_tags(&mut tx, *case_id, &tag_ids).await?,
                }
            }
            body.case_ids.len() as i64
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

    let mut tx = state.pool.begin().await?;
    snapshot_case(&mut tx, &case, "custom fields update").await?;

    for item in &body {
        let owned: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM custom_fields WHERE id = $1 AND project_id = $2)",
        )
        .bind(item.field_id)
        .bind(project_id)
        .fetch_one(&mut *tx)
        .await?;
        if !owned {
            return Err(AppError::BadRequest("custom field not in project".into()));
        }
        sqlx::query(
            r#"INSERT INTO custom_field_values (case_id, field_id, value)
               VALUES ($1, $2, $3)
               ON CONFLICT (case_id, field_id) DO UPDATE SET value = EXCLUDED.value"#,
        )
        .bind(case_id)
        .bind(item.field_id)
        .bind(&item.value)
        .execute(&mut *tx)
        .await?;
    }

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
    responses((status = 200, description = "Case result history across runs", body = CaseResultsPage))
)]
pub async fn list_case_results_project(
    State(state): State<AppState>,
    Path((project_id, case_id)): Path<(Uuid, Uuid)>,
    Query(query): Query<CaseResultsQuery>,
) -> AppResult<Json<CaseResultsPage>> {
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
    responses((status = 200, description = "Case result history across runs", body = CaseResultsPage))
)]
pub async fn list_case_results_by_id(
    State(state): State<AppState>,
    Path(case_id): Path<Uuid>,
    Query(query): Query<CaseResultsQuery>,
) -> AppResult<Json<CaseResultsPage>> {
    Ok(Json(list_case_results(&state.pool, case_id, &query).await?))
}
