//! Project CI tokens + CI reporting run lifecycle (open -> start/results -> close).

use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::auth::{
    generate_ci_token, hash_secret, require_permission, OptionalAuthUser, Permission,
};
use crate::error::{AppError, AppResult};
use crate::ingest::parse_payload;
use crate::models::{
    BulkResultsRequest, CiRunResultsResponse, CloseCiRunRequest, CreateProjectCiToken,
    CreateProjectCiTokenResponse, IngestRequest, OpenCiRunRequest, OpenCiRunResponse,
    ProjectCiTokenPublic, TestRun, TEST_RUN_COLS,
};
use crate::routes::cases::ensure_project;
use crate::routes::AppState;
use crate::services::ingest as ingest_svc;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/projects/{project_id}/ci-tokens",
            get(list_ci_tokens).post(create_ci_token),
        )
        .route(
            "/api/v1/projects/{project_id}/ci-tokens/{token_id}",
            axum::routing::delete(revoke_ci_token),
        )
        .route(
            "/api/v1/projects/{project_id}/ci/runs",
            axum::routing::post(open_ci_run),
        )
        .route(
            "/api/v1/projects/{project_id}/ci/runs/{run_id}",
            get(get_ci_run),
        )
        .route(
            "/api/v1/projects/{project_id}/ci/runs/{run_id}/start",
            axum::routing::post(start_ci_run),
        )
        .route(
            "/api/v1/projects/{project_id}/ci/runs/{run_id}/results",
            axum::routing::post(post_ci_results),
        )
        .route(
            "/api/v1/projects/{project_id}/ci/runs/{run_id}/ingest",
            axum::routing::post(ingest_ci_run),
        )
        .route(
            "/api/v1/projects/{project_id}/ci/runs/{run_id}/close",
            axum::routing::post(close_ci_run),
        )
}

// ── CI tokens ────────────────────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/ci-tokens",
    tag = "ci-report",
    params(("project_id" = Uuid, Path)),
    responses((status = 200, body = [ProjectCiTokenPublic])),
    security(("bearer_auth" = []))
)]
pub async fn list_ci_tokens(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
) -> AppResult<Json<Vec<ProjectCiTokenPublic>>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ManageApiTokens,
    )
    .await?;
    ensure_project(&state.pool, project_id).await?;
    let rows = sqlx::query_as::<_, ProjectCiTokenPublic>(
        r#"SELECT id, project_id, name, token_prefix, created_at, last_used_at, revoked_at
           FROM project_ci_tokens
           WHERE project_id = $1
           ORDER BY created_at DESC"#,
    )
    .bind(project_id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/ci-tokens",
    tag = "ci-report",
    params(("project_id" = Uuid, Path)),
    request_body = CreateProjectCiToken,
    responses((status = 200, body = CreateProjectCiTokenResponse)),
    security(("bearer_auth" = []))
)]
pub async fn create_ci_token(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<CreateProjectCiToken>,
) -> AppResult<Json<CreateProjectCiTokenResponse>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ManageApiTokens,
    )
    .await?;
    ensure_project(&state.pool, project_id).await?;
    if body.name.trim().is_empty() {
        return Err(AppError::BadRequest("name required".into()));
    }
    let (secret, prefix) = generate_ci_token();
    let hash = hash_secret(&secret);
    let created_by = auth
        .as_ref()
        .filter(|u| u.id() != Uuid::nil())
        .map(|u| u.id());

    let row = sqlx::query_as::<_, ProjectCiTokenPublic>(
        r#"INSERT INTO project_ci_tokens (project_id, name, token_prefix, token_hash, created_by)
           VALUES ($1, $2, $3, $4, $5)
           RETURNING id, project_id, name, token_prefix, created_at, last_used_at, revoked_at"#,
    )
    .bind(project_id)
    .bind(body.name.trim())
    .bind(&prefix)
    .bind(&hash)
    .bind(created_by)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(CreateProjectCiTokenResponse { token: row, secret }))
}

