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
use crate::streams::StreamManager;
use crate::subscriptions::new_shared_subscription_manager;

/// Start the WebSocket server and return the bound port.
/// This is a non-blocking version for testing.
pub async fn start(config: Config) -> Result<(u16, tokio::task::JoinHandle<()>), ServerError> {
    // Construct full database path: <db_path>/chats.db
    // Special case: ":memory:" is used for in-memory databases in tests
    let db_path = if config.db_path == ":memory:" {
        config.db_path.clone()
    } else {
        let db_dir = std::path::Path::new(&config.db_path);
        
        // Ensure the directory exists
        if !db_dir.exists() {
            std::fs::create_dir_all(db_dir)?;
        }
        
        db_dir.join("chats.db").to_str().unwrap().to_string()
    };
    
    // Initialize database
    let db = Arc::new(ChatDb::new(&db_path)?);
    info!("Database initialized at {}", db_path);

    // Create subscription manager
    let subscription_manager = new_shared_subscription_manager();
    info!("Subscription manager initialized");

    // Create plugin registry
    let plugin_registry = new_shared_plugin_registry();
    info!("Plugin registry initialized");

    // Create stream manager
    let stream_manager = Arc::new(StreamManager::new());
    info!("Stream manager initialized");

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
                    let stream_manager = stream_manager.clone();
                    tokio::spawn(async move {
                        match accept_async(stream).await {
                            Ok(ws_stream) => {
                                let (write, read) = ws_stream.split();
                                if let Err(e) = handle_connection(read, write, db, subscription_manager, plugin_registry, stream_manager).await {
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
