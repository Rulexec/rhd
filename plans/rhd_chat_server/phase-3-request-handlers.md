# Phase 3: Request Handlers

## Overview

This phase implements request handlers for all chat and message operations. The handlers will parse incoming requests, call the appropriate database methods, convert between internal and API types, and return responses. This phase covers 8 methods: `createChat`, `listChats`, `getChat`, `deleteChat`, `updateChat`, `addMessage`, `updateMessage`, and `deleteMessage`.

**Scope:**
- Create handler module structure
- Implement 5 chat operation handlers
- Implement 3 message operation handlers
- Implement type conversion between `rhd_db` and `rhd_chat_api` types
- Integrate handlers into connection handler
- Handle tag operations for create/update methods

**Out of scope:**
- Subscription handlers (Phase 4)
- Plugin handlers (Phase 5)
- Event broadcasting (Phase 4)

## Dependencies

- **Phase 1: Database Extensions** — Must be completed for tag operations
- **Phase 2: WebSocket Server Core** — Must be completed for connection handling
- **rhd_chat_api** — Must be implemented for all method types

## Files to Create

### 1. `packages/rhd_chat_server/src/handlers/mod.rs`

**Create handler module:**

```rust
//! Request handlers for the chat WebSocket protocol.

pub mod chat;
pub mod message;

use serde_json::Value;

use rhd_chat_api::protocol::{Request, Response};
use rhd_chat_api::ErrorResponse;
use rhd_db::ChatDb;

use crate::error::ServerError;

/// Route a request to the appropriate handler.
pub async fn handle_request(
    request: Request,
    db: &ChatDb,
) -> Result<Response, ServerError> {
    let request_id = request.id.clone();
    
    match request.method.as_str() {
        // Chat methods
        "createChat" => chat::create_chat(request.params, db, &request_id).await,
        "listChats" => chat::list_chats(request.params, db, &request_id).await,
        "getChat" => chat::get_chat(request.params, db, &request_id).await,
        "deleteChat" => chat::delete_chat(request.params, db, &request_id).await,
        "updateChat" => chat::update_chat(request.params, db, &request_id).await,
        
        // Message methods
        "addMessage" => message::add_message(request.params, db, &request_id).await,
        "updateMessage" => message::update_message(request.params, db, &request_id).await,
        "deleteMessage" => message::delete_message(request.params, db, &request_id).await,
        
        // Unknown method
        _ => Ok(ErrorResponse::invalid_request(
            request_id,
            format!("Unknown method: {}", request.method),
        )),
    }
}
```

### 2. `packages/rhd_chat_server/src/handlers/chat.rs`

**Create chat handlers:**