#[utoipa::path(
    delete,
    path = "/api/v1/projects/{project_id}/ci-tokens/{token_id}",
    tag = "ci-report",
    params(("project_id" = Uuid, Path), ("token_id" = Uuid, Path)),
    responses((status = 204, description = "Revoked")),
    security(("bearer_auth" = []))
)]
pub async fn revoke_ci_token(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, token_id)): Path<(Uuid, Uuid)>,
) -> AppResult<crate::routes::tokens::StatusNoContent> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ManageApiTokens,
    )
    .await?;
    let res = sqlx::query(
        r#"UPDATE project_ci_tokens SET revoked_at = NOW()
           WHERE id = $1 AND project_id = $2 AND revoked_at IS NULL"#,
    )
    .bind(token_id)
    .bind(project_id)
    .execute(&state.pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound("ci token".into()));
    }
    Ok(crate::routes::tokens::StatusNoContent)
}

// ── CI runs ──────────────────────────────────────────────────────────────────

fn normalize_ci_status(status: &str) -> &str {
    match status {
        "draft" | "queued" => "open",
        "running" => "in_progress",
        "completed" | "finished" => "closed",
        other => other,
    }
}

async fn load_ci_run(pool: &sqlx::PgPool, project_id: Uuid, run_id: Uuid) -> AppResult<TestRun> {
    let run = sqlx::query_as::<_, TestRun>(&format!(
        r#"SELECT {TEST_RUN_COLS} FROM test_runs WHERE id = $1 AND project_id = $2"#
    ))
    .bind(run_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("run".into()))?;
    Ok(run)
}

