//! Vulyk MCP server skeleton.
//!
//! Phase 1 ships a stub that documents planned tools. Workstream W10 wires
//! the `rmcp` crate and implements tools against `VULYK_API_URL` + token.

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "vulyk_mcp=info".into()),
        )
        .init();

    let api_url = std::env::var("VULYK_API_URL").unwrap_or_else(|_| "http://localhost:8080".into());

    tracing::info!("Vulyk MCP skeleton");
    tracing::info!("API base: {api_url}");
    tracing::info!(
        "Planned tools: search_cases, get_case, create_case, update_case, list_runs, create_run, ingest_results, ask_tests"
    );
    tracing::info!("Set VULYK_API_TOKEN when connecting (W10). Exiting stub.");
}
