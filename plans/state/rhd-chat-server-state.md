# RHD Chat Server Current Implementation State

**Last Updated**: 2026-08-21

## Overview

The `rhd_chat_server` package is a **planned** binary crate that will provide a WebSocket server for chat storage and management. This server is part of the new RHD architecture and focuses solely on chat persistence, message editing, and real-time subscriptions — without AI calls or MCP tool execution.

## Current Implementation Status

### ⏳ Not Yet Implemented

The `rhd_chat_server` package has not been created yet. All components are in the planning phase.

**Package location (planned)**: `packages/rhd_chat_server/`

## Planned Components

### Phase 1: Database Extensions
**Status**: ⏳ Not started

**Goal**: Extend database schema for tags, plugins, and custom events.

**Files to modify**:
- `packages/rhd_db/src/chat_db/schema.rs` — Add tag tables, plugins table, custom events tables
- `packages/rhd_db/src/chat_db/mod.rs` — Add tag methods, plugin methods, custom event methods
- `packages/rhd_db/src/chat_db/tags.rs` — New file for tag operations
- `packages/rhd_db/src/chat_db/plugins.rs` — New file for plugin operations
- `packages/rhd_db/src/chat_db/custom_events.rs` — New file for custom event operations

**Database tables to add**:
- `chat_tags` — Chat tags (chat_id, tag)
- `message_tags` — Message tags (message_id, tag)
- `plugins` — Plugin registry (plugin_id, is_active, created_at)
- `custom_events` — Custom events (event_id, event_name, sender_plugin_id, additional, created_at)
- `custom_event_acks` — Custom event acknowledgments (event_id, plugin_id, acked_at)

### Phase 2: WebSocket Server Core
**Status**: ⏳ Not started

**Goal**: Create the `rhd_chat_server` package and implement basic WebSocket server with connection handling.

**Files to create**:
- `packages/rhd_chat_server/Cargo.toml` — Package manifest
- `packages/rhd_chat_server/src/main.rs` — Entry point
- `packages/rhd_chat_server/src/config.rs` — Configuration
- `packages/rhd_chat_server/src/server.rs` — Server startup and listener
- `packages/rhd_chat_server/src/connection.rs` — Connection handler
- `packages/rhd_chat_server/src/error.rs` — Server error types

### Phase 3: Request Handlers
**Status**: ⏳ Not started

**Goal**: Implement all request handlers for chat and message operations.

**Files to create**:
- `packages/rhd_chat_server/src/handlers/mod.rs`
- `packages/rhd_chat_server/src/handlers/chat.rs` — Chat operations (create, list, get, delete, update)
- `packages/rhd_chat_server/src/handlers/message.rs` — Message operations (add, update, delete)

### Phase 4: Subscription System
**Status**: ⏳ Not started

**Goal**: Implement subscription mechanism for real-time events.

**Files to create**:
- `packages/rhd_chat_server/src/subscriptions.rs` — Subscription manager
- `packages/rhd_chat_server/src/events.rs` — Event broadcasting
- `packages/rhd_chat_server/src/handlers/subscription.rs` — Subscribe handlers

### Phase 5: Plugin Management System
**Status**: ⏳ Not started

**Goal**: Implement plugin registration, tracking, and custom event broadcasting.

**Files to create**:
- `packages/rhd_chat_server/src/plugins.rs` — Plugin registry and management
- `packages/rhd_chat_server/src/custom_events.rs` — Custom event broadcasting and acknowledgment tracking
- `packages/rhd_chat_server/src/handlers/plugin.rs` — Plugin operations (register, get, remove, subscribe, send/ack events)

### Phase 6: Integration and Testing
**Status**: ⏳ Not started

**Goal**: Wire everything together and add tests.

**Files to modify**:
- `packages/rhd_chat_server/src/main.rs` — Complete implementation
- `packages/rhd_chat_server/src/connection.rs` — Integrate handlers, subscriptions, and plugins

## Planned File Structure

