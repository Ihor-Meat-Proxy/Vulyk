//! Automation Git repositories: app-level CRUD + project M2M links.
//! Legacy singular `/projects/{id}/automation-repo` remains as a default-repo shim.

use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use uuid::Uuid;

use crate::auth::{require_permission, OptionalAuthUser, Permission};
use crate::error::{AppError, AppResult};
use crate::models::{
    AutomationRepoIndexResult, AutomationRepoPublic, AutomationRepoRow, AutomationRepoTestResult,
    CreateAutomationRepo, SetRepoProjects, TestAutomationRepoBody, UpdateAutomationRepo,
    UpsertAutomationRepo,
};
use crate::routes::cases::ensure_project;
use crate::routes::AppState;
use crate::services::automation_repo as repo_svc;
use crate::services::automation_repo_remote as remote_svc;

pub fn router() -> Router<AppState> {
    Router::new()
        // New app-level CRUD
        .route(
            "/api/v1/automation-repos",
            get(list_repos).post(create_repo),
        )
        .route(
            "/api/v1/automation-repos/test",
            axum::routing::post(test_repo_preview),
        )
        .route(
            "/api/v1/automation-repos/{id}",
            get(get_repo_by_id)
                .patch(update_repo)
                .delete(delete_repo_by_id),
        )
        .route(
            "/api/v1/automation-repos/{id}/projects",
            axum::routing::put(set_repo_projects),
        )
        .route(
            "/api/v1/automation-repos/{id}/test",
            axum::routing::post(test_repo_by_id),
        )
        .route(
            "/api/v1/automation-repos/{id}/reindex",
            axum::routing::post(reindex_repo_by_id),
        )
        // Project-scoped list (Generate picker)
        .route(
            "/api/v1/projects/{project_id}/automation-repos",
            get(list_project_repos),
        )
        // Legacy singular shim (default linked repo)
        .route(
            "/api/v1/projects/{project_id}/automation-repo",
            get(get_repo).put(upsert_repo).delete(delete_repo),
        )
        .route(
            "/api/v1/projects/{project_id}/automation-repo/test",
            axum::routing::post(test_repo),
        )
        .route(
            "/api/v1/projects/{project_id}/automation-repo/reindex",
            axum::routing::post(reindex_repo),
        )
}

const SELECT_COLS: &str = r#"id, name, project_id, provider, remote_url, branch, subpath,
    token_ciphertext, token_nonce, index_status, last_indexed_at,
    last_index_error, commit_sha, file_count, chunk_count, created_at, updated_at"#;

async fn to_public(
    pool: &sqlx::PgPool,
    row: &AutomationRepoRow,
) -> AppResult<AutomationRepoPublic> {
    let project_ids = repo_svc::project_ids_for_repo(pool, row.id).await?;
    Ok(row.to_public(project_ids))
}

fn validate_provider(provider: &str) -> AppResult<String> {
    let provider = provider.trim().to_lowercase();
    if !matches!(provider.as_str(), "github" | "gitlab" | "generic") {
        return Err(AppError::BadRequest(
            "provider must be github, gitlab, or generic".into(),
        ));
    }
    Ok(provider)
}

async fn require_manage_on_projects(
    state: &AppState,
    auth: &Option<crate::auth::AuthUser>,
    project_ids: &[Uuid],
) -> AppResult<()> {
    if project_ids.is_empty() {
        // Creating an unlinked repo requires system admin when auth is on.
        if !state.auth.required {
            return Ok(());
        }
        match auth {
            Some(u) if u.is_system_admin() => Ok(()),
            Some(_) => Err(AppError::Forbidden(
                "system admin required to create unlinked automation repos".into(),
            )),
            None => Err(AppError::Unauthorized("authentication required".into())),
        }
    } else {
        for pid in project_ids {
            require_permission(
                &state.pool,
                auth,
                state.auth.required,
                *pid,
                Permission::ManageAutomationRepo,
            )
            .await?;
        }
        Ok(())
    }
}

