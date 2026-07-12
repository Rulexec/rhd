# Phase 3: Chat-Project Integration (Backend)

## Goal
Attach projects to chats, manage system prompts, track state.

## Current State Analysis
- [`ChatDb`](packages/rhd_db/src/chat_db.rs) has `chats` and `messages` tables with foreign key relationship
- [`ChatManager`](packages/rhd_app/src/chat.rs:38) holds `Arc<ChatDb>` and `active_streams`
- [`send_message()`](packages/rhd_app/src/chat.rs:75) builds message history from DB, calls AI
- Database migration pattern: check column existence, add if missing

## Subtasks

### 3.1. Extend Chat DB schema
**File**: [`packages/rhd_db/src/chat_db.rs`](packages/rhd_db/src/chat_db.rs)

**New table**:
```sql
CREATE TABLE chat_projects (
    chat_id INTEGER NOT NULL,
    project_name TEXT NOT NULL,
    system_prompt_added BOOLEAN NOT NULL DEFAULT 0,
    attached_at TEXT NOT NULL,
    PRIMARY KEY (chat_id, project_name),
    FOREIGN KEY (chat_id) REFERENCES chats(id) ON DELETE CASCADE
);
```

**New methods**:
```rust
pub fn attach_project(&self, chat_id: i64, project_name: &str) -> Result<(), DbError>
pub fn detach_project(&self, chat_id: i64, project_name: &str) -> Result<(), DbError>
pub fn get_chat_projects(&self, chat_id: i64) -> Result<Vec<(String, bool)>, DbError>  // (name, system_prompt_added)
pub fn mark_system_prompt_added(&self, chat_id: i64, project_name: &str) -> Result<(), DbError>
```

**Migration**:
- Check if `chat_projects` table exists
- If not, create it
- Existing data preserved

**Test**: Attach/detach project, get chat projects, cascade delete

### 3.2. Extend ChatManager
**File**: [`packages/rhd_app/src/chat.rs`](packages/rhd_app/src/chat.rs)

**New methods**:
```rust
pub async fn attach_project(
    &self,
    chat_id: i64,
    project_name: &str,
    project_manager: &ProjectManager,
    mcp_cache: &McpServerCache,
    event_sender: broadcast::Sender<ChatEvent>,
) -> Result<(), ChatError>
```
- Validate chat exists
- Validate project exists in ProjectManager
- Spawn MCP servers if not already running (call `project_manager.spawn_project_mcp()`)
- Add to DB via `db.attach_project()`
- Emit `ProjectAttached` event

```rust
pub async fn detach_project(
    &self,
    chat_id: i64,
    project_name: &str,
    event_sender: broadcast::Sender<ChatEvent>,
) -> Result<(), ChatError>
```
- Remove from DB via `db.detach_project()`
- Emit `ProjectDetached` event
- MCP servers stay alive (not killed on detach)

```rust
pub fn get_chat_projects(&self, chat_id: i64) -> Result<Vec<ProjectInfo>, ChatError>
```
- Get projects from DB
- Return with metadata

**Modify `send_message()`**:
- Before building message history:
  - Get attached projects from DB
  - For each project with `system_prompt_added = false`:
    - Get project's system prompt from ProjectManager
    - Insert as system message at beginning of history
    - Mark as added in DB
  - Check all attached projects' MCP servers are connected
  - If any MCP not connected, return error
- Build message history with system prompts included

**Design decision**: System prompts injected as system messages, not stored in messages table. This keeps message history clean and allows re-injection if needed.

### 3.3. Add WebSocket API for chat-project operations
**File**: [`packages/rhd_api/src/lib.rs`](packages/rhd_api/src/lib.rs:150)

**New WsRequest variants**:
```rust
#[serde(rename = "attachProject", rename_all = "camelCase")]
AttachProject { id: String, chat_id: i64, project_name: String },

#[serde(rename = "detachProject", rename_all = "camelCase")]
DetachProject { id: String, chat_id: i64, project_name: String },

#[serde(rename = "getChatProjects", rename_all = "camelCase")]
GetChatProjects { id: String, chat_id: i64 },
```

**File**: [`packages/rhd_app/src/ws.rs`](packages/rhd_app/src/ws.rs)

**Handler logic**:
- `AttachProject` → call `chat_manager.attach_project()`, return success
- `DetachProject` → call `chat_manager.detach_project()`, return success
- `GetChatProjects` → call `chat_manager.get_chat_projects()`, return project list

### 3.4. Add chat events for project changes
**File**: [`packages/rhd_app/src/chat.rs`](packages/rhd_app/src/chat.rs:30)

**New ChatEvent variants**:
```rust
ProjectAttached { chat_id: i64, project_name: String },
ProjectDetached { chat_id: i64, project_name: String },
```

**Emit events**:
- In `attach_project()` after DB insert
- In `detach_project()` after DB delete

**WebSocket handler**:
- Convert to `WsEvent` with payload:
  ```rust
  ProjectAttachedEvent { chat_id: i64, project_name: String }
  ProjectDetachedEvent { chat_id: i64, project_name: String }
  ```

## Deliverables
- [ ] DB schema: `chat_projects` table in [`ChatDb`](packages/rhd_db/src/chat_db.rs)
- [ ] `attach_project()` / `detach_project()` methods in [`ChatManager`](packages/rhd_app/src/chat.rs:38)
- [ ] System prompt injection logic in `send_message()`
- [ ] WebSocket API: `attachProject`, `detachProject`, `getChatProjects`
- [ ] WebSocket events: `projectAttached`, `projectDetached`
- [ ] Unit tests for DB operations
- [ ] Unit tests for ChatManager project methods

## Dependencies
- Phase 1 (Project data model) must be complete
- Phase 2 (ProjectManager) must be complete

## Risk Assessment
- **Medium risk**: System prompt injection timing - must happen before first message
- **Unknown**: How to handle project detachment mid-conversation
- **Mitigation**: System prompts tracked per-project per-chat, detachment doesn't remove already-injected prompts
