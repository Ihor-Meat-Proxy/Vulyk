//! Per-project automation Git repository (connect, test, re-index).

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
    UpsertAutomationRepo,
};
use crate::routes::cases::ensure_project;
use crate::routes::AppState;
use crate::services::automation_repo as repo_svc;

pub fn router() -> Router<AppState> {
    Router::new()
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

const SELECT_COLS: &str = r#"id, project_id, provider, remote_url, branch, subpath,
    token_ciphertext, token_nonce, index_status, last_indexed_at,
    last_index_error, commit_sha, file_count, chunk_count, created_at, updated_at"#;

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
    Ok(Json(row.to_public()))
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

    let provider = body.provider.trim().to_lowercase();
    if !matches!(provider.as_str(), "github" | "gitlab" | "generic") {
        return Err(AppError::BadRequest(
            "provider must be github, gitlab, or generic".into(),
        ));
    }
    if body.remote_url.trim().is_empty() {
        return Err(AppError::BadRequest("remote_url required".into()));
    }
    let branch = if body.branch.trim().is_empty() {
        "main".to_string()
    } else {
        body.branch.trim().to_string()
    };

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

    let row = sqlx::query_as::<_, AutomationRepoRow>(&format!(
        r#"INSERT INTO automation_repos
            (project_id, provider, remote_url, branch, subpath, token_ciphertext, token_nonce)
           VALUES ($1, $2, $3, $4, $5, $6, $7)
           ON CONFLICT (project_id) DO UPDATE SET
             provider = EXCLUDED.provider,
             remote_url = EXCLUDED.remote_url,
             branch = EXCLUDED.branch,
             subpath = EXCLUDED.subpath,
             token_ciphertext = EXCLUDED.token_ciphertext,
             token_nonce = EXCLUDED.token_nonce,
             updated_at = NOW()
           RETURNING {SELECT_COLS}"#
    ))
    .bind(project_id)
    .bind(&provider)
    .bind(body.remote_url.trim())
    .bind(&branch)
    .bind(body.subpath.trim())
    .bind(ct)
    .bind(nonce)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(row.to_public()))
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
    sqlx::query("DELETE FROM automation_repos WHERE project_id = $1")
        .bind(project_id)
        .execute(&state.pool)
        .await?;
    let dir = repo_svc::repo_workdir(project_id);
    let _ = std::fs::remove_dir_all(dir);
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
        .ok_or_else(|| AppError::NotFound("automation repo".into()))?;
    let token = decrypt_token(&state, &row)?;
    let (ok, message, commit_sha) =
        repo_svc::test_connection(&row.remote_url, &row.branch, token.as_deref())?;
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
    let sha = repo_svc::sync_repo(row.project_id, &row.remote_url, &row.branch, token)?;
    let files = repo_svc::index_workdir(row.project_id, &row.subpath)?;
    let (fc, cc) = repo_svc::persist_index(&state.pool, row.id, &files, &sha).await?;
    Ok((sha, fc, cc))
}

fn decrypt_token(state: &AppState, row: &AutomationRepoRow) -> AppResult<Option<String>> {
    match (&row.token_ciphertext, &row.token_nonce) {
        (Some(ct), Some(nonce)) => Ok(Some(state.secrets.decrypt(ct, nonce)?)),
        _ => Ok(None),
    }
}
