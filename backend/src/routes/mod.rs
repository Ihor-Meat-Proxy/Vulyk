pub mod cases;
pub mod health;
pub mod projects;
pub mod suites;

use axum::Router;
use sqlx::PgPool;
use utoipa::OpenApi;
use utoipa_scalar::{Scalar, Servable};

use crate::openapi::ApiDoc;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
}

pub fn app_router(state: AppState) -> Router {
    let api = Router::new()
        .merge(health::router())
        .merge(projects::router())
        .merge(suites::router())
        .merge(cases::router());

    Router::new()
        .merge(api)
        .merge(Scalar::with_url("/api/docs", ApiDoc::openapi()))
        .with_state(state)
}
