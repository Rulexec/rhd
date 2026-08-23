# Backlog Implementation Plan: Messages Queue & Tools Management

## Overview

This plan implements two features from BACKLOG.md:
1. **Chat Messages Queue** - A separate queue collection alongside messages with CRUD operations and events
2. **Chat Tools Management** - Tool management for chat, with types copied from rhd_ai to rhd_chat_api

---

## Task 1: Chat Messages Queue

### 1.1 Database Layer (rhd_db)

**New table: `messages_queue`**
```sql
CREATE TABLE IF NOT EXISTS messages_queue (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    chat_id INTEGER NOT NULL,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL,
    model TEXT,
    thinking_content TEXT,
    tool_calls TEXT,
    FOREIGN KEY (chat_id) REFERENCES chats(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_messages_queue_chat_id ON messages_queue(chat_id);
```

**New table: `message_queue_tags`**
```sql
CREATE TABLE IF NOT EXISTS message_queue_tags (
    message_id INTEGER NOT NULL,
    tag TEXT NOT NULL,
    PRIMARY KEY (message_id, tag),
    FOREIGN KEY (message_id) REFERENCES messages_queue(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_message_queue_tags_tag ON message_queue_tags(tag);
```

**New functions in `rhd_db/src/chat_db/messages_queue.rs`:**
- `add_queue_message(chat_id, role, content, model, thinking_content) -> DbResult<i64>`
- `get_queue_messages(chat_id) -> DbResult<Vec<Message>>` (reuses existing Message type)
- `get_queue_message(message_id) -> DbResult<Option<Message>>`
- `update_queue_message(message_id, content) -> DbResult<()>`
- `update_queue_message_full(message: &Message) -> DbResult<Message>`
- `insert_queue_message(message: &Message) -> DbResult<Message>`
- `delete_queue_message(message_id) -> DbResult<()>`
- `set_queue_message_tags(message_id, tags: &[String]) -> DbResult<()>`
- `add_queue_message_tags(message_id, tags: &[String]) -> DbResult<()>`
- `remove_queue_message_tags(message_id, tags: &[String]) -> DbResult<()>`
- `get_queue_message_tags(message_id) -> DbResult<Vec<String>>`

### 1.2 API Types (rhd_chat_api)

**Note:** Queue messages reuse the existing `Message` type from `common.rs` - no separate type needed.

**New method types:**

| Method | Params | Result |
|--------|--------|--------|
| `addQueueMessage` | `AddQueueMessageParams` | `AddQueueMessageResult` |
| `updateQueueMessage` | `UpdateQueueMessageParams` | `UpdateQueueMessageResult` |
| `deleteQueueMessage` | `DeleteQueueMessageParams` | `DeleteQueueMessageResult` |
| `getQueueMessages` | `GetQueueMessagesParams` | `GetQueueMessagesResult` |

**`AddQueueMessageParams`:**
```rust
pub struct AddQueueMessageParams {
    pub chat_id: i64,
    pub role: String,
    pub content: String,
    pub reasoning_content: Option<String>,
    pub tags: Vec<String>,  // default empty
}
```

**`AddQueueMessageResult`:**
```rust
pub struct AddQueueMessageResult {
    pub message_id: i64,
}
```

**`UpdateQueueMessageParams`:**
```rust
pub struct UpdateQueueMessageParams {
    pub message_id: i64,
    pub content: Option<String>,
    pub reasoning_content: Option<String>,
    pub role: Option<String>,
    pub add_tags: Vec<String>,   // default empty
    pub remove_tags: Vec<String>, // default empty
}
```

**`UpdateQueueMessageResult`:**
```rust
pub struct UpdateQueueMessageResult {}
```

**`DeleteQueueMessageParams`:**
```rust
pub struct DeleteQueueMessageParams {
    pub message_id: i64,
}
```

**`DeleteQueueMessageResult`:**
```rust
pub struct DeleteQueueMessageResult {}
```

**`GetQueueMessagesParams`:**
```rust
pub struct GetQueueMessagesParams {
    pub chat_id: i64,
}
```

**`GetQueueMessagesResult`:**
```rust
pub struct GetQueueMessagesResult {
    pub messages: Vec<Message>,  // Reuses existing Message type
}
```

**New event types:**

| Event | Data Type |
|-------|-----------|
| `queueMessageAdded` | `QueueMessageAddedData` |
| `queueMessageUpdated` | `QueueMessageUpdatedData` |
| `queueMessageDeleted` | `QueueMessageDeletedData` |

**`QueueMessageAddedData`:**
```rust
pub struct QueueMessageAddedData {
    pub chat_id: i64,
    pub message: Message,  // Reuses existing Message type
}
```