```rust
//! Chat operation handlers.

use chrono::{DateTime, Utc};
use serde_json::Value;

use rhd_chat_api::common::{Chat, ChatSummary};
use rhd_chat_api::methods::{
    CreateChatParams, CreateChatResult, DeleteChatParams, DeleteChatResult, GetChatParams,
    GetChatResult, ListChatsParams, ListChatsResult, UpdateChatParams, UpdateChatResult,
};
use rhd_chat_api::protocol::Response;
use rhd_chat_api::ErrorResponse;
use rhd_db::ChatDb;

use crate::error::ServerError;

/// Convert rhd_db::ChatInfo to rhd_chat_api::Chat.
fn convert_chat_info_to_api(
    chat_info: rhd_db::ChatInfo,
    tags: Vec<String>,
) -> Result<Chat, ServerError> {
    let created_at: DateTime<Utc> = chat_info
        .created_at
        .parse()
        .map_err(|e| ServerError::Internal(format!("Failed to parse created_at: {}", e)))?;
    let updated_at: DateTime<Utc> = chat_info
        .updated_at
        .parse()
        .map_err(|e| ServerError::Internal(format!("Failed to parse updated_at: {}", e)))?;

    Ok(Chat {
        id: chat_info.id,
        title: chat_info.title,
        created_at,
        updated_at,
        tags,
    })
}

/// Convert rhd_db::ChatInfo to rhd_chat_api::ChatSummary.
fn convert_chat_info_to_summary(
    chat_info: rhd_db::ChatInfo,
    tags: Vec<String>,
) -> Result<ChatSummary, ServerError> {
    let created_at: DateTime<Utc> = chat_info
        .created_at
        .parse()
        .map_err(|e| ServerError::Internal(format!("Failed to parse created_at: {}", e)))?;
    let updated_at: DateTime<Utc> = chat_info
        .updated_at
        .parse()
        .map_err(|e| ServerError::Internal(format!("Failed to parse updated_at: {}", e)))?;

    Ok(ChatSummary {
        id: chat_info.id,
        title: chat_info.title,
        created_at,
        updated_at,
        tags,
    })
}

/// Convert rhd_db::Message to rhd_chat_api::Message.
fn convert_message_to_api(
    msg: rhd_db::Message,
    tags: Vec<String>,
) -> Result<rhd_chat_api::Message, ServerError> {
    let created_at: DateTime<Utc> = msg
        .created_at
        .parse()
        .map_err(|e| ServerError::Internal(format!("Failed to parse created_at: {}", e)))?;

    Ok(rhd_chat_api::Message {
        id: msg.id,
        chat_id: msg.chat_id,
        role: msg.role,
        content: msg.content,
        created_at,
        reasoning_content: msg.thinking_content,
        tags,
    })
}

/// Handle `createChat` request.
pub async fn create_chat(
    params: Value,
    db: &ChatDb,
    request_id: &str,
) -> Result<Response, ServerError> {
    let params: CreateChatParams = match serde_json::from_value(params) {
        Ok(p) => p,
        Err(e) => {
            return Ok(ErrorResponse::invalid_request(
                request_id,
                format!("Invalid params: {}", e),
            ));
        }
    };

    // Create chat
    let chat_id = db.create_chat(&params.title)?;

    // Set tags if provided
    if !params.tags.is_empty() {
        db.set_chat_tags(chat_id, &params.tags)?;
    }

    // Return result
    let result = CreateChatResult { chat_id };
    Ok(Response::success(request_id, serde_json::to_value(result)?))
}

/// Handle `listChats` request.
pub async fn list_chats(
    params: Value,
    db: &ChatDb,
    request_id: &str,
) -> Result<Response, ServerError> {
    let params: ListChatsParams = match serde_json::from_value(params) {
        Ok(p) => p,
        Err(e) => {
            return Ok(ErrorResponse::invalid_request(
                request_id,
                format!("Invalid params: {}", e),
            ));
        }
    };

    // Get all chats
    let chat_infos = db.list_chats()?;

    // Convert to API types with tags
    let mut chats = Vec::new();
    for chat_info in chat_infos {
        // If tags filter is provided, check if chat has any of the tags
        if !params.tags.is_empty() {
            let chat_tags = db.get_chat_tags(chat_info.id)?;
            let has_matching_tag = params.tags.iter().any(|t| chat_tags.contains(t));
            if !has_matching_tag {
                continue;
            }
            let summary = convert_chat_info_to_summary(chat_info, chat_tags)?;
            chats.push(summary);
        } else {
            let chat_tags = db.get_chat_tags(chat_info.id)?;
            let summary = convert_chat_info_to_summary(chat_info, chat_tags)?;
            chats.push(summary);
        }
    }

    let result = ListChatsResult { chats };
    Ok(Response::success(request_id, serde_json::to_value(result)?))
}

/// Handle `getChat` request.
pub async fn get_chat(
    params: Value,
    db: &ChatDb,
    request_id: &str,
) -> Result<Response, ServerError> {
    let params: GetChatParams = match serde_json::from_value(params) {
        Ok(p) => p,
        Err(e) => {
            return Ok(ErrorResponse::invalid_request(
                request_id,
                format!("Invalid params: {}", e),
            ));
        }
    };

    // Get chat info
    let chat_info = match db.get_chat(params.chat_id)? {
        Some(c) => c,
        None => {
            return Ok(ErrorResponse::chat_not_found(request_id, params.chat_id));
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

    let result = GetChatResult { chat, messages };
    Ok(Response::success(request_id, serde_json::to_value(result)?))
}

/// Handle `deleteChat` request.
pub async fn delete_chat(
    params: Value,
    db: &ChatDb,
    request_id: &str,
) -> Result<Response, ServerError> {
    let params: DeleteChatParams = match serde_json::from_value(params) {
        Ok(p) => p,
        Err(e) => {
            return Ok(ErrorResponse::invalid_request(
                request_id,
                format!("Invalid params: {}", e),
            ));
        }
    };

    // Check if chat exists
    if db.get_chat(params.chat_id)?.is_none() {
        return Ok(ErrorResponse::chat_not_found(request_id, params.chat_id));
    }

    // Delete chat (tags will be deleted by CASCADE)
    db.delete_chat(params.chat_id)?;

    let result = DeleteChatResult {};
    Ok(Response::success(request_id, serde_json::to_value(result)?))
}

/// Handle `updateChat` request.
pub async fn update_chat(
    params: Value,
    db: &ChatDb,
    request_id: &str,
) -> Result<Response, ServerError> {
    let params: UpdateChatParams = match serde_json::from_value(params) {
        Ok(p) => p,
        Err(e) => {
            return Ok(ErrorResponse::invalid_request(
                request_id,
                format!("Invalid params: {}", e),
            ));
        }
    };

    // Check if chat exists
    if db.get_chat(params.chat_id)?.is_none() {
        return Ok(ErrorResponse::chat_not_found(request_id, params.chat_id));
    }

    // Update title if provided
    if let Some(title) = params.title {
        db.update_chat_title(params.chat_id, &title)?;
    }

    // Add tags if provided
    if !params.add_tags.is_empty() {
        db.add_chat_tags(params.chat_id, &params.add_tags)?;
    }

    // Remove tags if provided
    if !params.remove_tags.is_empty() {
        db.remove_chat_tags(params.chat_id, &params.remove_tags)?;
    }

    // Touch chat to update updated_at
    db.touch_chat(params.chat_id)?;

    let result = UpdateChatResult {};
    Ok(Response::success(request_id, serde_json::to_value(result)?))
}
```

