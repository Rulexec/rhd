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
use rhd_db::ChatDb;

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

    // Clear pending acks if requested
    if config.clear_pending_acks {
        let db_path = format!("{}/chats.db", config.db_path);
        let db = ChatDb::new(&db_path)?;
        db.clear_all_custom_events()?;
        info!("Cleared all pending custom event acknowledgments");
    }

    // Start server
    server::run(config).await?;

    Ok(())
}