async fn require_manage_linked_or_admin(
    state: &AppState,
    auth: &Option<crate::auth::AuthUser>,
    repo_id: Uuid,
) -> AppResult<()> {
    let links = repo_svc::project_ids_for_repo(&state.pool, repo_id).await?;
    if links.is_empty() {
        if !state.auth.required {
            return Ok(());
        }
        match auth {
            Some(u) if u.is_system_admin() => Ok(()),
            Some(_) => Err(AppError::Forbidden(
                "system admin required for unlinked automation repos".into(),
            )),
            None => Err(AppError::Unauthorized("authentication required".into())),
        }
    } else {
        // Any linked project with ManageAutomationRepo is enough; check first that works.
        let mut last_err = None;
        for pid in &links {
            match require_permission(
                &state.pool,
                auth,
                state.auth.required,
                *pid,
                Permission::ManageAutomationRepo,
            )
            .await
            {
                Ok(()) => return Ok(()),
                Err(e) => last_err = Some(e),
            }
        }
        Err(last_err.unwrap_or_else(|| {
            AppError::Forbidden("missing permission: manage_automation_repo".into())
        }))
    }
}

// ── App-level CRUD ───────────────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/v1/automation-repos",
    tag = "automation-repo",
    responses((status = 200, body = [AutomationRepoPublic])),
    security(("bearer_auth" = []))
)]
pub async fn list_repos(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
) -> AppResult<Json<Vec<AutomationRepoPublic>>> {
    if state.auth.required && auth.is_none() {
        return Err(AppError::Unauthorized("authentication required".into()));
    }
    let is_admin = auth.as_ref().map(|u| u.is_system_admin()).unwrap_or(false);
    let user_id = auth
        .as_ref()
        .map(|u| u.id())
        .filter(|id| *id != Uuid::nil());
    let rows = repo_svc::list_repos_for_user(&state.pool, user_id, is_admin).await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in &rows {
        out.push(to_public(&state.pool, row).await?);
    }
    Ok(Json(out))
}

#[utoipa::path(
    post,
    path = "/api/v1/automation-repos",
    tag = "automation-repo",
    request_body = CreateAutomationRepo,
    responses((status = 200, body = AutomationRepoPublic)),
    security(("bearer_auth" = []))
)]
pub async fn create_repo(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Json(body): Json<CreateAutomationRepo>,
) -> AppResult<Json<AutomationRepoPublic>> {
    require_manage_on_projects(&state, &auth, &body.project_ids).await?;
    for pid in &body.project_ids {
        ensure_project(&state.pool, *pid).await?;
    }
    let provider = validate_provider(&body.provider)?;
    if body.remote_url.trim().is_empty() {
        return Err(AppError::BadRequest("remote_url required".into()));
    }
    crate::services::egress::validate_public_https_url(body.remote_url.trim())?;
    if body.name.trim().is_empty() {
        return Err(AppError::BadRequest("name required".into()));
    }
    let branch = if body.branch.trim().is_empty() {
        "main".to_string()
    } else {
        body.branch.trim().to_string()
    };
    let (ct, nonce) = match body.token.as_deref() {
        Some("") | None => (None, None),
        Some(tok) => {
            let (c, n) = state.secrets.encrypt(tok)?;
            (Some(c), Some(n))
        }
    };
    let legacy_project = body.project_ids.first().copied();
    let row = sqlx::query_as::<_, AutomationRepoRow>(&format!(
        r#"INSERT INTO automation_repos
            (name, project_id, provider, remote_url, branch, subpath, token_ciphertext, token_nonce)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
           RETURNING {SELECT_COLS}"#
    ))
    .bind(body.name.trim())
    .bind(legacy_project)
    .bind(&provider)
    .bind(body.remote_url.trim())
    .bind(&branch)
    .bind(body.subpath.trim())
    .bind(ct)
    .bind(nonce)
    .fetch_one(&state.pool)
    .await?;

    if !body.project_ids.is_empty() {
        repo_svc::set_project_links(&state.pool, row.id, &body.project_ids, legacy_project).await?;
    }
    Ok(Json(to_public(&state.pool, &row).await?))
}