### 3. `packages/rhd_chat_server/src/handlers/message.rs`

**Create message handlers:**

```rust
//! Message operation handlers.

use chrono::{DateTime, Utc};
use serde_json::Value;

use rhd_chat_api::methods::{
    AddMessageParams, AddMessageResult, DeleteMessageParams, DeleteMessageResult,
    UpdateMessageParams, UpdateMessageResult,
};
use rhd_chat_api::protocol::Response;
use rhd_chat_api::ErrorResponse;
use rhd_db::ChatDb;

use crate::error::ServerError;

/// Handle `addMessage` request.
pub async fn add_message(
    params: Value,
    db: &ChatDb,
    request_id: &str,
) -> Result<Response, ServerError> {
    let params: AddMessageParams = match serde_json::from_value(params) {
        Ok(p) => p,
        Err(e) => {
            return Ok(ErrorResponse::invalid_request(
                request_id,
                format!("Invalid params: {}", e),
            ));
        }
    };

    // Check if chat exists
    if db.get_chat(params.chat_id)?.is_none() {
        return Ok(ErrorResponse::chat_not_found(request_id, params.chat_id));
    }

    // Add message
    let message_id = db.add_message(
        params.chat_id,
        &params.role,
        &params.content,
        None, // model
        params.reasoning_content.as_deref(),
    )?;

    // Set tags if provided
    if !params.tags.is_empty() {
        db.set_message_tags(message_id, &params.tags)?;
    }

    // Touch chat to update updated_at
    db.touch_chat(params.chat_id)?;

    let result = AddMessageResult { message_id };
    Ok(Response::success(request_id, serde_json::to_value(result)?))
}

/// Handle `updateMessage` request.
pub async fn update_message(
    params: Value,
    db: &ChatDb,
    request_id: &str,
) -> Result<Response, ServerError> {
    let params: UpdateMessageParams = match serde_json::from_value(params) {
        Ok(p) => p,
        Err(e) => {
            return Ok(ErrorResponse::invalid_request(
                request_id,
                format!("Invalid params: {}", e),
            ));
        }
    };

    // Check if message exists
    let message = match db.get_message(params.message_id)? {
        Some(m) => m,
        None => {
            return Ok(ErrorResponse::message_not_found(request_id, params.message_id));
        }
    };

    // Update content if provided
    if let Some(content) = params.content {
        db.update_message(params.message_id, &content)?;
    }

    // Note: rhd_db doesn't have methods to update reasoning_content or role separately
    // For now, we'll need to use update_message_full or add new methods to rhd_db
    // This is a limitation that should be addressed in a future phase

    // Add tags if provided
    if !params.add_tags.is_empty() {
        db.add_message_tags(params.message_id, &params.add_tags)?;
    }

    // Remove tags if provided
    if !params.remove_tags.is_empty() {
        db.remove_message_tags(params.message_id, &params.remove_tags)?;
    }

    // Touch chat to update updated_at
    db.touch_chat(message.chat_id)?;

    let result = UpdateMessageResult {};
    Ok(Response::success(request_id, serde_json::to_value(result)?))
}

/// Handle `deleteMessage` request.
pub async fn delete_message(
    params: Value,
    db: &ChatDb,
    request_id: &str,
) -> Result<Response, ServerError> {
    let params: DeleteMessageParams = match serde_json::from_value(params) {
        Ok(p) => p,
        Err(e) => {
            return Ok(ErrorResponse::invalid_request(
                request_id,
                format!("Invalid params: {}", e),
            ));
        }
    };

    // Check if message exists
    let message = match db.get_message(params.message_id)? {
        Some(m) => m,
        None => {
            return Ok(ErrorResponse::message_not_found(request_id, params.message_id));
        }
    };

    // Delete message (tags will be deleted by CASCADE)
    db.delete_message(params.message_id)?;

    // Touch chat to update updated_at
    db.touch_chat(message.chat_id)?;

    let result = DeleteMessageResult {};
    Ok(Response::success(request_id, serde_json::to_value(result)?))
}
```

