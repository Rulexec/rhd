# Chat Versioning for Consistency and Race-Free Operations

## Problem Statement

The current chat system has race conditions in state management:
1. **Event timing issues**: Between subscribing to a chat and fetching its state, changes can be missed
2. **Plugin state staleness**: Plugins like `rhd_plugin_ai_completions` may operate on stale message state
3. **No consistency validation**: Events don't carry version information, making it impossible to detect missed updates

## Solution Overview

Implement per-chat versioning where:
- Each chat has a monotonically increasing version number
- Version increments on ANY change (messages, queue, tags, metadata)
- Events carry the new version number
- Clients validate event versions against their local state
- Getters support conditional fetching based on version

## Architecture

### Version Lifecycle

```
┌─────────────────────────────────────────────────────────────┐
│ Chat Creation                                                │
│ - Initial version: 1                                         │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│ Any Mutation (message add/update/delete, tag change, etc.)  │
│ - version = version + 1                                      │
│ - Event includes new version                                 │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│ Client Receives Event                                        │
│ - If event.version == local.version + 1: apply event        │
│ - Else: ignore event, refetch state with getChat            │
└─────────────────────────────────────────────────────────────┘
```

### Version Validation Flow

```
┌─────────────────────────────────────────────────────────────┐
│ getChat(chatId, ifVersionHigherThan?)                        │
├─────────────────────────────────────────────────────────────┤
│ if ifVersionHigherThan provided:                             │
│   - if current_version > ifVersionHigherThan: return state  │
│   - if current_version == ifVersionHigherThan: return "actual"│
│   - if current_version < ifVersionHigherThan: return error  │
│ else:                                                        │
│   - return state with current_version                        │
└─────────────────────────────────────────────────────────────┘
```

## Implementation Plan

### Phase 1: Database Schema Changes

**File**: `packages/rhd_db/src/chat_db/schema.rs`

1. Add `version` column to `chats` table:
   ```sql
   ALTER TABLE chats ADD COLUMN version INTEGER NOT NULL DEFAULT 1;
   ```

2. Update migration logic to handle existing databases

**Files to modify**:
- `packages/rhd_db/src/chat_db/schema.rs` - Add version column
- `packages/rhd_db/src/chat_db/chats.rs` - Update Chat struct and queries

### Phase 2: Database Operations

**Files to modify**:
- `packages/rhd_db/src/chat_db/chats.rs`
- `packages/rhd_db/src/chat_db/messages.rs`
- `packages/rhd_db/src/chat_db/messages_queue.rs`
- `packages/rhd_db/src/chat_db/tags.rs`
- `packages/rhd_db/src/chat_db/plugins.rs`
- `packages/rhd_db/src/chat_db/tools.rs`

**Changes**:
1. All mutation operations must increment chat version atomically
2. Add `get_chat_with_version()` method
3. Add `get_chat_if_version_higher()` method with three-state result:
   - `NewerVersion { chat, version }` - state is newer than requested
   - `Actual` - state matches requested version
   - `OlderVersion` - error case (should not happen)

### Phase 3: API Types

**Files to modify**:
- `packages/rhd_chat_api/src/common.rs` - Add version to Chat struct
- `packages/rhd_chat_api/src/methods/get_chat.rs` - Add version field and conditional parameter
- `packages/rhd_chat_api/src/events/*.rs` - Add version field to all chat-related events

**Event changes**:
```rust
// All chat events get version field
pub struct MessageAddedData {
    pub chat_id: i64,
    pub message: Message,
    pub chat_version: i64,  // NEW
}

pub struct ChatUpdatedData {
    pub chat: ChatSummary,
    pub chat_version: i64,  // NEW
}
```

**GetChat changes**:
```rust
pub struct GetChatParams {
    pub chat_id: i64,
    pub if_version_higher_than: Option<i64>,  // NEW
}

pub struct GetChatResult {
    pub chat: Chat,
    pub messages: Vec<Message>,
    pub queued_messages_count: i64,
    pub version: i64,  // NEW
    pub status: GetChatStatus,  // NEW: NewerVersion | Actual
}
```

### Phase 4: Server Implementation

**Files to modify**:
- `packages/rhd_chat_server/src/handlers/chat.rs`
- `packages/rhd_chat_server/src/handlers/message.rs`
- `packages/rhd_chat_server/src/handlers/queue_message.rs`
- `packages/rhd_chat_server/src/handlers/plugin.rs`
- `packages/rhd_chat_server/src/handlers/tools.rs`
- `packages/rhd_chat_server/src/events.rs`

**Changes**:
1. All handlers must return new version after mutations
2. Event emission must include new version
3. `get_chat` handler must support conditional fetching

### Phase 5: Client Implementation

**Files to modify**:
- `packages/rhd_chat_client/src/chat_monitor.rs`
- `packages/rhd_chat_client/src/client.rs`

**Changes**:
1. `ChatState` must track version:
   ```rust
   pub struct ChatState {
       pub chat_id: i64,
       pub messages: Vec<Message>,
       pub queued_messages_count: i64,
       pub tags: Vec<String>,
       pub version: i64,  // NEW
   }
   ```

