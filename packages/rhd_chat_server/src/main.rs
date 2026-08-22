//! RHD Chat Server — WebSocket server for chat storage and management.
//!
//! This server provides a WebSocket API for chat persistence, message editing,
//! and real-time subscriptions. It does not perform AI calls or MCP tool execution.

use clap::Parser;
use tracing::info;
use tracing_subscriber::EnvFilter;

use rhd_chat_server::config::Config;
use rhd_chat_server::error::ServerError;
use rhd_chat_server::server;

#[tokio::main]
async fn main() -> Result<(), ServerError> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive("rhd_chat_server=info".parse().unwrap()),
        )
        .init();

    // Parse CLI arguments
    let config = Config::parse();
    info!("Starting RHD Chat Server on {}:{}", config.host, config.port);
    info!("Database path: {}", config.db_path);

    // Start server
    server::run(config).await?;

    Ok(())
}
