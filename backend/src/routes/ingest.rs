use axum::{
    extract::{Path, State},
    routing::post,
    Json, Router,
};
use serde_json::json;
use uuid::Uuid;

use crate::auth::{require_permission, OptionalAuthUser, Permission};
use crate::error::{AppError, AppResult};
use crate::ingest::parse_payload;
use crate::models::{BulkResultsRequest, BulkResultsResponse, IngestRequest, IngestResponse};
use crate::routes::AppState;
use crate::services::{ingest as ingest_svc, jobs as jobs_svc};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/runs/{id}/results:bulk", post(bulk_results))
        .route("/api/v1/runs/{id}/ingest", post(ingest))
}

#[utoipa::path(
    post,
    path = "/api/v1/runs/{id}/results:bulk",
    tag = "ingest",
    params(("id" = Uuid, Path, description = "Run id")),
    request_body = BulkResultsRequest,
    responses((status = 200, description = "Bulk insert summary", body = BulkResultsResponse)),
    security(("bearer_auth" = []))
)]
pub async fn bulk_results(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<BulkResultsRequest>,
) -> AppResult<Json<BulkResultsResponse>> {
    if body.results.is_empty() {
        return Err(AppError::BadRequest("results must not be empty".into()));
    }
    let project_id = ingest_svc::ensure_run_exists(&state.pool, id).await?;
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::UploadResults,
    )
    .await?;

    if body.results.len() >= ingest_svc::ASYNC_THRESHOLD {
        let job = jobs_svc::enqueue(
            &state.pool,
            "ingest",
            json!({
                "run_id": id,
                "project_id": project_id,
                "format": "vulyk_json",
                "content": serde_json::to_string(&json!({
                    "format": "vulyk/results@v1",
                    "results": body.results,
                })).unwrap_or_default(),
                "source": "bulk",
                "upsert": body.upsert,
            }),
            5,
        )
        .await?;
        return Ok(Json(BulkResultsResponse {
            inserted: 0,
            updated: 0,
            unmatched_case_keys: vec![],
            job_id: Some(job.id),
        }));
    }

    let report = ingest_svc::persist_results(
        &state.pool,
        id,
        project_id,
        &body.results,
        "bulk",
        body.upsert,
    )
    .await?;
    Ok(Json(report))
}

#[utoipa::path(
    post,
    path = "/api/v1/runs/{id}/ingest",
    tag = "ingest",
    params(("id" = Uuid, Path, description = "Run id")),
    request_body = IngestRequest,
    responses((status = 200, description = "Ingest summary", body = IngestResponse)),
    security(("bearer_auth" = []))
)]
pub async fn ingest(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<IngestRequest>,
) -> AppResult<Json<IngestResponse>> {
    let project_id = ingest_svc::ensure_run_exists(&state.pool, id).await?;
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::UploadResults,
    )
    .await?;
    let format = body.format.trim().to_lowercase();
    if format.is_empty() {
        return Err(AppError::BadRequest("format is required".into()));
    }
    if body.content.trim().is_empty() {
        return Err(AppError::BadRequest("content is required".into()));
    }

    // Validate parse early even for async path so clients get fast feedback.
    let parsed = parse_payload(&format, &body.content).map_err(AppError::BadRequest)?;
    let source = if body.source.trim().is_empty() {
        format.clone()
    } else {
        body.source
    };

    let async_mode = body.async_mode || parsed.results.len() >= ingest_svc::ASYNC_THRESHOLD;

    if async_mode {
        let kind = if format.starts_with("junit") {
            "ingest_junit"
        } else {
            "ingest_json"
        };
        let job = jobs_svc::enqueue(
            &state.pool,
            kind,
            json!({
                "run_id": id,
                "project_id": project_id,
                "format": format,
                "content": body.content,
                "source": source,
                "upsert": true,
            }),
            5,
        )
        .await?;
        return Ok(Json(IngestResponse {
            inserted: 0,
            updated: 0,
            unmatched_case_keys: vec![],
            job_id: Some(job.id),
            parsed: parsed.results.len() as i64,
        }));
    }

    let report =
        ingest_svc::persist_results(&state.pool, id, project_id, &parsed.results, &source, true)
            .await?;

    Ok(Json(IngestResponse {
        inserted: report.inserted,
        updated: report.updated,
        unmatched_case_keys: report.unmatched_case_keys,
        job_id: None,
        parsed: parsed.results.len() as i64,
    }))
}
