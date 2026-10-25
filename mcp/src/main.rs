//! Vulyk MCP server (W10) — stdio and streamable HTTP transports.

use anyhow::{anyhow, Result};
use rmcp::{
    transport::{
        stdio,
        streamable_http_server::{session::local::LocalSessionManager, StreamableHttpService},
    },
    ServiceExt,
};
use tracing_subscriber::EnvFilter;
use vulyk_mcp::client::VulykClient;
use vulyk_mcp::server::VulykMcp;

#[tokio::main]
async fn main() -> Result<()> {
    let transport = std::env::args()
        .nth(1)
        .or_else(|| std::env::var("VULYK_MCP_TRANSPORT").ok())
        .unwrap_or_else(|| "stdio".into());

    // stdio: logs must go to stderr so they don't corrupt the protocol stream.
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| "vulyk_mcp=info".into()),
        )
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    let client = VulykClient::from_env()?;
    tracing::info!("Vulyk MCP starting (transport={transport})");

    match transport.as_str() {
        "stdio" => run_stdio(client).await,
        "http" | "streamable-http" | "sse" => run_http(client).await,
        other => Err(anyhow!("unknown transport '{other}' (use stdio or http)")),
    }
}

async fn run_stdio(client: VulykClient) -> Result<()> {
    let server = VulykMcp::new(client);
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

async fn run_http(client: VulykClient) -> Result<()> {
    let bind = std::env::var("VULYK_MCP_BIND").unwrap_or_else(|_| "127.0.0.1:8090".into());
    let service = StreamableHttpService::new(
        move || Ok(VulykMcp::new(client.clone())),
        LocalSessionManager::default().into(),
        Default::default(),
    );
    let router = axum::Router::new().nest_service("/mcp", service);
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!("Vulyk MCP HTTP at http://{bind}/mcp");
    axum::serve(listener, router).await?;
    Ok(())
}
