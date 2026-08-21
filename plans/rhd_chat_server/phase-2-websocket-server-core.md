# Phase 2: WebSocket Server Core

## Overview

This phase creates the `rhd_chat_server` binary package with a basic WebSocket server that accepts connections, parses JSON messages, and handles the request-response protocol. The server will be able to start, accept WebSocket connections, and route messages to handlers (which will be implemented in Phase 3).

**Scope:**
- Create `rhd_chat_server` package structure
- Implement WebSocket server using `tokio-tungstenite`
- Implement configuration via CLI arguments
- Implement connection handler with message parsing
- Implement error types for server operations
- Add package to workspace

**Out of scope:**
- Request handlers for specific methods (Phase 3)
- Subscription system (Phase 4)
- Plugin management (Phase 5)

## Dependencies

- **Phase 1: Database Extensions** — Must be completed first to have tag/plugin/event tables available
- **rhd_chat_api** — Must be implemented (already done) for protocol types
- **rhd_db** — Must be available for database operations

## Files to Create

### 1. `packages/rhd_chat_server/Cargo.toml`

**Create package manifest:**

```toml
[package]
name = "rhd_chat_server"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "rhd_chat_server"
path = "src/main.rs"

[dependencies]
tokio = { workspace = true }
tokio-tungstenite = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
chrono = { workspace = true }
futures-util = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
clap = { workspace = true }
uuid = { version = "1", features = ["v4"] }
thiserror = { workspace = true }

rhd_chat_api = { path = "../rhd_chat_api" }
rhd_db = { path = "../rhd_db" }
rhd_util = { path = "../rhd_util" }
```

**Note:** Uses workspace dependencies where available. The workspace uses `tokio-tungstenite = "0.24"` (not 0.21 as in the original plan).

### 2. `packages/rhd_chat_server/src/main.rs`

**Create entry point:**

```rust
//! RHD Chat Server — WebSocket server for chat storage and management.
//!
//! This server provides a WebSocket API for chat persistence, message editing,
//! and real-time subscriptions. It does not perform AI calls or MCP tool execution.

mod config;
mod connection;
mod error;
mod server;

use clap::Parser;
use tracing::info;
use tracing_subscriber::EnvFilter;

use crate::config::Config;
use crate::error::ServerError;

#[tokio::main]
async fn main() -> Result<(), ServerError> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("rhd_chat_server=info".parse().unwrap()))
        .init();

    // Parse CLI arguments
    let config = Config::parse();
    info!("Starting RHD Chat Server on {}:{}", config.host, config.port);
    info!("Database path: {}", config.db_path);

    // Start server
    server::run(config).await?;

    Ok(())
}
```

### 3. `packages/rhd_chat_server/src/config.rs`

**Create configuration module:**

```rust
//! Server configuration.

use clap::Parser;

/// RHD Chat Server configuration.
#[derive(Parser, Debug, Clone)]
#[command(name = "rhd_chat_server")]
#[command(about = "WebSocket server for chat storage and management")]
pub struct Config {
    /// Host to bind to
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,

    /// Port to listen on
    #[arg(long, default_value = "8080")]
    pub port: u16,

    /// Path to SQLite database
    #[arg(long, default_value = "./rhd_db/chats.db")]
    pub db_path: String,
}

impl Config {
    /// Get the socket address string.
    pub fn socket_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}
```

### 4. `packages/rhd_chat_server/src/error.rs`

**Create error types:**

```rust
//! Server error types.

use thiserror::Error;

/// Server error types.
#[derive(Error, Debug)]
pub enum ServerError {
    #[error("Database error: {0}")]
    Database(#[from] rhd_db::DbError),

    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl ServerError {
    /// Convert to error code for API response.
    pub fn to_error_code(&self) -> rhd_chat_api::ErrorCode {
        match self {
            ServerError::Database(_) => rhd_chat_api::ErrorCode::InternalError,
            ServerError::WebSocket(_) => rhd_chat_api::ErrorCode::InternalError,
            ServerError::Json(_) => rhd_chat_api::ErrorCode::InvalidRequest,
            ServerError::Io(_) => rhd_chat_api::ErrorCode::InternalError,
            ServerError::Internal(_) => rhd_chat_api::ErrorCode::InternalError,
        }
    }
}
```

### 5. `packages/rhd_chat_server/src/server.rs`

**Create WebSocket server:**