#[utoipa::path(
    get,
    path = "/api/v1/automation-repos/{id}",
    tag = "automation-repo",
    params(("id" = Uuid, Path)),
    responses((status = 200, body = AutomationRepoPublic)),
    security(("bearer_auth" = []))
)]
pub async fn get_repo_by_id(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<AutomationRepoPublic>> {
    let row = repo_svc::load_repo_by_id(&state.pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound("automation repo".into()))?;
    let links = repo_svc::project_ids_for_repo(&state.pool, id).await?;
    if links.is_empty() {
        if state.auth.required {
            match &auth {
                Some(u) if u.is_system_admin() => {}
                Some(_) => return Err(AppError::Forbidden("not allowed to view this repo".into())),
                None => return Err(AppError::Unauthorized("authentication required".into())),
            }
        }
    } else {
        let mut ok = false;
        for pid in &links {
            if require_permission(
                &state.pool,
                &auth,
                state.auth.required,
                *pid,
                Permission::ViewProject,
            )
            .await
            .is_ok()
            {
                ok = true;
                break;
            }
        }
        if !ok {
            return Err(AppError::Forbidden("not allowed to view this repo".into()));
        }
    }
    Ok(Json(to_public(&state.pool, &row).await?))
}

#[utoipa::path(
    patch,
    path = "/api/v1/automation-repos/{id}",
    tag = "automation-repo",
    params(("id" = Uuid, Path)),
    request_body = UpdateAutomationRepo,
    responses((status = 200, body = AutomationRepoPublic)),
    security(("bearer_auth" = []))
)]
pub async fn update_repo(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateAutomationRepo>,
) -> AppResult<Json<AutomationRepoPublic>> {
    require_manage_linked_or_admin(&state, &auth, id).await?;
    let existing = repo_svc::load_repo_by_id(&state.pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound("automation repo".into()))?;

    let provider = if let Some(ref p) = body.provider {
        validate_provider(p)?
    } else {
        existing.provider.clone()
    };
    let name = body
        .name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(existing.name.as_str())
        .to_string();
    let remote_url = body
        .remote_url
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(existing.remote_url.as_str())
        .to_string();
    crate::services::egress::validate_public_https_url(&remote_url)?;
    let branch = body
        .branch
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(existing.branch.as_str())
        .to_string();
    let subpath = body
        .subpath
        .as_deref()
        .map(str::trim)
        .unwrap_or(existing.subpath.as_str())
        .to_string();

    let (ct, nonce) = match body.token.as_deref() {
        Some("") => (None, None),
        Some(tok) => {
            let (c, n) = state.secrets.encrypt(tok)?;
            (Some(c), Some(n))
        }
        None => (
            existing.token_ciphertext.clone(),
            existing.token_nonce.clone(),
        ),
    };

    let row = sqlx::query_as::<_, AutomationRepoRow>(&format!(
        r#"UPDATE automation_repos SET
             name = $2, provider = $3, remote_url = $4, branch = $5, subpath = $6,
             token_ciphertext = $7, token_nonce = $8, updated_at = NOW()
           WHERE id = $1
           RETURNING {SELECT_COLS}"#
    ))
    .bind(id)
    .bind(&name)
    .bind(&provider)
    .bind(&remote_url)
    .bind(&branch)
    .bind(&subpath)
    .bind(ct)
    .bind(nonce)
    .fetch_one(&state.pool)
    .await?;

    if let Some(project_ids) = body.project_ids {
        require_manage_on_projects(&state, &auth, &project_ids).await?;
        for pid in &project_ids {
            ensure_project(&state.pool, *pid).await?;
        }
        // Also need manage on currently linked projects being removed.
        let current = repo_svc::project_ids_for_repo(&state.pool, id).await?;
        require_manage_on_projects(&state, &auth, &current).await?;
        repo_svc::set_project_links(&state.pool, id, &project_ids, None).await?;
    }

    Ok(Json(to_public(&state.pool, &row).await?))
}

