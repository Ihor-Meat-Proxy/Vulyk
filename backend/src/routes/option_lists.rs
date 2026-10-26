use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use uuid::Uuid;

use crate::auth::RequireSystemAdmin;
use crate::error::AppResult;
use crate::models::{
    CreateOptionList, CreateOptionListItem, OptionList, OptionListDetail, OptionListItem,
    ReorderOptionListItems, UpdateOptionList, UpdateOptionListItem,
};
use crate::routes::AppState;
use crate::services::option_lists;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/admin/lists", get(list_lists).post(create_list))
        .route(
            "/api/v1/admin/lists/{id}",
            get(get_list).put(update_list).delete(delete_list),
        )
        .route(
            "/api/v1/admin/lists/{id}/items",
            axum::routing::post(create_item),
        )
        .route(
            "/api/v1/admin/lists/{id}/items/reorder",
            axum::routing::put(reorder_items),
        )
        .route(
            "/api/v1/admin/lists/{id}/items/{item_id}",
            axum::routing::put(update_item).delete(delete_item),
        )
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/lists",
    tag = "option_lists",
    responses((status = 200, body = [OptionList])),
    security(("bearer_auth" = []))
)]
pub async fn list_lists(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
) -> AppResult<Json<Vec<OptionList>>> {
    Ok(Json(option_lists::list_all(&state.pool).await?))
}

#[utoipa::path(
    post,
    path = "/api/v1/admin/lists",
    tag = "option_lists",
    request_body = CreateOptionList,
    responses((status = 200, body = OptionListDetail)),
    security(("bearer_auth" = []))
)]
pub async fn create_list(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
    Json(body): Json<CreateOptionList>,
) -> AppResult<Json<OptionListDetail>> {
    Ok(Json(option_lists::create(&state.pool, body).await?))
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/lists/{id}",
    tag = "option_lists",
    params(("id" = Uuid, Path)),
    responses((status = 200, body = OptionListDetail)),
    security(("bearer_auth" = []))
)]
pub async fn get_list(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
    Path(id): Path<Uuid>,
) -> AppResult<Json<OptionListDetail>> {
    Ok(Json(option_lists::get_detail(&state.pool, id).await?))
}

#[utoipa::path(
    put,
    path = "/api/v1/admin/lists/{id}",
    tag = "option_lists",
    params(("id" = Uuid, Path)),
    request_body = UpdateOptionList,
    responses((status = 200, body = OptionList)),
    security(("bearer_auth" = []))
)]
pub async fn update_list(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateOptionList>,
) -> AppResult<Json<OptionList>> {
    Ok(Json(option_lists::update(&state.pool, id, body).await?))
}

#[utoipa::path(
    delete,
    path = "/api/v1/admin/lists/{id}",
    tag = "option_lists",
    params(("id" = Uuid, Path)),
    responses((status = 200, description = "deleted or deactivated")),
    security(("bearer_auth" = []))
)]
pub async fn delete_list(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let action = option_lists::delete_or_deactivate(&state.pool, id).await?;
    Ok(Json(serde_json::json!({ "action": action })))
}

#[utoipa::path(
    post,
    path = "/api/v1/admin/lists/{id}/items",
    tag = "option_lists",
    params(("id" = Uuid, Path)),
    request_body = CreateOptionListItem,
    responses((status = 200, body = OptionListItem)),
    security(("bearer_auth" = []))
)]
pub async fn create_item(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
    Path(id): Path<Uuid>,
    Json(body): Json<CreateOptionListItem>,
) -> AppResult<Json<OptionListItem>> {
    Ok(Json(option_lists::add_item(&state.pool, id, body).await?))
}

#[utoipa::path(
    put,
    path = "/api/v1/admin/lists/{id}/items/{item_id}",
    tag = "option_lists",
    params(("id" = Uuid, Path), ("item_id" = Uuid, Path)),
    request_body = UpdateOptionListItem,
    responses((status = 200, body = OptionListItem)),
    security(("bearer_auth" = []))
)]
pub async fn update_item(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
    Path((id, item_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateOptionListItem>,
) -> AppResult<Json<OptionListItem>> {
    Ok(Json(
        option_lists::update_item(&state.pool, id, item_id, body).await?,
    ))
}

#[utoipa::path(
    delete,
    path = "/api/v1/admin/lists/{id}/items/{item_id}",
    tag = "option_lists",
    params(("id" = Uuid, Path), ("item_id" = Uuid, Path)),
    responses((status = 200, description = "deleted or deprecated")),
    security(("bearer_auth" = []))
)]
pub async fn delete_item(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
    Path((id, item_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<serde_json::Value>> {
    let action = option_lists::delete_or_deprecate_item(&state.pool, id, item_id).await?;
    Ok(Json(serde_json::json!({ "action": action })))
}

#[utoipa::path(
    put,
    path = "/api/v1/admin/lists/{id}/items/reorder",
    tag = "option_lists",
    params(("id" = Uuid, Path)),
    request_body = ReorderOptionListItems,
    responses((status = 200, body = [OptionListItem])),
    security(("bearer_auth" = []))
)]
pub async fn reorder_items(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
    Path(id): Path<Uuid>,
    Json(body): Json<ReorderOptionListItems>,
) -> AppResult<Json<Vec<OptionListItem>>> {
    Ok(Json(
        option_lists::reorder_items(&state.pool, id, &body.item_ids).await?,
    ))
}
