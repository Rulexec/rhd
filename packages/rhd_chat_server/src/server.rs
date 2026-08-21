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
use crate::subscriptions::new_shared_subscription_manager;

/// Run the WebSocket server.
pub async fn run(config: Config) -> Result<(), ServerError> {
    // Initialize database
    let db = Arc::new(ChatDb::new(&config.db_path)?);
    info!("Database initialized at {}", config.db_path);

    // Create subscription manager
    let subscription_manager = new_shared_subscription_manager();
    info!("Subscription manager initialized");

    // Bind TCP listener
    let listener = TcpListener::bind(&config.socket_addr()).await?;
    info!("WebSocket server listening on ws://{}/", config.socket_addr());

    // Accept connections
    loop {
        let (stream, addr) = listener.accept().await?;
        info!("New connection from: {}", addr);

        let db = Arc::clone(&db);
        let subscription_manager = subscription_manager.clone();
        tokio::spawn(async move {
            match accept_async(stream).await {
                Ok(ws_stream) => {
                    let (write, read) = ws_stream.split();
                    if let Err(e) = handle_connection(read, write, db, subscription_manager).await {
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
}