#[utoipa::path(
    delete,
    path = "/api/v1/automation-repos/{id}",
    tag = "automation-repo",
    params(("id" = Uuid, Path)),
    responses((status = 204, description = "Deleted")),
    security(("bearer_auth" = []))
)]
pub async fn delete_repo_by_id(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<crate::routes::tokens::StatusNoContent> {
    require_manage_linked_or_admin(&state, &auth, id).await?;
    let res = sqlx::query("DELETE FROM automation_repos WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound("automation repo".into()));
    }
    let dir = repo_svc::repo_workdir(id);
    let _ = std::fs::remove_dir_all(dir);
    Ok(crate::routes::tokens::StatusNoContent)
}

#[utoipa::path(
    put,
    path = "/api/v1/automation-repos/{id}/projects",
    tag = "automation-repo",
    params(("id" = Uuid, Path)),
    request_body = SetRepoProjects,
    responses((status = 200, body = AutomationRepoPublic)),
    security(("bearer_auth" = []))
)]
pub async fn set_repo_projects(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<SetRepoProjects>,
) -> AppResult<Json<AutomationRepoPublic>> {
    let row = repo_svc::load_repo_by_id(&state.pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound("automation repo".into()))?;
    let current = repo_svc::project_ids_for_repo(&state.pool, id).await?;
    require_manage_on_projects(&state, &auth, &current).await?;
    require_manage_on_projects(&state, &auth, &body.project_ids).await?;
    for pid in &body.project_ids {
        ensure_project(&state.pool, *pid).await?;
    }
    repo_svc::set_project_links(&state.pool, id, &body.project_ids, body.default_project_id)
        .await?;
    Ok(Json(to_public(&state.pool, &row).await?))
}

