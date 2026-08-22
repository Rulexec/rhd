//! WebSocket server implementation.

use std::sync::Arc;

use futures_util::StreamExt;
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use tracing::{error, info};

use rhd_db::ChatDb;

use crate::config::Config;
use crate::connection::handle_connection;
use crate::error::ServerError;
use crate::plugins::new_shared_plugin_registry;
use crate::subscriptions::new_shared_subscription_manager;

/// Start the WebSocket server and return the bound port.
/// This is a non-blocking version for testing.
pub async fn start(config: Config) -> Result<(u16, tokio::task::JoinHandle<()>), ServerError> {
    // Initialize database
    let db = Arc::new(ChatDb::new(&config.db_path)?);
    info!("Database initialized at {}", config.db_path);

    // Create subscription manager
    let subscription_manager = new_shared_subscription_manager();
    info!("Subscription manager initialized");

    // Create plugin registry
    let plugin_registry = new_shared_plugin_registry();
    info!("Plugin registry initialized");

    // Bind TCP listener
    let listener = TcpListener::bind(&config.socket_addr()).await?;
    let actual_port = listener.local_addr()?.port();
    info!("WebSocket server listening on ws://{}/ (port {})", config.socket_addr(), actual_port);

    // Spawn server task
    let handle = tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    info!("New connection from: {}", addr);

                    let db = Arc::clone(&db);
                    let subscription_manager = subscription_manager.clone();
                    let plugin_registry = plugin_registry.clone();
                    tokio::spawn(async move {
                        match accept_async(stream).await {
                            Ok(ws_stream) => {
                                let (write, read) = ws_stream.split();
                                if let Err(e) = handle_connection(read, write, db, subscription_manager, plugin_registry).await {
                                    error!("Connection error from {}: {}", addr, e);
                                }
                                info!("Connection closed: {}", addr);
                            }
                            Err(e) => {
                                error!("WebSocket handshake failed for {}: {}", addr, e);
                            }
                        }
                    });
                }
                Err(e) => {
                    error!("Accept error: {}", e);
                    break;
                }
            }
        }
    });

    Ok((actual_port, handle))
}

/// Run the WebSocket server (blocking version).
pub async fn run(config: Config) -> Result<(), ServerError> {
    let (_port, handle) = start(config).await?;
    handle.await.map_err(|e| ServerError::Internal(format!("Server task failed: {}", e)))?;
    Ok(())
}
