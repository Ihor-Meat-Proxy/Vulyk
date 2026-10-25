pub mod cases;
pub mod health;
pub mod llm;
pub mod projects;
pub mod runs;
pub mod suites;
pub mod tokens;

use axum::{middleware, Router};
use sqlx::PgPool;
use utoipa::OpenApi;
use utoipa_scalar::{Scalar, Servable};

use crate::auth::bearer_middleware;
use crate::crypto::SecretsKey;
use crate::openapi::ApiDoc;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub secrets: SecretsKey,
}

pub fn app_router(state: AppState) -> Router {
    // Health stays open for probes even when VULYK_REQUIRE_AUTH=1.
    let protected = Router::new()
        .merge(projects::router())
        .merge(suites::router())
        .merge(cases::router())
        .merge(runs::router())
        .merge(llm::router())
        .merge(tokens::router())
        .layer(middleware::from_fn_with_state(
            state.clone(),
            bearer_middleware,
        ));

    Router::new()
        .merge(health::router())
        .merge(protected)
        .merge(Scalar::with_url("/api/docs", ApiDoc::openapi()))
        .with_state(state)
}
