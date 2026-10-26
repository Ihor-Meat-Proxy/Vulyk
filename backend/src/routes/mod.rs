pub mod admin;
pub mod attachments;
pub mod auth;
pub mod automation_repos;
pub mod cases;
pub mod chat_propose;
pub mod ci;
pub mod custom_fields;
pub mod health;
pub mod imports;
pub mod ingest;
pub mod instance_settings;
pub mod integrations;
pub mod jobs;
pub mod llm;
pub mod memberships;
pub mod plans;
pub mod projects;
pub mod reports;
pub mod requirements;
pub mod runs;
pub mod setup;
pub mod sso_admin;
pub mod suites;
pub mod tags;
pub mod tokens;
pub mod users;
pub mod webhooks;

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
    // Auth bootstrap/login/register/setup/SSO start+callback, health, and CI webhooks
    // stay open for probes / providers / first-run wizard.
    let protected = Router::new()
        .merge(projects::router())
        .merge(suites::router())
        .merge(cases::router())
        .merge(tags::router())
        .merge(custom_fields::router())
        .merge(plans::router())
        .merge(runs::router())
        .merge(reports::router())
        .merge(attachments::router())
        .merge(requirements::router())
        .merge(integrations::router())
        .merge(imports::router())
        .merge(ingest::router())
        .merge(jobs::router())
        .merge(ci::router())
        .merge(llm::router())
        .merge(chat_propose::router())
        .merge(automation_repos::router())
        .merge(tokens::router())
        .merge(users::router())
        .merge(memberships::router())
        .merge(admin::router())
        .merge(sso_admin::router())
        .merge(instance_settings::router())
        .layer(middleware::from_fn_with_state(
            state.clone(),
            bearer_middleware,
        ));

    Router::new()
        .merge(health::router())
        .merge(auth::router())
        .merge(setup::router())
        .merge(instance_settings::public_router())
        .merge(webhooks::router())
        .merge(protected)
        .merge(Scalar::with_url("/api/docs", ApiDoc::openapi()))
        .with_state(state)
}
