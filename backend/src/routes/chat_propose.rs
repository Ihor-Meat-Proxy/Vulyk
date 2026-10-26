//! Chat propose + confirm create endpoints.

use axum::{extract::Path, extract::State, routing::post, Json, Router};
use uuid::Uuid;

use crate::auth::{require_permission, OptionalAuthUser, Permission};
use crate::error::{AppError, AppResult};
use crate::models::{
    ChatConfirmRequest, ChatConfirmResponse, ChatProposeRequest, ChatProposeResponse, LlmConfigRow,
};
use crate::routes::cases::ensure_project;
use crate::routes::llm::resolve_provider;
use crate::routes::AppState;
use crate::services::chat_propose::{confirm_proposal, run_chat_propose, tool_schemas};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/projects/{project_id}/llm/chat", post(chat_propose))
        .route(
            "/api/v1/projects/{project_id}/llm/chat/confirm",
            post(chat_confirm),
        )
        .route(
            "/api/v1/llm/chat-tools",
            axum::routing::get(list_chat_tools),
        )
}

#[utoipa::path(
    get,
    path = "/api/v1/llm/chat-tools",
    tag = "llm",
    responses((status = 200, description = "Tool / JSON-schema catalog for chat propose")),
    security(("bearer_auth" = []))
)]
pub async fn list_chat_tools(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
) -> AppResult<Json<serde_json::Value>> {
    if state.auth.required && auth.is_none() {
        return Err(AppError::Unauthorized("authentication required".into()));
    }
    Ok(Json(tool_schemas()))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/llm/chat",
    tag = "llm",
    request_body = ChatProposeRequest,
    params(("project_id" = Uuid, Path, description = "Project id")),
    responses((status = 200, body = ChatProposeResponse)),
    security(("bearer_auth" = []))
)]
pub async fn chat_propose(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
    Json(body): Json<ChatProposeRequest>,
) -> AppResult<Json<ChatProposeResponse>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::LlmAssist,
    )
    .await?;
    ensure_project(&state.pool, project_id).await?;
    if body.message.trim().is_empty() {
        return Err(AppError::BadRequest("message required".into()));
    }

    let model = load_model_name(&state, project_id, body.config_id)
        .await
        .unwrap_or_else(|| "mock-1".into());
    let provider = resolve_provider(&state, project_id, body.config_id).await?;
    let resp = run_chat_propose(&state.pool, project_id, provider.as_ref(), &model, &body).await?;
    Ok(Json(resp))
}

#[utoipa::path(
    post,
    path = "/api/v1/projects/{project_id}/llm/chat/confirm",
    tag = "llm",
    request_body = ChatConfirmRequest,
    params(("project_id" = Uuid, Path, description = "Project id")),
    responses((status = 200, body = ChatConfirmResponse)),
    security(("bearer_auth" = []))
)]
pub async fn chat_confirm(
    State(state): State<AppState>,
    OptionalAuthUser(auth): OptionalAuthUser,
    Path(project_id): Path<Uuid>,
    Json(mut body): Json<ChatConfirmRequest>,
) -> AppResult<Json<ChatConfirmResponse>> {
    require_permission(
        &state.pool,
        &auth,
        state.auth.required,
        project_id,
        Permission::LlmAssist,
    )
    .await?;
    ensure_project(&state.pool, project_id).await?;

    if !body.proposal.cases.is_empty() {
        require_permission(
            &state.pool,
            &auth,
            state.auth.required,
            project_id,
            Permission::CreateEditCases,
        )
        .await?;
    }
    if !body.proposal.requirements.is_empty() {
        require_permission(
            &state.pool,
            &auth,
            state.auth.required,
            project_id,
            Permission::ManageRequirements,
        )
        .await?;
    }
    let new_folder = body
        .proposal
        .placement
        .new_folder_name
        .as_ref()
        .is_some_and(|s| !s.trim().is_empty());
    if new_folder && !body.proposal.cases.is_empty() {
        require_permission(
            &state.pool,
            &auth,
            state.auth.required,
            project_id,
            Permission::ManageSuites,
        )
        .await?;
    }
    if new_folder && !body.proposal.requirements.is_empty() {
        require_permission(
            &state.pool,
            &auth,
            state.auth.required,
            project_id,
            Permission::ManageRequirements,
        )
        .await?;
    }

    body.proposal.placement.project_id = project_id;
    let user_id = auth
        .as_ref()
        .map(|u| u.id())
        .filter(|id| *id != Uuid::nil());
    let resp = confirm_proposal(&state.pool, user_id, body).await?;
    Ok(Json(resp))
}

async fn load_model_name(
    state: &AppState,
    project_id: Uuid,
    config_id: Option<Uuid>,
) -> Option<String> {
    if let Some(id) = config_id {
        let row: Option<LlmConfigRow> = sqlx::query_as(
            r#"SELECT id, project_id, provider, name, base_url, model,
                      api_key_ciphertext, api_key_nonce, extra, is_default, created_at, updated_at
               FROM llm_configs
               WHERE id = $1 AND (project_id = $2 OR project_id IS NULL)"#,
        )
        .bind(id)
        .bind(project_id)
        .fetch_optional(&state.pool)
        .await
        .ok()?;
        return row.map(|r| r.model);
    }
    let row: Option<LlmConfigRow> = sqlx::query_as(
        r#"SELECT id, project_id, provider, name, base_url, model,
                  api_key_ciphertext, api_key_nonce, extra, is_default, created_at, updated_at
           FROM llm_configs
           WHERE (project_id = $1 OR project_id IS NULL)
           ORDER BY
             CASE
               WHEN project_id = $1 AND is_default THEN 0
               WHEN project_id = $1 THEN 1
               WHEN project_id IS NULL AND is_default THEN 2
               ELSE 3
             END,
             created_at ASC
           LIMIT 1"#,
    )
    .bind(project_id)
    .fetch_optional(&state.pool)
    .await
    .ok()?;
    row.map(|r| r.model)
}