**`QueueMessageUpdatedData`:**
```rust
pub struct QueueMessageUpdatedData {
    pub chat_id: i64,
    pub message: Message,  // Reuses existing Message type
}
```

**`QueueMessageDeletedData`:**
```rust
pub struct QueueMessageDeletedData {
    pub chat_id: i64,
    pub message_id: i64,
}
```

### 1.3 Server Handlers (rhd_chat_server)

**New file: `handlers/queue_message.rs`**

Functions:
- `add_queue_message(params, db, request_id, subscription_manager) -> Result<Value, ServerError>`
- `update_queue_message(params, db, request_id, subscription_manager) -> Result<Value, ServerError>`
- `delete_queue_message(params, db, request_id, subscription_manager) -> Result<Value, ServerError>`
- `get_queue_messages(params, db, request_id, subscription_manager) -> Result<Value, ServerError>`

**New file: `events/queue_events.rs`**

Functions:
- `queue_message_added_event(chat_id, message) -> Event`
- `queue_message_updated_event(chat_id, message) -> Event`
- `queue_message_deleted_event(chat_id, message_id) -> Event`

**Update `server.rs`:**
- Add routing for new methods: `addQueueMessage`, `updateQueueMessage`, `deleteQueueMessage`, `getQueueMessages`

### 1.4 Client Methods (rhd_chat_client)

**New methods in `client.rs`:**
- `add_queue_message(params: AddQueueMessageParams) -> Result<AddQueueMessageResult, ClientError>`
- `update_queue_message(params: UpdateQueueMessageParams) -> Result<UpdateQueueMessageResult, ClientError>`
- `delete_queue_message(params: DeleteQueueMessageParams) -> Result<DeleteQueueMessageResult, ClientError>`
- `get_queue_messages(params: GetQueueMessagesParams) -> Result<GetQueueMessagesResult, ClientError>`

**Update `event_stream.rs`:**
- Add `QueueMessageAdded`, `QueueMessageUpdated`, `QueueMessageDeleted` variants to `ChatEvent`
- Add handling for `queueMessageAdded`, `queueMessageUpdated`, `queueMessageDeleted` events

---

## Task 2: Chat Tools Management

### 2.1 API Types (rhd_chat_api)

**Copy types from rhd_ai to new file `tools.rs`:**

```rust
/// Tool definition for chat (copied from rhd_ai).
pub struct ToolDefinition {
    pub tool_type: String,  // "function"
    pub function: FunctionDefinition,
}

pub struct FunctionDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

pub struct ToolCall {
    pub id: String,
    pub call_type: String,  // "function"
    pub function: FunctionCall,
}

pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

/// Wrapper around ToolDefinition that includes the plugin_id which added this tool.
/// Used in getTools response.
pub struct ToolInfo {
    pub plugin_id: String,
    pub tool: ToolDefinition,
}
```

**New method types:**

| Method | Params | Result |
|--------|--------|--------|
| `addTools` | `AddToolsParams` | `AddToolsResult` |
| `removeTools` | `RemoveToolsParams` | `RemoveToolsResult` |
| `getTools` | `GetToolsParams` | `GetToolsResult` |

**`AddToolsParams`:**
```rust
pub struct AddToolsParams {
    pub chat_id: i64,
    pub tools: Vec<ToolDefinition>,
}
```

**`AddToolsResult`:**
```rust
pub struct AddToolsResult {}
```

**`RemoveToolsParams`:**
```rust
pub struct RemoveToolsParams {
    pub chat_id: i64,
    pub tool_names: Vec<String>,  // Names of tools to remove
}
```

**`RemoveToolsResult`:**
```rust
pub struct RemoveToolsResult {}
```

**`GetToolsParams`:**
```rust
pub struct GetToolsParams {
    pub chat_id: i64,
}
```

**`GetToolsResult`:**
```rust
pub struct GetToolsResult {
    pub tools: Vec<ToolInfo>,  // Wrapper includes plugin_id
}
```

**New event types:**

| Event | Data Type |
|-------|-----------|
| `toolsUpdated` | `ToolsUpdatedData` |

**`ToolsUpdatedData`:**
```rust
pub struct ToolsUpdatedData {
    pub chat_id: i64,
    pub tools: Vec<ToolInfo>,  // Wrapper includes plugin_id
}
```

### 2.2 Database Layer (rhd_db)

**New table: `chat_tools`**
```sql
CREATE TABLE IF NOT EXISTS chat_tools (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    chat_id INTEGER NOT NULL,
    plugin_id TEXT NOT NULL,
    tool_name TEXT NOT NULL,
    tool_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (chat_id) REFERENCES chats(id) ON DELETE CASCADE,
    FOREIGN KEY (plugin_id) REFERENCES plugins(plugin_id) ON DELETE CASCADE,
    UNIQUE(chat_id, tool_name)
);

CREATE INDEX IF NOT EXISTS idx_chat_tools_chat_id ON chat_tools(chat_id);
CREATE INDEX IF NOT EXISTS idx_chat_tools_plugin_id ON chat_tools(plugin_id);
```