### 4. Update `packages/rhd_chat_server/src/connection.rs`

**Integrate handlers into connection handler:**

Replace the TODO section in `handle_connection` with actual handler routing:

```rust
// Import handlers module
use crate::handlers;

// ... in handle_connection function, replace the TODO section with:

// Route to appropriate handler
let response = match handlers::handle_request(request, &db).await {
    Ok(resp) => resp,
    Err(e) => {
        error!("Handler error: {}", e);
        ErrorResponse::internal_error(request.id, format!("Internal error: {}", e))
    }
};

write.send(Message::Text(serde_json::to_string(&response)?)).await?;
```

## Implementation Notes

1. **Type Conversion**: The handlers convert between `rhd_db` types (which use `String` for timestamps) and `rhd_chat_api` types (which use `DateTime<Utc>`). This requires parsing timestamps and handling errors.

2. **Tag Operations**: 
   - `createChat` and `addMessage` use `set_*_tags()` to set initial tags
   - `updateChat` and `updateMessage` use `add_*_tags()` and `remove_*_tags()` for partial updates
   - Tags are fetched separately for each chat/message to include in API responses

3. **Error Handling**: 
   - Invalid params return `ErrorResponse::invalid_request()`
   - Missing chat/message returns `ErrorResponse::chat_not_found()` or `message_not_found()`
   - Database errors are propagated as `ServerError` and converted to `ErrorResponse::internal_error()`

4. **Touch Chat**: After message operations, we call `db.touch_chat()` to update the chat's `updated_at` timestamp.

5. **Limitation**: The current `rhd_db` doesn't have methods to update `reasoning_content` or `role` separately. The `updateMessage` handler only updates `content` and tags. This should be addressed by adding new methods to `rhd_db` in a future phase.

6. **Filtering**: `listChats` supports filtering by tags. If tags are provided, only chats with at least one matching tag are returned.

## Testing

### Manual Testing

After implementation, test each method:

1. **createChat:**
   ```json
   {"type": "request", "id": "1", "method": "createChat", "params": {"title": "Test Chat", "tags": ["test"]}}
   ```
   Expected: `{"type": "response", "id": "1", "success": true, "data": {"chatId": 1}}`

2. **listChats:**
   ```json
   {"type": "request", "id": "2", "method": "listChats", "params": {}}
   ```
   Expected: List with the created chat

3. **getChat:**
   ```json
   {"type": "request", "id": "3", "method": "getChat", "params": {"chatId": 1}}
   ```
   Expected: Full chat with messages

4. **updateChat:**
   ```json
   {"type": "request", "id": "4", "method": "updateChat", "params": {"chatId": 1, "title": "Updated Title", "addTags": ["new-tag"]}}
   ```
   Expected: Success response

5. **addMessage:**
   ```json
   {"type": "request", "id": "5", "method": "addMessage", "params": {"chatId": 1, "role": "user", "content": "Hello", "tags": ["greeting"]}}
   ```
   Expected: `{"type": "response", "id": "5", "success": true, "data": {"messageId": 1}}`

6. **updateMessage:**
   ```json
   {"type": "request", "id": "6", "method": "updateMessage", "params": {"messageId": 1, "content": "Updated content", "addTags": ["updated"]}}
   ```
   Expected: Success response

7. **deleteMessage:**
   ```json
   {"type": "request", "id": "7", "method": "deleteMessage", "params": {"messageId": 1}}
   ```
   Expected: Success response

8. **deleteChat:**
   ```json
   {"type": "request", "id": "8", "method": "deleteChat", "params": {"chatId": 1}}
   ```
   Expected: Success response

### Error Cases

Test error handling:

1. **Invalid params:**
   ```json
   {"type": "request", "id": "9", "method": "createChat", "params": {}}
   ```
   Expected: Error response with `INVALID_REQUEST`

2. **Chat not found:**
   ```json
   {"type": "request", "id": "10", "method": "getChat", "params": {"chatId": 999}}
   ```
   Expected: Error response with `CHAT_NOT_FOUND`

3. **Unknown method:**
   ```json
   {"type": "request", "id": "11", "method": "unknownMethod", "params": {}}
   ```
   Expected: Error response with `INVALID_REQUEST`

## Success Criteria

1. All 8 chat and message methods work correctly
2. Tags are properly set, added, and removed
3. Type conversions between `rhd_db` and `rhd_chat_api` work correctly
4. Error responses are returned for invalid requests
5. Chat `updated_at` is updated after message operations
6. No breaking changes to existing functionality

## Next Steps

After this phase is complete, proceed to **Phase 4: Subscription System** to implement real-time event broadcasting.