2. Event handlers must validate version:
   ```rust
   // On event received:
   if event.chat_version == state.version + 1 {
       // Apply event
       state.version = event.chat_version;
   } else {
       // Version mismatch - refetch
       let fresh_state = client.get_chat(...).await;
       state = fresh_state;
   }
   ```

3. Add method to check if state is current:
   ```rust
   pub async fn ensure_current(chat_id: i64) -> Result<ChatState, ClientError>
   ```

### Phase 6: Plugin Integration

**File**: `plugins/rhd_plugin_ai_completions/src/plugin.rs`

**Changes**:
1. Before sending AI request, verify state is current:
   ```rust
   // In main loop before handle_ai_request:
   let current_state = chat_monitor.ensure_current(chat_id).await?;
   // Use current_state for AI request
   ```

2. Add version to trigger detection context

## Detailed Changes by File

### Database Layer

#### `packages/rhd_db/src/chat_db/schema.rs`
- Add version column to chats table
- Add migration for existing databases

#### `packages/rhd_db/src/chat_db/chats.rs`
- Update `Chat` struct to include `version: i64`
- Update `create_chat()` to initialize version = 1
- Update `get_chat()` to return version
- Add `increment_chat_version()` helper
- Add `get_chat_if_version_higher()` method

#### `packages/rhd_db/src/chat_db/messages.rs`
- Update `add_message()` to increment chat version
- Update `update_message()` to increment chat version
- Update `delete_message()` to increment chat version

#### `packages/rhd_db/src/chat_db/messages_queue.rs`
- Update all queue operations to increment chat version

#### `packages/rhd_db/src/chat_db/tags.rs`
- Update tag operations to increment chat version

#### `packages/rhd_db/src/chat_db/tools.rs`
- Update tool operations to increment chat version

### API Layer

#### `packages/rhd_chat_api/src/common.rs`
- Add `version: i64` to `Chat` struct
- Add `version: i64` to `ChatSummary` struct

#### `packages/rhd_chat_api/src/methods/get_chat.rs`
- Add `if_version_higher_than: Option<i64>` to `GetChatParams`
- Add `version: i64` to `GetChatResult`
- Add `GetChatStatus` enum

#### `packages/rhd_chat_api/src/events/*.rs`
- Add `chat_version: i64` to all chat-related events:
  - `chat_created.rs`
  - `chat_updated.rs`
  - `chat_deleted.rs`
  - `message_added.rs`
  - `message_updated.rs`
  - `message_deleted.rs`
  - `queue_message_added.rs`
  - `queue_message_updated.rs`
  - `queue_message_deleted.rs`
  - `tools_updated.rs`

### Server Layer

#### `packages/rhd_chat_server/src/handlers/*.rs`
- All mutation handlers must:
  1. Perform database operation
  2. Get new version
  3. Include version in event emission
  4. Return version in response

#### `packages/rhd_chat_server/src/handlers/chat.rs`
- Update `get_chat` handler to support conditional fetching
- Return version in all chat responses

### Client Layer

#### `packages/rhd_chat_client/src/chat_monitor.rs`
- Add `version: i64` to `ChatState`
- Update event handlers to validate versions
- Add `ensure_current()` method
- Implement version-based event filtering

#### `packages/rhd_chat_client/src/client.rs`
- Update `get_chat()` to support new parameters
- Update event types to include version

### Plugin Layer

#### `plugins/rhd_plugin_ai_completions/src/plugin.rs`
- Before AI request, call `ensure_current()` to verify state
- Pass version context to trigger detection

## Testing Strategy

### Unit Tests
1. Database version increment tests
2. Conditional get tests (all three states)
3. Event version validation tests

### Integration Tests
1. Race condition test: subscribe → concurrent modification → verify no missed events
2. Plugin staleness test: verify AI request uses current state
3. Version mismatch recovery test

### Test Files to Update
- `packages/rhd_db/src/chat_db/tests/chat_tests.rs`
- `packages/rhd_db/src/chat_db/tests/message_tests.rs`
- `packages/rhd_chat_server/tests/websocket_tests.rs`
- `plugins/rhd_plugin_ai_completions/tests/integration_test.rs`

## Migration Strategy

1. **Backward Compatibility**: 
   - Version column has default value of 1
   - Existing chats get version = 1 on migration
   - Old clients can ignore version field (JSON deserialization)

2. **Rollout Order**:
   1. Database migration (schema + data)
   2. Server implementation (emit versions)
   3. Client implementation (validate versions)
   4. Plugin integration (ensure current)

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Version overflow | Use i64 (9 quintillion operations) |
| Concurrent increments | SQLite WAL + atomic transactions |
| Client ignores version | Document breaking change, version field is additive |
| Performance impact | Minimal - single integer increment per mutation |

## Success Criteria

1. All chat mutations atomically increment version
2. All events include new version
3. Client detects and recovers from version mismatches
4. Plugin verifies state freshness before AI requests
5. No race conditions in chat state management
6. All existing tests pass
7. New tests cover versioning scenarios