#[utoipa::path(
    post,
    path = "/api/v1/automation-repos/test",
    tag = "automation-repo",
    request_body = TestAutomationRepoBody,
    responses((status = 200, body = AutomationRepoTestResult)),
    security(("bearer_auth" = []))
)]
pub async fn test_repo_preview(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Json(body): Json<TestAutomationRepoBody>,
) -> AppResult<Json<AutomationRepoTestResult>> {
    require_manage_on_projects(&state, &auth, &body.project_ids).await?;
    for pid in &body.project_ids {
        ensure_project(&state.pool, *pid).await?;
    }
    let provider = validate_provider(&body.provider)?;
    if body.remote_url.trim().is_empty() {
        return Ok(Json(AutomationRepoTestResult {
            ok: false,
            message: "remote_url required".into(),
            commit_sha: None,
        }));
    }
    let branch = if body.branch.trim().is_empty() {
        "main".to_string()
    } else {
        body.branch.trim().to_string()
    };
    let (ok, message, commit_sha) = test_provider_connection(
        &provider,
        body.remote_url.trim(),
        &branch,
        body.token.as_deref(),
    )
    .await?;
    Ok(Json(AutomationRepoTestResult {
        ok,
        message,
        commit_sha,
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/automation-repos/{id}/test",
    tag = "automation-repo",
    params(("id" = Uuid, Path)),
    responses((status = 200, body = AutomationRepoTestResult)),
    security(("bearer_auth" = []))
)]
pub async fn test_repo_by_id(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<AutomationRepoTestResult>> {
    require_manage_linked_or_admin(&state, &auth, id).await?;
    let row = repo_svc::load_repo_by_id(&state.pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound("automation repo".into()))?;
    let token = decrypt_token(&state, &row)?;
    let (ok, message, commit_sha) = test_provider_connection(
        &row.provider,
        &row.remote_url,
        &row.branch,
        token.as_deref(),
    )
    .await?;
    Ok(Json(AutomationRepoTestResult {
        ok,
        message,
        commit_sha,
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/automation-repos/{id}/reindex",
    tag = "automation-repo",
    params(("id" = Uuid, Path)),
    responses((status = 200, body = AutomationRepoIndexResult)),
    security(("bearer_auth" = []))
)]
pub async fn reindex_repo_by_id(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<AutomationRepoIndexResult>> {
    require_manage_linked_or_admin(&state, &auth, id).await?;
    let row = repo_svc::load_repo_by_id(&state.pool, id)
        .await?
        .ok_or_else(|| AppError::NotFound("automation repo".into()))?;
    repo_svc::mark_indexing(&state.pool, row.id).await?;
    let token = decrypt_token(&state, &row)?;
    match sync_and_index(&state, &row, token.as_deref()).await {
        Ok((sha, files, chunks)) => Ok(Json(AutomationRepoIndexResult {
            status: "ready".into(),
            commit_sha: Some(sha),
            file_count: files,
            chunk_count: chunks,
            error: None,
        })),
        Err(e) => {
            let msg = e.to_string();
            let _ = repo_svc::mark_index_error(&state.pool, row.id, &msg).await;
            Ok(Json(AutomationRepoIndexResult {
                status: "error".into(),
                commit_sha: None,
                file_count: 0,
                chunk_count: 0,
                error: Some(msg),
            }))
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/automation-repos",
    tag = "automation-repo",
    params(("project_id" = Uuid, Path)),
    responses((status = 200, body = [AutomationRepoPublic])),
    security(("bearer_auth" = []))
)]
pub async fn list_project_repos(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
) -> AppResult<Json<Vec<AutomationRepoPublic>>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    ensure_project(&state.pool, project_id).await?;
    let rows = repo_svc::load_repos_for_project(&state.pool, project_id).await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in &rows {
        out.push(to_public(&state.pool, row).await?);
    }
    Ok(Json(out))
}

// ── Legacy singular shim ─────────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/automation-repo",
    tag = "automation-repo",
    params(("project_id" = Uuid, Path, description = "Project id")),
    responses((status = 200, body = AutomationRepoPublic), (status = 404, description = "Not configured")),
    security(("bearer_auth" = []))
)]
pub async fn get_repo(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
) -> AppResult<Json<AutomationRepoPublic>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    ensure_project(&state.pool, project_id).await?;
    let row = repo_svc::load_repo_for_project(&state.pool, project_id)
        .await?
        .ok_or_else(|| AppError::NotFound("automation repo".into()))?;
    Ok(Json(to_public(&state.pool, &row).await?))
}

#[utoipa::path(
    put,
    path = "/api/v1/projects/{project_id}/automation-repo",
    tag = "automation-repo",
    request_body = UpsertAutomationRepo,
    params(("project_id" = Uuid, Path, description = "Project id")),
    responses((status = 200, body = AutomationRepoPublic)),
    security(("bearer_auth" = []))
)]
pub async fn upsert_repo(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<UpsertAutomationRepo>,
) -> AppResult<Json<AutomationRepoPublic>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ManageAutomationRepo,
    )
    .await?;
    ensure_project(&state.pool, project_id).await?;

    let provider = validate_provider(&body.provider)?;
    if body.remote_url.trim().is_empty() {
        return Err(AppError::BadRequest("remote_url required".into()));
    }
    crate::services::egress::validate_public_https_url(body.remote_url.trim())?;
    let branch = if body.branch.trim().is_empty() {
        "main".to_string()
    } else {
        body.branch.trim().to_string()
    };
    let name = body
        .name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            body.remote_url
                .trim()
                .trim_end_matches('/')
                .rsplit('/')
                .next()
                .unwrap_or("Automation repo")
                .to_string()
        });

    let existing = repo_svc::load_repo_for_project(&state.pool, project_id).await?;
    let (ct, nonce) = match body.token.as_deref() {
        Some("") => (None, None),
        Some(tok) => {
            let (c, n) = state.secrets.encrypt(tok)?;
            (Some(c), Some(n))
        }
        None => match &existing {
            Some(e) => (e.token_ciphertext.clone(), e.token_nonce.clone()),
            None => (None, None),
        },
    };

    let row = if let Some(ex) = existing {
        sqlx::query_as::<_, AutomationRepoRow>(&format!(
            r#"UPDATE automation_repos SET
                 name = $2, provider = $3, remote_url = $4, branch = $5, subpath = $6,
                 token_ciphertext = $7, token_nonce = $8, project_id = $9, updated_at = NOW()
               WHERE id = $1
               RETURNING {SELECT_COLS}"#
        ))
        .bind(ex.id)
        .bind(&name)
        .bind(&provider)
        .bind(body.remote_url.trim())
        .bind(&branch)
        .bind(body.subpath.trim())
        .bind(ct)
        .bind(nonce)
        .bind(project_id)
        .fetch_one(&state.pool)
        .await?
    } else {
        let created = sqlx::query_as::<_, AutomationRepoRow>(&format!(
            r#"INSERT INTO automation_repos
                (name, project_id, provider, remote_url, branch, subpath, token_ciphertext, token_nonce)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
               RETURNING {SELECT_COLS}"#
        ))
        .bind(&name)
        .bind(project_id)
        .bind(&provider)
        .bind(body.remote_url.trim())
        .bind(&branch)
        .bind(body.subpath.trim())
        .bind(ct)
        .bind(nonce)
        .fetch_one(&state.pool)
        .await?;
        repo_svc::set_project_links(&state.pool, created.id, &[project_id], Some(project_id))
            .await?;
        created
    };

    // Ensure junction link exists for updated legacy rows.
    let links = repo_svc::project_ids_for_repo(&state.pool, row.id).await?;
    if !links.contains(&project_id) {
        let mut ids = links;
        ids.push(project_id);
        repo_svc::set_project_links(&state.pool, row.id, &ids, Some(project_id)).await?;
    }

    Ok(Json(to_public(&state.pool, &row).await?))
}

