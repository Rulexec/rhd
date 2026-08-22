# Phase 2: Server-Side Enhancements

## Overview

This phase extends the chat server API to support plugin requirements, specifically:
1. Returning queued messages count in `getChat` response
2. Supporting random port binding for integration testing

**Scope**:
- Add `queued_messages_count` field to `GetChatResult`
- Update `get_chat` handler to query and return queue count
- Add `count_queue_messages` function to database layer
- Support random port binding (port 0) in chat server
- Update existing tests

**Out of Scope**:
- Plugin implementation (Phase 4)
- Chat client enhancements (Phase 3)
- Integration testing (Phase 5)

## Files to Modify

### 1. `packages/rhd_db/src/chat_db/messages_queue.rs`

**Purpose**: Add function to count queue messages for a chat.

**Add function** (after line 207):
```rust
/// Count queue messages for a chat
pub(crate) fn count_queue_messages(conn: &Mutex<Connection>, chat_id: i64) -> DbResult<i64> {
    let conn = conn
        .lock()
        .map_err(|e| DbError::InitializationError(e.to_string()))?;
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM messages_queue WHERE chat_id = ?1",
        params![chat_id],
        |row| row.get(0),
    )?;
    Ok(count)
}
```

### 2. `packages/rhd_db/src/chat_db/mod.rs`

**Purpose**: Expose the count function through the ChatDb API.

**Add method** (after line 233, in the queue message operations section):
```rust
    pub fn count_queue_messages(&self, chat_id: i64) -> DbResult<i64> {
        messages_queue::count_queue_messages(&self.conn, chat_id)
    }
```

### 3. `packages/rhd_chat_api/src/methods/get_chat.rs`

**Purpose**: Add `queued_messages_count` field to `GetChatResult`.

**Modify struct** (lines 48-55):
```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GetChatResult {
    /// Full chat information.
    pub chat: Chat,
    /// List of messages in the chat.
    pub messages: Vec<Message>,
    /// Number of messages in the queue.
    pub queued_messages_count: i64,
}
```

**Update test** (lines 73-93):
```rust
    #[test]
    fn test_get_chat_result_serialization() {
        use chrono::Utc;

        let result = GetChatResult {
            chat: Chat {
                id: 123,
                title: "Test".to_string(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                tags: vec![],
            },
            messages: vec![],
            queued_messages_count: 5,
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"chat\""));
        assert!(json.contains("\"messages\""));
        assert!(json.contains("\"queuedMessagesCount\":5"));

        let deserialized: GetChatResult = serde_json::from_str(&json).unwrap();
        assert_eq!(result, deserialized);
    }
```

### 4. `packages/rhd_chat_server/src/handlers/chat.rs`

**Purpose**: Update `get_chat` handler to include queued messages count.

**Modify function** (lines 166-205):
```rust
/// Handle `getChat` request.
pub async fn get_chat(
    params: Value,
    db: &ChatDb,
    request_id: &str,
) -> Result<Value, ServerError> {
    let params: GetChatParams = match serde_json::from_value(params) {
        Ok(p) => p,
        Err(e) => {
            return Ok(serde_json::to_value(ErrorResponse::invalid_request(
                request_id,
                format!("Invalid params: {}", e),
            ))?);
        }
    };

    // Get chat info
    let chat_info = match db.get_chat(params.chat_id)? {
        Some(c) => c,
        None => {
            return Ok(serde_json::to_value(ErrorResponse::chat_not_found(request_id, params.chat_id))?);
        }
    };

    // Get chat tags
    let chat_tags = db.get_chat_tags(params.chat_id)?;
    let chat = convert_chat_info_to_api(chat_info, chat_tags)?;

    // Get messages
    let db_messages = db.get_messages(params.chat_id)?;
    let mut messages = Vec::new();
    for msg in db_messages {
        let msg_tags = db.get_message_tags(msg.id)?;
        let api_msg = convert_message_to_api(msg, msg_tags)?;
        messages.push(api_msg);
    }

    // Get queued messages count
    let queued_messages_count = db.count_queue_messages(params.chat_id)?;

    let result = GetChatResult {
        chat,
        messages,
        queued_messages_count,
    };
    Ok(serde_json::to_value(Response::success(request_id, serde_json::to_value(result)?))?)
}
```

### 5. `packages/rhd_chat_server/src/server.rs`

**Purpose**: Support binding to random port (port 0) and return actual bound port.

**Modify function** (lines 18-59):
```rust
/// Run the WebSocket server.
/// Returns the actual port the server is listening on.
pub async fn run(config: Config) -> Result<u16, ServerError> {
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

    // Accept connections
    loop {
        let (stream, addr) = listener.accept().await?;
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
}
```