fn assert_ci_writable(run: &TestRun) -> AppResult<()> {
    let status = normalize_ci_status(&run.status);
    if status == "closed" {
        return Err(AppError::Conflict(
            "run is closed; further results are rejected".into(),
        ));
    }
    Ok(())
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/ci/runs",
    tag = "ci-report",
    params(("project_id" = Uuid, Path)),
    request_body = OpenCiRunRequest,
    responses((status = 200, body = OpenCiRunResponse)),
    security(("bearer_auth" = []))
)]
pub async fn open_ci_run(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<OpenCiRunRequest>,
) -> AppResult<Json<OpenCiRunResponse>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::UploadResults,
    )
    .await?;
    ensure_project(&state.pool, project_id).await?;

    let external_ref = body
        .external_ref
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    if let Some(ref ext) = external_ref {
        let existing = sqlx::query_as::<_, TestRun>(&format!(
            r#"SELECT {TEST_RUN_COLS} FROM test_runs
               WHERE project_id = $1 AND external_ref = $2
               LIMIT 1"#
        ))
        .bind(project_id)
        .bind(ext)
        .fetch_optional(&state.pool)
        .await?;
        if let Some(run) = existing {
            return Ok(Json(OpenCiRunResponse {
                run,
                idempotent: true,
            }));
        }
    }

    let provider = body
        .provider
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_lowercase());
    let source = match provider.as_deref() {
        Some("github") => "ci:github".to_string(),
        Some("gitlab") => "ci:gitlab".to_string(),
        Some(other) => format!("ci:{other}"),
        None => "ci".to_string(),
    };
    let status = if body.start { "in_progress" } else { "open" };
    let suite_name = body
        .suite_name
        .as_deref()
        .or(body.name.as_deref())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let name = suite_name.clone().unwrap_or_else(|| "CI run".to_string());
    let environment = body.environment.unwrap_or_else(|| json!({}));
    let artifacts = body.artifacts.unwrap_or_else(|| json!([]));
    let meta = body.meta.unwrap_or_else(|| json!({}));
    let started_at: Option<chrono::DateTime<chrono::Utc>> = if body.start {
        Some(chrono::Utc::now())
    } else {
        None
    };

    let mut iteration_id = body.iteration_id;
    if let Some(iid) = iteration_id {
        let exists: Option<(Uuid,)> =
            sqlx::query_as(r#"SELECT id FROM iterations WHERE id = $1 AND project_id = $2"#)
                .bind(iid)
                .bind(project_id)
                .fetch_optional(&state.pool)
                .await?;
        if exists.is_none() {
            return Err(AppError::BadRequest(
                "iteration_id does not belong to this project".into(),
            ));
        }
    } else if body.campaign_id.is_some() || body.campaign.is_some() || body.iteration.is_some() {
        let campaign = if let Some(cid) = body.campaign_id {
            crate::services::campaigns::get_campaign(&state.pool, project_id, cid).await?
        } else {
            let campaign_name = body
                .campaign
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("CI");
            crate::services::campaigns::find_or_create_campaign(
                &state.pool,
                project_id,
                campaign_name,
                None,
            )
            .await?
        };
        let iteration_name = body
            .iteration
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("Pipeline");
        let iteration = crate::services::campaigns::find_or_create_iteration(
            &state.pool,
            project_id,
            campaign.id,
            iteration_name,
            "",
            body.cadence.as_deref().unwrap_or("on-push"),
        )
        .await?;
        iteration_id = Some(iteration.id);
    }

    let run = match sqlx::query_as::<_, TestRun>(&format!(
        r#"INSERT INTO test_runs
             (project_id, iteration_id, name, description, status, source, external_ref,
              meta, created_by_name, environment, artifacts, started_at,
              suite_name, ci_provider, ci_repo, ci_branch, ci_commit,
              pipeline_url, job_url, lifecycle)
           VALUES ($1, $2, $3, '', $4, $5, $6, $7, $8, $9, $10, $11,
                   $12, $13, $14, $15, $16, $17, $18, 'ci')
           RETURNING {TEST_RUN_COLS}"#
    ))
    .bind(project_id)
    .bind(iteration_id)
    .bind(&name)
    .bind(status)
    .bind(&source)
    .bind(&external_ref)
    .bind(&meta)
    .bind(&body.created_by_name)
    .bind(&environment)
    .bind(&artifacts)
    .bind(started_at)
    .bind(&suite_name)
    .bind(&provider)
    .bind(&body.repo)
    .bind(&body.branch)
    .bind(&body.commit)
    .bind(&body.pipeline_url)
    .bind(&body.job_url)
    .fetch_one(&state.pool)
    .await
    {
        Ok(run) => run,
        Err(sqlx::Error::Database(err))
            if err.constraint() == Some("test_runs_project_external_ref_uidx") =>
        {
            // Race: another open won the unique index — return existing.
            if let Some(ref ext) = external_ref {
                let existing = sqlx::query_as::<_, TestRun>(&format!(
                    r#"SELECT {TEST_RUN_COLS} FROM test_runs
                       WHERE project_id = $1 AND external_ref = $2
                       LIMIT 1"#
                ))
                .bind(project_id)
                .bind(ext)
                .fetch_optional(&state.pool)
                .await?
                .ok_or_else(|| AppError::Conflict("external_ref conflict".into()))?;
                return Ok(Json(OpenCiRunResponse {
                    run: existing,
                    idempotent: true,
                }));
            }
            return Err(AppError::Conflict("external_ref conflict".into()));
        }
        Err(e) => return Err(e.into()),
    };

    Ok(Json(OpenCiRunResponse {
        run,
        idempotent: false,
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/ci/runs/{run_id}",
    tag = "ci-report",
    params(("project_id" = Uuid, Path), ("run_id" = Uuid, Path)),
    responses((status = 200, body = TestRun)),
    security(("bearer_auth" = []))
)]
pub async fn get_ci_run(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, run_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<TestRun>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ViewProject,
    )
    .await?;
    let run = load_ci_run(&state.pool, project_id, run_id).await?;
    Ok(Json(run))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/ci/runs/{run_id}/start",
    tag = "ci-report",
    params(("project_id" = Uuid, Path), ("run_id" = Uuid, Path)),
    responses((status = 200, body = TestRun)),
    security(("bearer_auth" = []))
)]
pub async fn start_ci_run(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, run_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<TestRun>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::UploadResults,
    )
    .await?;
    let run = load_ci_run(&state.pool, project_id, run_id).await?;
    assert_ci_writable(&run)?;
    let updated = sqlx::query_as::<_, TestRun>(&format!(
        r#"UPDATE test_runs SET
             status = 'in_progress',
             started_at = COALESCE(started_at, NOW()),
             updated_at = NOW()
           WHERE id = $1
           RETURNING {TEST_RUN_COLS}"#
    ))
    .bind(run_id)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(updated))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/ci/runs/{run_id}/results",
    tag = "ci-report",
    params(("project_id" = Uuid, Path), ("run_id" = Uuid, Path)),
    request_body = BulkResultsRequest,
    responses((status = 200, body = CiRunResultsResponse)),
    security(("bearer_auth" = []))
)]
pub async fn post_ci_results(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, run_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<BulkResultsRequest>,
) -> AppResult<Json<CiRunResultsResponse>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::UploadResults,
    )
    .await?;
    if body.results.is_empty() {
        return Err(AppError::BadRequest("results must not be empty".into()));
    }
    let run = load_ci_run(&state.pool, project_id, run_id).await?;
    assert_ci_writable(&run)?;

    let report = ingest_svc::persist_results(
        &state.pool,
        run_id,
        project_id,
        &body.results,
        "ci",
        body.upsert,
    )
    .await?;
    let updated = load_ci_run(&state.pool, project_id, run_id).await?;
    Ok(Json(CiRunResultsResponse {
        run_id,
        status: updated.status,
        inserted: report.inserted,
        updated: report.updated,
        unmatched_case_keys: report.unmatched_case_keys,
        idempotent_replays: report.idempotent_replays,
        job_id: report.job_id,
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/ci/runs/{run_id}/ingest",
    tag = "ci-report",
    params(("project_id" = Uuid, Path), ("run_id" = Uuid, Path)),
    request_body = IngestRequest,
    responses((status = 200, body = CiRunResultsResponse)),
    security(("bearer_auth" = []))
)]
pub async fn ingest_ci_run(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, run_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<IngestRequest>,
) -> AppResult<Json<CiRunResultsResponse>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::UploadResults,
    )
    .await?;
    let run = load_ci_run(&state.pool, project_id, run_id).await?;
    assert_ci_writable(&run)?;

    let format = body.format.trim().to_lowercase();
    if format.is_empty() {
        return Err(AppError::BadRequest("format is required".into()));
    }
    if body.content.trim().is_empty() {
        return Err(AppError::BadRequest("content is required".into()));
    }
    let parsed = parse_payload(&format, &body.content).map_err(AppError::BadRequest)?;
    let source = if body.source.trim().is_empty() {
        format
    } else {
        body.source
    };

    if body.created_by_name.is_some() || body.environment.is_some() || body.artifacts.is_some() {
        sqlx::query(
            r#"UPDATE test_runs SET
                 created_by_name = COALESCE($2, created_by_name),
                 environment = COALESCE($3, environment),
                 artifacts = COALESCE($4, artifacts),
                 updated_at = NOW()
               WHERE id = $1"#,
        )
        .bind(run_id)
        .bind(&body.created_by_name)
        .bind(&body.environment)
        .bind(&body.artifacts)
        .execute(&state.pool)
        .await?;
    }

    let report = ingest_svc::persist_results(
        &state.pool,
        run_id,
        project_id,
        &parsed.results,
        &source,
        true,
    )
    .await?;
    let updated = load_ci_run(&state.pool, project_id, run_id).await?;
    Ok(Json(CiRunResultsResponse {
        run_id,
        status: updated.status,
        inserted: report.inserted,
        updated: report.updated,
        unmatched_case_keys: report.unmatched_case_keys,
        idempotent_replays: report.idempotent_replays,
        job_id: report.job_id,
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/ci/runs/{run_id}/close",
    tag = "ci-report",
    params(("project_id" = Uuid, Path), ("run_id" = Uuid, Path)),
    request_body = CloseCiRunRequest,
    responses((status = 200, body = TestRun)),
    security(("bearer_auth" = []))
)]
pub async fn close_ci_run(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path((project_id, run_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<CloseCiRunRequest>,
) -> AppResult<Json<TestRun>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::UploadResults,
    )
    .await?;
    let run = load_ci_run(&state.pool, project_id, run_id).await?;
    // Closing an already-closed run is idempotent.
    if normalize_ci_status(&run.status) == "closed" {
        return Ok(Json(run));
    }

    let mut meta = run.meta.clone();
    if body.cancelled {
        if let Some(obj) = meta.as_object_mut() {
            obj.insert("cancelled".into(), Value::Bool(true));
            if let Some(msg) = body.message {
                obj.insert("cancel_message".into(), Value::String(msg));
            }
        } else {
            meta = json!({
                "cancelled": true,
                "cancel_message": body.message,
            });
        }
    } else if let Some(msg) = body.message {
        if let Some(obj) = meta.as_object_mut() {
            obj.insert("close_message".into(), Value::String(msg));
        }
    }

    let updated = sqlx::query_as::<_, TestRun>(&format!(
        r#"UPDATE test_runs SET
             status = 'closed',
             finished_at = COALESCE(finished_at, NOW()),
             meta = $2,
             updated_at = NOW()
           WHERE id = $1
           RETURNING {TEST_RUN_COLS}"#
    ))
    .bind(run_id)
    .bind(&meta)
    .fetch_one(&state.pool)
    .await?;
    let _ = crate::services::campaigns::refresh_run_counters(&state.pool, run_id).await;
    Ok(Json(updated))
}
