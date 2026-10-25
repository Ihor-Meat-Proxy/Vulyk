use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use uuid::Uuid;

use crate::auth::RequireSystemAdmin;
use crate::error::AppResult;
use crate::routes::AppState;
use crate::sso::{self, SsoProviderAdmin, UpdateSsoProviderRequest};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/admin/sso/providers", get(list_providers))
        .route(
            "/api/v1/admin/sso/providers/{id}",
            axum::routing::put(update_provider).patch(update_provider),
        )
}

#[utoipa::path(
    get,
    path = "/api/v1/admin/sso/providers",
    tag = "admin",
    responses((status = 200, description = "SSO providers", body = [SsoProviderAdmin])),
    security(("bearer_auth" = []))
)]
pub async fn list_providers(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
) -> AppResult<Json<Vec<SsoProviderAdmin>>> {
    let rows = sso::list_providers(&state.pool).await?;
    Ok(Json(rows.iter().map(SsoProviderAdmin::from).collect()))
}

#[utoipa::path(
    put,
    path = "/api/v1/admin/sso/providers/{id}",
    tag = "admin",
    params(("id" = Uuid, Path, description = "Provider id")),
    request_body = UpdateSsoProviderRequest,
    responses((status = 200, description = "Updated provider", body = SsoProviderAdmin)),
    security(("bearer_auth" = []))
)]
pub async fn update_provider(
    State(state): State<AppState>,
    RequireSystemAdmin(_admin): RequireSystemAdmin,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateSsoProviderRequest>,
) -> AppResult<Json<SsoProviderAdmin>> {
    let updated = sso::update_provider(&state.pool, &state.secrets, id, body).await?;
    Ok(Json(SsoProviderAdmin::from(&updated)))
}
