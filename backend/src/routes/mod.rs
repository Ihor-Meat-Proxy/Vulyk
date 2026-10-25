pub mod admin;
pub mod auth;
pub mod cases;
pub mod health;
pub mod imports;
pub mod integrations;
pub mod llm;
pub mod memberships;
pub mod projects;
pub mod requirements;
pub mod runs;
pub mod suites;
pub mod tokens;
pub mod users;

use axum::{middleware, Router};
use sqlx::PgPool;
use utoipa::OpenApi;
use utoipa_scalar::{Scalar, Servable};

use crate::auth::{bearer_middleware, AuthConfig};
use crate::crypto::SecretsKey;
use crate::openapi::ApiDoc;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub secrets: SecretsKey,
    pub auth: AuthConfig,
}

pub fn app_router(state: AppState) -> Router {
    // Auth bootstrap/login stay open; health stays open for probes.
    let protected = Router::new()
        .merge(projects::router())
        .merge(suites::router())
        .merge(cases::router())
        .merge(runs::router())
        .merge(requirements::router())
        .merge(integrations::router())
        .merge(imports::router())
        .merge(llm::router())
        .merge(tokens::router())
        .merge(users::router())
        .merge(memberships::router())
        .merge(admin::router())
        .layer(middleware::from_fn_with_state(
            state.clone(),
            bearer_middleware,
        ));

    Router::new()
        .merge(health::router())
        .merge(auth::router())
        .merge(protected)
        .merge(Scalar::with_url("/api/docs", ApiDoc::openapi()))
        .with_state(state)
}