**Note**: The function now returns `Result<u16, ServerError>` instead of `Result<(), ServerError>`. The return value is the actual port. However, since the function runs in a loop and never returns normally, we need to refactor this to support testing.

**Alternative approach for testing support**:

Add a new function for testing that returns immediately after binding:

```rust
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
```

**Keep the original `run` function** for backward compatibility, but have it call `start`:

```rust
/// Run the WebSocket server (blocking version).
pub async fn run(config: Config) -> Result<(), ServerError> {
    let (_port, handle) = start(config).await?;
    handle.await.map_err(|e| ServerError::Internal(format!("Server task failed: {}", e)))?;
    Ok(())
}
```

### 6. `packages/rhd_chat_server/src/main.rs`

**Purpose**: Update main to handle the new return type.

**No changes needed** if we keep the `run` function as a wrapper. The main function calls `run(config).await` which still returns `Result<(), ServerError>`.

## Tests

### Unit Tests for Database Layer

Add test in `packages/rhd_db/src/chat_db/tests.rs` (or create new test file):

```rust
#[test]
fn test_count_queue_messages() {
    let db = ChatDb::new(":memory:").unwrap();
    
    // Create a chat
    let chat_id = db.create_chat("Test Chat").unwrap();
    
    // Initially, queue should be empty
    assert_eq!(db.count_queue_messages(chat_id).unwrap(), 0);
    
    // Add queue messages
    db.add_queue_message(chat_id, "user", "Message 1", None, None).unwrap();
    db.add_queue_message(chat_id, "user", "Message 2", None, None).unwrap();
    
    // Count should be 2
    assert_eq!(db.count_queue_messages(chat_id).unwrap(), 2);
    
    // Delete one message
    let queue_messages = db.get_queue_messages(chat_id).unwrap();
    db.delete_queue_message(queue_messages[0].id).unwrap();
    
    // Count should be 1
    assert_eq!(db.count_queue_messages(chat_id).unwrap(), 1);
}
```

### Integration Tests for getChat Handler

Update existing tests in `packages/rhd_chat_server/tests/websocket_tests.rs`:

```rust
#[tokio::test]
async fn test_get_chat_includes_queue_count() {
    // Start server on random port
    let config = Config {
        host: "127.0.0.1".to_string(),
        port: 0, // Random port
        db_path: ":memory:".to_string(),
    };
    let (port, _handle) = rhd_chat_server::server::start(config).await.unwrap();
    
    // Connect client
    let client = ChatClient::connect(&format!("ws://127.0.0.1:{}/", port)).await.unwrap();
    
    // Create chat
    let create_result = client.create_chat(CreateChatParams {
        title: "Test Chat".to_string(),
        tags: vec![],
    }).await.unwrap();
    
    // Add queue messages
    client.add_queue_message(AddQueueMessageParams {
        chat_id: create_result.chat_id,
        role: "user".to_string(),
        content: "Queue message 1".to_string(),
        reasoning_content: None,
        tags: vec![],
    }).await.unwrap();
    
    client.add_queue_message(AddQueueMessageParams {
        chat_id: create_result.chat_id,
        role: "user".to_string(),
        content: "Queue message 2".to_string(),
        reasoning_content: None,
        tags: vec![],
    }).await.unwrap();
    
    // Get chat
    let get_result = client.get_chat(GetChatParams {
        chat_id: create_result.chat_id,
    }).await.unwrap();
    
    // Verify queue count
    assert_eq!(get_result.queued_messages_count, 2);
}
```

## Implementation Notes

1. **Database Function**: The `count_queue_messages` function uses a simple `SELECT COUNT(*)` query, which is efficient for SQLite.

2. **API Backward Compatibility**: Adding a new field to `GetChatResult` is backward compatible. Old clients will simply ignore the new field.

3. **Random Port Support**: Using port 0 lets the OS assign an available port. The `start` function returns the actual port so tests can connect to it.

4. **Server Refactoring**: The `run` function is kept for backward compatibility, but internally calls `start` which is non-blocking and returns the port.

5. **Error Handling**: All new functions properly propagate errors using the existing error types (`DbResult`, `ServerError`).

## Dependencies

- **Depends on**: Phase 1 (plugin system foundation must exist to validate the API changes)
- **Must be completed before**: Phase 4 (plugin needs the queued messages count)

## Success Criteria

- [ ] `count_queue_messages` function exists in database layer
- [ ] `GetChatResult` includes `queued_messages_count` field
- [ ] `get_chat` handler returns correct queue count
- [ ] Chat server supports random port binding (port 0)
- [ ] `start` function returns actual bound port
- [ ] All existing tests pass
- [ ] New tests verify queue count functionality
- [ ] New tests verify random port binding