```rust
//! WebSocket server implementation.

use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use tracing::{error, info};

use rhd_db::ChatDb;

use crate::config::Config;
use crate::connection::handle_connection;
use crate::error::ServerError;

/// Run the WebSocket server.
pub async fn run(config: Config) -> Result<(), ServerError> {
    // Initialize database
    let db = Arc::new(ChatDb::new(&config.db_path)?);
    info!("Database initialized at {}", config.db_path);

    // Bind TCP listener
    let listener = TcpListener::bind(&config.socket_addr()).await?;
    info!("WebSocket server listening on ws://{}/", config.socket_addr());

    // Accept connections
    loop {
        let (stream, addr) = listener.accept().await?;
        info!("New connection from: {}", addr);

        let db = Arc::clone(&db);
        tokio::spawn(async move {
            match accept_async(stream).await {
                Ok(ws_stream) => {
                    let (write, read) = ws_stream.split();
                    if let Err(e) = handle_connection(read, write, db).await {
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
```

### 6. `packages/rhd_chat_server/src/connection.rs`

**Create connection handler:**

```rust
//! Per-connection state and message handling.

use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, error};

use rhd_chat_api::protocol::{Request, Response};
use rhd_db::ChatDb;

use crate::error::ServerError;

/// Type alias for WebSocket read half.
type WsRead = futures_util::stream::SplitStream<tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>>;

/// Type alias for WebSocket write half.
type WsWrite = futures_util::sink::SplitSink<tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>, Message>;

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
                        let error_response = Response::error(
                            None,
                            rhd_chat_api::ErrorCode::InvalidRequest,
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
                        let error_response = Response::error(
                            None,
                            rhd_chat_api::ErrorCode::InvalidRequest,
                            format!("Invalid request format: {}", e),
                        );
                        write.send(Message::Text(serde_json::to_string(&error_response)?)).await?;
                        continue;
                    }
                };

                debug!("Parsed request: method={}, id={}", request.method, request.id);

                // TODO: Route to appropriate handler (Phase 3)
                // For now, return "method not implemented" error
                let response = Response::error(
                    Some(request.id),
                    rhd_chat_api::ErrorCode::InvalidRequest,
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
```

### 7. Update `Cargo.toml` (workspace root)

**Add `rhd_chat_server` to workspace members:**

```toml
[workspace]
members = [
    "packages/rhd_util",
    "packages/rhd_ai",
    "packages/rhd_mcp_client",
    "packages/rhd_api",
    "packages/rhd_db",
    "packages/rhd_chat",
    "packages/rhd_chat_api",
    "packages/rhd_chat_server",  # ADD THIS LINE
    "packages/rhd_app",
    "packages/rhd_test",
    "packages/rhd_fsm",
]
```

## Implementation Notes

1. **WebSocket Library Version**: The workspace uses `tokio-tungstenite = "0.24"`, not 0.21 as mentioned in the original plan. Ensure the Cargo.toml uses workspace dependencies.

2. **Connection Handling**: Each connection runs in its own tokio task. The connection handler splits the WebSocket stream into read and write halves for concurrent operation.

3. **Message Parsing**: The server currently parses incoming messages as JSON and validates them as `Request` objects. Invalid JSON or malformed requests receive an error response.

4. **Error Handling**: All errors are converted to `ServerError` and logged. Client-facing errors are sent as `Response::error()` messages.

5. **Database Access**: The database is wrapped in `Arc<ChatDb>` and shared across all connections. Each connection handler receives a clone of the Arc.

6. **Stub Implementation**: The connection handler currently returns "method not implemented" for all requests. Phase 3 will add the actual request routing and handlers.

7. **Tracing**: Uses `tracing` for structured logging. The log level can be controlled via the `RUST_LOG` environment variable.

## Testing

### Manual Testing

After implementation, you can test the server manually:

1. **Start the server:**
   ```bash
   cargo run --bin rhd_chat_server -- --port 8080 --db-path ./test.db
   ```

2. **Connect with a WebSocket client:**
   Use `websocat` or a browser console:
   ```bash
   websocat ws://127.0.0.1:8080/
   ```

3. **Send a test request:**
   ```json
   {"type": "request", "id": "test-1", "method": "listChats", "params": {}}
   ```

4. **Expected response:**
   ```json
   {"type": "response", "id": "test-1", "success": false, "errorCode": "INVALID_REQUEST", "error": "Method 'listChats' not yet implemented"}
   ```

### Build Test

```bash
cargo build --bin rhd_chat_server
```

## Success Criteria

1. Package `rhd_chat_server` compiles without errors
2. Server starts and listens on the configured port
3. Server accepts WebSocket connections
4. Server parses JSON messages correctly
5. Server returns error responses for invalid JSON or unknown methods
6. Server handles ping/pong and close frames correctly
7. Database is initialized successfully
8. No breaking changes to existing packages

## Next Steps

After this phase is complete, proceed to **Phase 3: Request Handlers** to implement the actual chat and message operations.
