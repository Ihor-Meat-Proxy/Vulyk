use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use uuid::Uuid;

use crate::auth::{require_permission, OptionalAuthUser, Permission};
use crate::error::{AppError, AppResult};
use crate::importers::{self, ImportContext, ImportInput};
use crate::models::{
    FieldMapping, ImportPreview, ImportPreviewRequest, ImportReport, ImportRunRequest, ImporterInfo,
};
use crate::routes::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/importers", get(list_importers))
        .route(
            "/api/v1/projects/{project_id}/imports:preview",
            axum::routing::post(preview_import),
        )
        .route(
            "/api/v1/projects/{project_id}/imports",
            axum::routing::post(run_import),
        )
}

#[utoipa::path(
    get,
    path = "/api/v1/importers",
    tag = "imports",
    responses((status = 200, body = [ImporterInfo]))
)]
pub async fn list_importers() -> AppResult<Json<Vec<ImporterInfo>>> {
    let list = importers::list()
        .into_iter()
        .map(|d| ImporterInfo {
            id: d.id,
            name: d.name,
            supported_formats: d.supported_formats,
        })
        .collect();
    Ok(Json(list))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/imports:preview",
    tag = "imports",
    params(("project_id" = Uuid, Path, description = "Project id")),
    request_body = ImportPreviewRequest,
    responses((status = 200, body = ImportPreview)),
    security(("bearer_auth" = []))
)]
pub async fn preview_import(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<ImportPreviewRequest>,
) -> AppResult<Json<ImportPreview>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ImportCases,
    )
    .await?;
    ensure_project(&state, project_id).await?;
    let importer = importers::get(&body.importer_id)?;
    if !importer.supported_formats().contains(&body.format.as_str()) {
        return Err(AppError::BadRequest(format!(
            "importer {} does not support format {}",
            body.importer_id, body.format
        )));
    }
    let mapping = if is_default_mapping(&body.field_mapping) {
        importers::default_mapping_for(&body.importer_id)
    } else {
        body.field_mapping
    };
    let preview = importer
        .preview(ImportInput {
            format: body.format,
            content: body.content,
            field_mapping: mapping,
            options: body.options,
        })
        .await?;
    Ok(Json(preview))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/imports",
    tag = "imports",
    params(("project_id" = Uuid, Path, description = "Project id")),
    request_body = ImportRunRequest,
    responses(
        (status = 200, description = "Import report or dry-run preview"),
    ),
    security(("bearer_auth" = []))
)]
pub async fn run_import(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<ImportRunRequest>,
) -> AppResult<Json<serde_json::Value>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::ImportCases,
    )
    .await?;
    ensure_project(&state, project_id).await?;
    let importer = importers::get(&body.importer_id)?;
    if !importer.supported_formats().contains(&body.format.as_str()) {
        return Err(AppError::BadRequest(format!(
            "importer {} does not support format {}",
            body.importer_id, body.format
        )));
    }
    let mapping = if is_default_mapping(&body.field_mapping) {
        importers::default_mapping_for(&body.importer_id)
    } else {
        body.field_mapping.clone()
    };
    let input = ImportInput {
        format: body.format.clone(),
        content: body.content.clone(),
        field_mapping: mapping,
        options: body.options.clone(),
    };

    if body.dry_run {
        let preview = importer.preview(input).await?;
        return Ok(Json(serde_json::to_value(preview).unwrap_or_default()));
    }

    let report: ImportReport = importer
        .run(
            ImportContext {
                pool: state.pool.clone(),
                project_id,
            },
            input,
        )
        .await?;
    Ok(Json(serde_json::to_value(report).unwrap_or_default()))
}

fn is_default_mapping(mapping: &FieldMapping) -> bool {
    mapping.title == "title"
        && mapping.key == "key"
        && mapping.description == "description"
        && mapping.precondition == "precondition"
}

async fn ensure_project(state: &AppState, project_id: Uuid) -> AppResult<()> {
    let ok: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM projects WHERE id = $1")
        .bind(project_id)
        .fetch_optional(&state.pool)
        .await?;
    if ok.is_none() {
        return Err(AppError::NotFound("project".into()));
    }
    Ok(())
}
