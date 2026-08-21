//! Per-connection state and message handling.

use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, error};

use rhd_chat_api::protocol::Request;
use rhd_chat_api::ErrorResponse;
use rhd_db::ChatDb;

use crate::error::ServerError;

/// Type alias for WebSocket read half.
type WsRead = futures_util::stream::SplitStream<tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>>;

/// Type alias for WebSocket write half.
type WsWrite = futures_util::stream::SplitSink<tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>, Message>;

/// Handle a single WebSocket connection.
pub async fn handle_connection(
    mut read: WsRead,
    mut write: WsWrite,
    db: Arc<ChatDb>,
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
                        write.send(Message::Text(serde_json::to_string(&error_response)?)).await?;
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
                        write.send(Message::Text(serde_json::to_string(&error_response)?)).await?;
                        continue;
                    }
                };

                debug!("Parsed request: method={}, id={}", request.method, request.id);

                // TODO: Route to appropriate handler (Phase 3)
                // For now, return "method not implemented" error
                let response = ErrorResponse::invalid_request(
                    request.id,
                    format!("Method '{}' not yet implemented", request.method),
                );
                write.send(Message::Text(serde_json::to_string(&response)?)).await?;
            }
            Message::Binary(_) => {
                debug!("Received binary message (ignoring)");
            }
            Message::Ping(data) => {
                debug!("Received ping");
                write.send(Message::Pong(data)).await?;
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