**New functions in `rhd_db/src/chat_db/tools.rs`:**
- `add_chat_tools(chat_id, plugin_id, tools: &[ToolDefinition]) -> DbResult<()>`
- `remove_chat_tools(chat_id, plugin_id, tool_names: &[String]) -> DbResult<()>`
- `get_chat_tools(chat_id) -> DbResult<Vec<(String, ToolDefinition)>>` (returns plugin_id + tool)
- `get_chat_tools_by_plugin(chat_id, plugin_id) -> DbResult<Vec<ToolDefinition>>`

### 2.3 Server Handlers (rhd_chat_server)

**New file: `handlers/tools.rs`**

Functions:
- `add_tools(params, db, request_id, subscription_manager, plugin_id) -> Result<Value, ServerError>`
  - **Validates that plugin_id is registered for this connection, returns error if not**
- `remove_tools(params, db, request_id, subscription_manager, plugin_id) -> Result<Value, ServerError>`
  - **Validates that plugin_id is registered for this connection, returns error if not**
- `get_tools(params, db, request_id, subscription_manager) -> Result<Value, ServerError>`

**New file: `events/tools_events.rs`**

Functions:
- `tools_updated_event(chat_id, tools: Vec<ToolInfo>) -> Event`

**Update `server.rs`:**
- Add routing for new methods: `addTools`, `removeTools`, `getTools`
- Pass plugin_id to add_tools and remove_tools handlers

### 2.4 Client Methods (rhd_chat_client)

**New methods in `client.rs`:**
- `add_tools(params: AddToolsParams) -> Result<AddToolsResult, ClientError>`
- `remove_tools(params: RemoveToolsParams) -> Result<RemoveToolsResult, ClientError>`
- `get_tools(params: GetToolsParams) -> Result<GetToolsResult, ClientError>`

**Update `event_stream.rs`:**
- Add `ToolsUpdated` variant to `ChatEvent`
- Add handling for `toolsUpdated` event

---

## Implementation Order

1. **Phase 1: Database Layer**
   - Add `messages_queue` and `message_queue_tags` tables to rhd_db
   - Add `chat_tools` table to rhd_db
   - Implement all database functions

2. **Phase 2: API Types (rhd_chat_api)**
   - Add tool types (ToolDefinition, ToolInfo, etc.) to new tools.rs
   - Add queue message method types (add/update/delete/get)
   - Add tools method types (add/remove/get)
   - Add queue message event types
   - Add tools event types

3. **Phase 3: Server Implementation (rhd_chat_server)**
   - Add queue message handlers
   - Add tools handlers (with plugin_id validation)
   - Add event emission functions
   - Update server.rs routing

4. **Phase 4: Client Implementation (rhd_chat_client)**
   - Add queue message client methods
   - Add tools client methods
   - Update event_stream.rs for new events

---

## File Changes Summary

### rhd_db
- `src/chat_db/schema.rs` - Add new tables
- `src/chat_db/messages_queue.rs` - New file with queue message operations
- `src/chat_db/tools.rs` - New file with tools operations
- `src/chat_db/mod.rs` - Export new modules

### rhd_chat_api
- `src/tools.rs` - New file with tool types (ToolDefinition, ToolInfo, etc.)
- `src/methods/add_queue_message.rs` - New file
- `src/methods/update_queue_message.rs` - New file
- `src/methods/delete_queue_message.rs` - New file
- `src/methods/get_queue_messages.rs` - New file
- `src/methods/add_tools.rs` - New file
- `src/methods/remove_tools.rs` - New file
- `src/methods/get_tools.rs` - New file
- `src/methods/mod.rs` - Export new methods
- `src/events/queue_message_added.rs` - New file
- `src/events/queue_message_updated.rs` - New file
- `src/events/queue_message_deleted.rs` - New file
- `src/events/tools_updated.rs` - New file
- `src/events/mod.rs` - Export new events
- `src/lib.rs` - Export new types

### rhd_chat_server
- `src/handlers/queue_message.rs` - New file
- `src/handlers/tools.rs` - New file
- `src/handlers/mod.rs` - Export new handlers
- `src/events/queue_events.rs` - New file
- `src/events/tools_events.rs` - New file
- `src/events.rs` - Export new event functions
- `src/server.rs` - Add routing for new methods

### rhd_chat_client
- `src/client.rs` - Add new client methods
- `src/event_stream.rs` - Add new event types and handling
