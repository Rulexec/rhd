//! Per-connection state and message handling.

use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, error};

use rhd_chat_api::protocol::Request;
use rhd_chat_api::ErrorResponse;
use rhd_db::ChatDb;

use crate::error::ServerError;
use crate::handlers;
use crate::plugins::{self, SharedPluginRegistry};
use crate::subscriptions::SharedSubscriptionManager;

/// Type alias for WebSocket read half.
type WsRead = futures_util::stream::SplitStream<tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>>;

/// Type alias for WebSocket write half.
type WsWrite = futures_util::stream::SplitSink<tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>, Message>;

/// Handle a single WebSocket connection.
pub async fn handle_connection(
    read: WsRead,
    mut write: WsWrite,
    db: Arc<ChatDb>,
    subscription_manager: SharedSubscriptionManager,
    plugin_registry: SharedPluginRegistry,
) -> Result<(), ServerError> {
    // Register connection and get the outgoing message receiver
    let (connection_id, mut outgoing_rx) = {
        let mut manager = subscription_manager.write().await;
        manager.register_connection()
    };

    debug!("Connection registered: {}", connection_id);

    // Get the sender for this connection to pass to process_messages
    let outgoing_tx = {
        let manager = subscription_manager.read().await;
        manager.get_sender(&connection_id)
    };

    let outgoing_tx = match outgoing_tx {
        Some(tx) => tx,
        None => {
            error!("Failed to get outgoing sender for connection {}", connection_id);
            return Ok(());
        }
    };

    // Spawn task to forward outgoing messages to WebSocket
    let write_connection_id = connection_id.clone();
    let write_task = tokio::spawn(async move {
        while let Some(msg) = outgoing_rx.recv().await {
            if let Err(e) = write.send(Message::Text(msg)).await {
                error!("Failed to send message to connection {}: {}", write_connection_id, e);
                break;
            }
        }
    });

    // Process incoming messages
    let result = process_messages(read, db.clone(), subscription_manager.clone(), plugin_registry.clone(), &connection_id, outgoing_tx).await;

    // Deactivate plugins for this connection
    if let Err(e) = plugins::deactivate_plugins_on_disconnect(&db, &plugin_registry, &connection_id).await {
        error!("Failed to deactivate plugins for connection {}: {}", connection_id, e);
    }

    // Unregister connection
    {
        let mut manager = subscription_manager.write().await;
        manager.unregister_connection(&connection_id);
    }

    // Wait for write task to finish
    let _ = write_task.await;

    debug!("Connection unregistered: {}", connection_id);

    result
}

async fn process_messages(
    mut read: WsRead,
    db: Arc<ChatDb>,
    subscription_manager: SharedSubscriptionManager,
    plugin_registry: SharedPluginRegistry,
    connection_id: &str,
    outgoing_tx: mpsc::UnboundedSender<String>,
) -> Result<(), ServerError> {
    while let Some(msg) = read.next().await {
        let msg = msg?;

        match msg {
            Message::Text(text) => {
                debug!("Received text message: {}", text);
                
                // Parse as JSON
                let json: Value = match serde_json::from_str(&text) {
                    Ok(v) => v,
                    Err(e) => {
                        error!("Failed to parse JSON: {}", e);
                        let error_response = ErrorResponse::invalid_request(
                            "unknown",
                            format!("Invalid JSON: {}", e),
                        );
                        let _ = outgoing_tx.send(serde_json::to_string(&error_response)?);
                        continue;
                    }
                };

                // Parse as request
                let request: Request = match serde_json::from_value(json) {
                    Ok(r) => r,
                    Err(e) => {
                        error!("Failed to parse request: {}", e);
                        let error_response = ErrorResponse::invalid_request(
                            "unknown",
                            format!("Invalid request format: {}", e),
                        );
                        let _ = outgoing_tx.send(serde_json::to_string(&error_response)?);
                        continue;
                    }
                };

                debug!("Parsed request: method={}, id={}", request.method, request.id);

                // Route to appropriate handler
                let response_value = match handlers::handle_request(
                    request.clone(),
                    &db,
                    connection_id,
                    subscription_manager.clone(),
                    plugin_registry.clone(),
                ).await {
                    Ok(resp) => resp,
                    Err(e) => {
                        error!("Handler error: {}", e);
                        serde_json::to_value(ErrorResponse::internal_error(request.id, format!("Internal error: {}", e)))?
                    }
                };
                let _ = outgoing_tx.send(serde_json::to_string(&response_value)?);
            }
            Message::Binary(_) => {
                debug!("Received binary message (ignoring)");
            }
            Message::Ping(_data) => {
                debug!("Received ping");
                // For ping/pong, we need to send directly through the write channel
                // But since we're using a single outgoing channel, we'll skip pong for now
                // In a production system, you'd want to handle this differently
            }
            Message::Pong(_) => {
                debug!("Received pong");
            }
            Message::Close(frame) => {
                debug!("Received close frame: {:?}", frame);
                break;
            }
            Message::Frame(_) => {
                // Raw frame, not typically used
            }
        }
    }

    Ok(())
}
