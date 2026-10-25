use std::net::SocketAddr;

use axum::http::Method;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use vulyk_api::auth::AuthConfig;
use vulyk_api::crypto::SecretsKey;
use vulyk_api::db;
use vulyk_api::jobs;
use vulyk_api::routes::{app_router, AppState};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "vulyk_api=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://vulyk:vulyk@localhost:5432/vulyk".to_string());
    let bind = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());

    let pool = db::connect(&database_url).await?;
    db::migrate(&pool).await?;

    jobs::spawn_worker(pool.clone());

    let state = AppState {
        pool,
        secrets: SecretsKey::from_env(),
        auth: AuthConfig::from_env(),
    };
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers(Any);

    let app = app_router(state)
        .layer(TraceLayer::new_for_http())
        .layer(cors);

    let addr: SocketAddr = bind.parse()?;
    tracing::info!("Vulyk API listening on http://{addr}");
    tracing::info!("OpenAPI docs at http://{addr}/api/docs");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