```
packages/rhd_chat_server/
├── Cargo.toml
├── src/
│   ├── main.rs              # Entry point, server startup
│   ├── config.rs            # Configuration (port, db path)
│   ├── server.rs            # WebSocket server implementation
│   ├── connection.rs        # Per-connection state and message handling
│   ├── subscriptions.rs     # Subscription manager
│   ├── plugins.rs           # Plugin registry and management
│   ├── custom_events.rs     # Custom event broadcasting and acknowledgment tracking
│   ├── handlers/            # Request handlers
│   │   ├── mod.rs
│   │   ├── chat.rs          # Chat operations (create, list, get, delete, update)
│   │   ├── message.rs       # Message operations (add, update, delete)
│   │   ├── subscription.rs  # Subscribe/unsubscribe operations
│   │   └── plugin.rs        # Plugin operations (register, get, remove, subscribe, send/ack events)
│   ├── events.rs            # Event broadcasting
│   └── error.rs             # Server error types
```

## Planned Dependencies

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
tokio-tungstenite = "0.21"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
rhd_chat_api = { path = "../rhd_chat_api" }
rhd_db = { path = "../rhd_db" }
rhd_util = { path = "../rhd_util" }
uuid = { version = "1", features = ["v4"] }
futures-util = "0.3"
```

## Planned API Methods (20 total)

### Chat Methods (5)
1. `createChat` — Create a new chat
2. `listChats` — Get list of all chats
3. `getChat` — Get chat info and all messages
4. `deleteChat` — Delete a chat and all its messages
5. `updateChat` — Partially update a chat

### Message Methods (3)
6. `addMessage` — Add a new message to a chat
7. `updateMessage` — Partially update a message
8. `deleteMessage` — Delete a message

### Subscription Methods (4)
9. `subscribeChat` — Subscribe to changes in a specific chat
10. `unsubscribeChat` — Unsubscribe from chat changes
11. `subscribeChatsList` — Subscribe to changes in the chats list
12. `unsubscribeChatsList` — Unsubscribe from chats list changes

### Plugin Methods (8)
13. `registerPlugin` — Register current WebSocket connection as a plugin
14. `getPlugins` — List all registered plugins
15. `subscribePluginsList` — Subscribe to changes in the plugins list
16. `unsubscribePluginsList` — Unsubscribe from plugins list changes
17. `removePlugin` — Remove a plugin from the list
18. `sendCustomEvent` — Send a custom event to all connected WebSockets
19. `ackCustomEvent` — Acknowledge receiving a custom event
20. `getPendingAcks` — Get list of events not yet acknowledged

## Planned Event Types (11 total)

### Chat Events (3)
1. `chatCreated` — Emitted when a new chat is created
2. `chatUpdated` — Emitted when a chat is updated
3. `chatDeleted` — Emitted when a chat is deleted

### Message Events (3)
4. `messageAdded` — Emitted when a message is added
5. `messageUpdated` — Emitted when a message is edited
6. `messageDeleted` — Emitted when a message is deleted

### Plugin Events (5)
7. `pluginRegistered` — Emitted when a plugin is registered or becomes active
8. `pluginRemoved` — Emitted when a plugin is removed
9. `pluginUpdated` — Emitted when a plugin's active status changes
10. `customEvent` — Broadcast when a custom event is sent
11. `customEventAcknowledged` — Sent when a plugin acknowledges a custom event

## Configuration

The server will be configured via command-line arguments:

```bash
rhd_chat_server --port 8080 --db-path ./rhd_db/chats.db
```

**Options**:
- `--port` — WebSocket server port (default: 8080)
- `--db-path` — Path to SQLite database (default: `./rhd_db/chats.db`)
- `--host` — Host to bind to (default: 127.0.0.1)

## Next Steps

1. Create the `rhd_chat_server` package structure
2. Implement database schema extensions (tags, plugins, custom events)
3. Implement WebSocket server core
4. Implement request handlers for all 20 methods
5. Implement subscription system for all 11 event types
6. Implement plugin management system
7. Integrate all components and add tests

## Related Plans

- [`plans/rhd-chat-server-implementation.md`](../rhd-chat-server-implementation.md) — Server implementation plan
- [`plans/rhd-chat-api-implementation.md`](../rhd-chat-api-implementation.md) — API types implementation plan
- [`plans/rhd-chat-api-plugin-methods-implementation.md`](../rhd-chat-api-plugin-methods-implementation.md) — Plugin methods implementation plan
- [`plans/state/rhd-chat-api-state.md`](./rhd-chat-api-state.md) — API current state
