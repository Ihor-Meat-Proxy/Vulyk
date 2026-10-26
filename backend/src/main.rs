use std::net::SocketAddr;

use axum::http::Method;
use clap::{Parser, Subcommand};
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
use vulyk_api::seed;
use vulyk_api::setup_cli::{self, SetupArgs};
use vulyk_api::sso;

#[derive(Parser, Debug)]
#[command(name = "vulyk-api", about = "Vulyk test management API", version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run the HTTP API server (default)
    Serve,
    /// Complete first-run instance setup
    Setup(Box<SetupArgs>),
    /// Database migration helpers
    Migrate {
        #[command(subcommand)]
        action: MigrateAction,
    },
    /// Print the API binary version (semver from Cargo.toml)
    Version,
}

#[derive(Subcommand, Debug)]
enum MigrateAction {
    /// List applied and pending migrations
    Status,
    /// Apply pending migrations (advisory-locked)
    Run,
}

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

    let cli = Cli::parse();
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://vulyk:vulyk@localhost:5432/vulyk".to_string());

    match cli.command.unwrap_or(Commands::Serve) {
        Commands::Serve => run_serve(&database_url).await?,
        Commands::Setup(args) => {
            let pool = db::connect(&database_url).await?;
            db::prepare_schema(&pool).await?;
            setup_cli::run_setup(&pool, *args).await?;
        }
        Commands::Migrate { action } => match action {
            MigrateAction::Status => {
                let pool = db::connect(&database_url).await?;
                let status = db::migration_status(&pool).await?;
                print!("{}", db::format_migration_status(&status));
                if status.schema_newer_than_binary() {
                    std::process::exit(2);
                }
                if !status.pending.is_empty() {
                    std::process::exit(1);
                }
            }
            MigrateAction::Run => {
                let pool = db::connect(&database_url).await?;
                db::migrate(&pool).await?;
                let status = db::migration_status(&pool).await?;
                print!("{}", db::format_migration_status(&status));
            }
        },
        Commands::Version => {
            println!("{}", db::crate_version());
        }
    }
    Ok(())
}

async fn run_serve(database_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let bind = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());

    let pool = db::connect(database_url).await?;
    db::prepare_schema(&pool).await?;

    let secrets = SecretsKey::from_env();
    seed::run(&pool).await?;
    sso::sync_from_env(&pool, &secrets).await?;

    jobs::spawn_worker(pool.clone());

    let state = AppState {
        pool,
        secrets,
        auth: AuthConfig::from_env(),
    };
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers(Any);

    let app = app_router(state)
        .layer(TraceLayer::new_for_http())
        .layer(cors);

    let addr: SocketAddr = bind.parse()?;
    tracing::info!(
        version = db::crate_version(),
        "Vulyk API listening on http://{addr}"
    );
    tracing::info!("OpenAPI docs at http://{addr}/api/docs");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