#[utoipa::path(
    delete,
    path = "/api/v1/projects/{project_id}/automation-repo",
    tag = "automation-repo",
    params(("project_id" = Uuid, Path, description = "Project id")),
    responses((status = 204, description = "Deleted")),
    security(("bearer_auth" = []))
)]
pub async fn delete_repo(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
) -> AppResult<crate::routes::tokens::StatusNoContent> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ManageAutomationRepo,
    )
    .await?;
    let row = repo_svc::load_repo_for_project(&state.pool, project_id).await?;
    if let Some(row) = row {
        // Unlink this project; delete the repo if it has no remaining links.
        let mut links = repo_svc::project_ids_for_repo(&state.pool, row.id).await?;
        links.retain(|p| *p != project_id);
        if links.is_empty() {
            sqlx::query("DELETE FROM automation_repos WHERE id = $1")
                .bind(row.id)
                .execute(&state.pool)
                .await?;
            let dir = repo_svc::repo_workdir(row.id);
            let _ = std::fs::remove_dir_all(dir);
        } else {
            repo_svc::set_project_links(&state.pool, row.id, &links, None).await?;
        }
    }
    Ok(crate::routes::tokens::StatusNoContent)
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/automation-repo/test",
    tag = "automation-repo",
    params(("project_id" = Uuid, Path, description = "Project id")),
    responses((status = 200, body = AutomationRepoTestResult)),
    security(("bearer_auth" = []))
)]
pub async fn test_repo(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
) -> AppResult<Json<AutomationRepoTestResult>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ManageAutomationRepo,
    )
    .await?;
    let row = repo_svc::load_repo_for_project(&state.pool, project_id)
        .await?
        .ok_or_else(|| {
            AppError::BadRequest(
                "No automation repo saved for this project yet. Save the connection first, or use \
                 POST /api/v1/automation-repos/test with the form values."
                    .into(),
            )
        })?;
    let token = decrypt_token(&state, &row)?;
    let (ok, message, commit_sha) = test_provider_connection(
        &row.provider,
        &row.remote_url,
        &row.branch,
        token.as_deref(),
    )
    .await?;
    Ok(Json(AutomationRepoTestResult {
        ok,
        message,
        commit_sha,
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/automation-repo/reindex",
    tag = "automation-repo",
    params(("project_id" = Uuid, Path, description = "Project id")),
    responses((status = 200, body = AutomationRepoIndexResult)),
    security(("bearer_auth" = []))
)]
pub async fn reindex_repo(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
) -> AppResult<Json<AutomationRepoIndexResult>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ManageAutomationRepo,
    )
    .await?;
    let row = repo_svc::load_repo_for_project(&state.pool, project_id)
        .await?
        .ok_or_else(|| AppError::NotFound("automation repo".into()))?;
    repo_svc::mark_indexing(&state.pool, row.id).await?;
    let token = decrypt_token(&state, &row)?;
    match sync_and_index(&state, &row, token.as_deref()).await {
        Ok((sha, files, chunks)) => Ok(Json(AutomationRepoIndexResult {
            status: "ready".into(),
            commit_sha: Some(sha),
            file_count: files,
            chunk_count: chunks,
            error: None,
        })),
        Err(e) => {
            let msg = e.to_string();
            let _ = repo_svc::mark_index_error(&state.pool, row.id, &msg).await;
            Ok(Json(AutomationRepoIndexResult {
                status: "error".into(),
                commit_sha: None,
                file_count: 0,
                chunk_count: 0,
                error: Some(msg),
            }))
        }
    }
}

