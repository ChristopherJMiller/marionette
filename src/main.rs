//! Marionette - Window manipulation MCP server for Linux
//!
//! This MCP server enables AI assistants to interact with windows on Linux desktops,
//! supporting both X11 (including XWayland) and native Wayland environments.

use marionette::server::MarionetteServer;
use rmcp::ServiceExt;
use rmcp::transport::io::stdio;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing (stderr to keep stdout clean for MCP protocol)
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".to_string().into()),
        )
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .init();

    tracing::info!("Starting Marionette MCP Server");

    // Create the server
    let server = MarionetteServer::new().await?;

    // Run with stdio transport
    let transport = stdio();

    tracing::info!("Marionette MCP Server ready, listening on stdio");

    let service = server.serve(transport).await?;

    // Wait for graceful shutdown
    service.waiting().await?;

    tracing::info!("Marionette MCP Server shutting down");
    Ok(())
}