async fn sync_and_index(
    state: &AppState,
    row: &AutomationRepoRow,
    token: Option<&str>,
) -> AppResult<(String, i32, i32)> {
    let provider = row.provider.to_lowercase();
    if matches!(provider.as_str(), "github" | "gitlab") {
        let known = repo_svc::load_content_hashes(&state.pool, row.id).await?;
        let index =
            remote_svc::fetch_tree(&provider, &row.remote_url, &row.branch, &row.subpath, token)
                .await?;
        let changed = remote_svc::fetch_changed_files(
            &provider,
            &row.remote_url,
            &row.branch,
            token,
            &index,
            &known,
        )
        .await?;
        let tree_paths: std::collections::HashMap<String, String> = index
            .entries
            .iter()
            .map(|e| (e.path.clone(), e.blob_sha.clone()))
            .collect();
        let (fc, cc) = repo_svc::persist_index_incremental(
            &state.pool,
            row.id,
            &tree_paths,
            &changed,
            &index.commit_sha,
        )
        .await?;
        return Ok((index.commit_sha, fc, cc));
    }

    if !repo_svc::git_available() {
        return Err(AppError::BadRequest(
            "git is not installed in this environment. The generic provider needs the git binary \
             in the API image; use the github or gitlab provider for REST indexing without git."
                .into(),
        ));
    }
    let repo_id = row.id;
    let remote_url = row.remote_url.clone();
    let branch = row.branch.clone();
    let subpath = row.subpath.clone();
    let token_owned = token.map(str::to_string);
    let sha = tokio::task::spawn_blocking(move || {
        repo_svc::sync_repo(repo_id, &remote_url, &branch, token_owned.as_deref())
    })
    .await
    .map_err(|e| AppError::Internal(format!("git sync task: {e}")))??;
    let files = tokio::task::spawn_blocking(move || repo_svc::index_workdir(repo_id, &subpath))
        .await
        .map_err(|e| AppError::Internal(format!("git index task: {e}")))??;
    let (fc, cc) = repo_svc::persist_index(&state.pool, row.id, &files, &sha).await?;
    Ok((sha, fc, cc))
}

async fn test_provider_connection(
    provider: &str,
    remote_url: &str,
    branch: &str,
    token: Option<&str>,
) -> AppResult<(bool, String, Option<String>)> {
    let provider = provider.trim().to_lowercase();
    if matches!(provider.as_str(), "github" | "gitlab") {
        return remote_svc::test_remote(&provider, remote_url, branch, token).await;
    }
    if !repo_svc::git_available() {
        return Ok((
            false,
            "git is not installed in this environment. The generic provider needs the git binary \
             in the API image; use the github or gitlab provider for REST indexing without git."
                .into(),
            None,
        ));
    }
    let remote_url = remote_url.to_string();
    let branch = branch.to_string();
    let token_owned = token.map(str::to_string);
    tokio::task::spawn_blocking(move || {
        repo_svc::test_connection(&remote_url, &branch, token_owned.as_deref())
    })
    .await
    .map_err(|e| AppError::Internal(format!("git test task: {e}")))?
}

fn decrypt_token(state: &AppState, row: &AutomationRepoRow) -> AppResult<Option<String>> {
    match (&row.token_ciphertext, &row.token_nonce) {
        (Some(ct), Some(nonce)) => Ok(Some(state.secrets.decrypt(ct, nonce)?)),
        _ => Ok(None),
    }
}
