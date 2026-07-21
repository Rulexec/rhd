# Plan: Unified Message Addition Point

## Problem Statement

Currently, messages are added to the database via `db.add_message()` and events are emitted separately. This has led to inconsistencies where some places forget to emit the `MessageAdded` event, causing the frontend to not receive real-time updates.

### Current State Analysis

| Location | File | Role | Event Emitted? |
|----------|------|------|----------------|
| `inject_todo_tool_contract()` | stream.rs:75 | system | ❌ **NO** |
| `send_message()` (paused) | stream.rs:99-113 | user | ✅ Yes |
| `send_message()` (normal) | stream.rs:149-163 | user | ✅ Yes |
| `handle_stream_result()` | stream.rs:537-560 | assistant | ✅ Yes |
| `inject_todo_list_message()` (empty) | tools.rs:241 | system | ❌ **NO** |
| `inject_todo_list_message()` (items) | tools.rs:272 | system | ❌ **NO** |
| `tool_loop()` (final assistant) | tools.rs:515-529 | assistant | ✅ Yes |
| `tool_loop()` (intermediate) | tools.rs:583-602 | assistant | ✅ Yes |
| `tool_loop()` (tool result) | tools.rs:648-661 | tool | ✅ Yes |
| `inject_system_prompts()` | projects.rs:187-201 | system | ✅ Yes |
| `inject_roles_prompt()` | projects.rs:252-265 | system | ✅ Yes |
| `inject_role_system_prompt()` | projects.rs:291-304 | system | ✅ Yes |

**3 places are missing event emission** — all in system message injection paths.

## Solution Design

### Approach: Add `add_message_and_notify()` to `ChatManager`

Create a single method on `ChatManager` that:
1. Calls `db.add_message()` to persist the message
2. Constructs the `Message` struct with the returned ID
3. Emits `ChatEvent::MessageAdded` event
4. Returns the `Message` struct (for callers who need the ID or full message)

### Method Signature

```rust
impl<P: ProjectProvider> ChatManager<P> {
    /// Adds a message to the database and emits a MessageAdded event.
    /// This is the single point of message addition to ensure consistency.
    pub fn add_message_and_notify(
        &self,
        chat_id: i64,
        role: &str,
        content: &str,
        model: Option<&str>,
        thinking_content: Option<&str>,
        event_sender: &broadcast::Sender<ChatEvent>,
    ) -> Result<Message, ChatError> {
        let message_id = self.db.add_message(chat_id, role, content, model, thinking_content)?;
        let message = Message {
            id: message_id,
            chat_id,
            role: role.to_string(),
            content: content.to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            model: model.map(|s| s.to_string()),
            thinking_content: thinking_content.map(|s| s.to_string()),
        };
        let _ = event_sender.send(ChatEvent::MessageAdded {
            chat_id,
            message: message.clone(),
        });
        Ok(message)
    }
}
```

## Implementation Steps

### Step 1: Add `add_message_and_notify()` to `ChatManager`

**File:** `packages/rhd_chat/src/manager.rs`

Add the new method to the `ChatManager` impl block.

### Step 2: Update `stream.rs` — `inject_todo_tool_contract()`

**File:** `packages/rhd_chat/src/stream.rs:75`

**Before:**
```rust
manager.db().add_message(chat_id, "system", &contract_content, None, None)?;
```

**After:**
```rust
manager.add_message_and_notify(chat_id, "system", &contract_content, None, None, event_sender)?;
```

### Step 3: Update `stream.rs` — `send_message()` (paused case)

**File:** `packages/rhd_chat/src/stream.rs:99-113`

**Before:**
```rust
let user_message_id = manager.db().add_message(chat_id, "user", &content, Some(model), None)?;
let user_message = Message { /* ... */ };
let _ = event_sender.send(ChatEvent::MessageAdded { chat_id, message: user_message });
```

**After:**
```rust
let user_message = manager.add_message_and_notify(chat_id, "user", &content, Some(model), None, &event_sender)?;
let user_message_id = user_message.id;
```

### Step 4: Update `stream.rs` — `send_message()` (normal case)

**File:** `packages/rhd_chat/src/stream.rs:149-163`

**Before:**
```rust
let user_message_id = manager.db().add_message(chat_id, "user", &content, Some(model), None)?;
let user_message = Message { /* ... */ };
let _ = event_sender.send(ChatEvent::MessageAdded { chat_id, message: user_message });
```

**After:**
```rust
let user_message = manager.add_message_and_notify(chat_id, "user", &content, Some(model), None, &event_sender)?;
let user_message_id = user_message.id;
```

### Step 5: Update `stream.rs` — `handle_stream_result()`

**File:** `packages/rhd_chat/src/stream.rs:537-560`

**Before:**
```rust
let assistant_message_id = manager.db().add_message(chat_id, "assistant", &full_content, Some(model), thinking_option)?;
let assistant_message = Message { /* ... */ };
let _ = event_sender.send(ChatEvent::MessageAdded { chat_id, message: assistant_message });
```

**After:**
```rust
let assistant_message = manager.add_message_and_notify(chat_id, "assistant", &full_content, Some(model), thinking_option.as_deref(), event_sender)?;
let assistant_message_id = assistant_message.id;
```

### Step 6: Update `tools.rs` — `inject_todo_list_message()`

**File:** `packages/rhd_chat/src/tools.rs:222-275`

**Changes:**
1. Add `event_sender: &broadcast::Sender<ChatEvent>` parameter to function signature
2. Replace both `manager.db().add_message()` calls with `manager.add_message_and_notify()`

**Before (line 241):**
```rust
manager.db().add_message(chat_id, "system", &empty_prompt, None, None)?;
```

**After:**
```rust
manager.add_message_and_notify(chat_id, "system", &empty_prompt, None, None, event_sender)?;
```

**Before (line 272):**
```rust
manager.db().add_message(chat_id, "system", &environment_content, None, None)?;
```

**After:**
```rust
manager.add_message_and_notify(chat_id, "system", &environment_content, None, None, event_sender)?;
```

### Step 7: Update `tools.rs` — `tool_loop()` call to `inject_todo_list_message()`

**File:** `packages/rhd_chat/src/tools.rs:665`

**Before:**
```rust
if let Err(e) = inject_todo_list_message(manager, chat_id, template_loader) {
```

**After:**
```rust
if let Err(e) = inject_todo_list_message(manager, chat_id, template_loader, event_sender) {
```

### Step 8: Update `tools.rs` — `tool_loop()` assistant messages

**File:** `packages/rhd_chat/src/tools.rs:515-529` (final assistant)

**Before:**
```rust
let assistant_message_id = manager.db().add_message(chat_id, "assistant", &final_content, Some(model), thinking_option.as_deref())?;
let assistant_message = Message { /* ... */ };
let _ = event_sender.send(ChatEvent::MessageAdded { chat_id, message: assistant_message });
```

**After:**
```rust
let assistant_message = manager.add_message_and_notify(chat_id, "assistant", &final_content, Some(model), thinking_option.as_deref(), event_sender)?;
let assistant_message_id = assistant_message.id;
```

**File:** `packages/rhd_chat/src/tools.rs:583-602` (intermediate assistant)

**Before:**
```rust
let intermediate_msg_id = manager.db().add_message(chat_id, "assistant", &assistant_msg_content, Some(model), thinking_option)?;
let intermediate_message = Message { /* ... */ };
let _ = event_sender.send(ChatEvent::MessageAdded { chat_id, message: intermediate_message });
```

**After:**
```rust
let intermediate_message = manager.add_message_and_notify(chat_id, "assistant", &assistant_msg_content, Some(model), thinking_option.as_deref(), event_sender)?;
let intermediate_msg_id = intermediate_message.id;
```

**File:** `packages/rhd_chat/src/tools.rs:648-661` (tool result)

**Before:**
```rust
let tool_msg_id = manager.db().add_message(chat_id, "tool", &tool_result_json, None, None)?;
let tool_message = Message { /* ... */ };
let _ = event_sender.send(ChatEvent::MessageAdded { chat_id, message: tool_message });
```

**After:**
```rust
let tool_message = manager.add_message_and_notify(chat_id, "tool", &tool_result_json, None, None, event_sender)?;
let tool_msg_id = tool_message.id;
```

### Step 9: Refactor `projects.rs` to use `ChatManager`

**File:** `packages/rhd_chat/src/projects.rs`

Change function signatures to accept `&ChatManager<P>` instead of separate `db` and `project_provider`:

#### 9a: `inject_system_prompts()` (lines 167-207)

**Before:**
```rust
pub async fn inject_system_prompts<P: ProjectProvider>(
    db: &Arc<ChatDb>,
    project_provider: &Arc<P>,
    chat_id: i64,
    event_sender: &broadcast::Sender<ChatEvent>,
) -> Result<(), ChatError> {
    // ...
    let system_message_id = db.add_message(chat_id, "system", &system_prompt, None, None)?;
    let system_message = rhd_db::Message { /* ... */ };
    let _ = event_sender.send(ChatEvent::MessageAdded { chat_id, message: system_message });
}
```

**After:**
```rust
pub async fn inject_system_prompts<P: ProjectProvider>(
    manager: &ChatManager<P>,
    chat_id: i64,
    event_sender: &broadcast::Sender<ChatEvent>,
) -> Result<(), ChatError> {
    let db = manager.db();
    let project_provider = manager.project_provider();
    // ...
    manager.add_message_and_notify(chat_id, "system", &system_prompt, None, None, event_sender)?;
}
```

#### 9b: `inject_roles_prompt()` (lines 209-270)

**Before:**
```rust
pub async fn inject_roles_prompt<P: ProjectProvider>(
    db: &Arc<ChatDb>,
    project_provider: &Arc<P>,
    chat_id: i64,
    event_sender: &broadcast::Sender<ChatEvent>,
) -> Result<(), ChatError> {
    // ...
    let system_message_id = db.add_message(chat_id, "system", &prompt, None, None)?;
    let system_message = rhd_db::Message { /* ... */ };
    let _ = event_sender.send(ChatEvent::MessageAdded { chat_id, message: system_message });
}
```

**After:**
```rust
pub async fn inject_roles_prompt<P: ProjectProvider>(
    manager: &ChatManager<P>,
    chat_id: i64,
    event_sender: &broadcast::Sender<ChatEvent>,
) -> Result<(), ChatError> {
    let db = manager.db();
    let project_provider = manager.project_provider();
    // ...
    manager.add_message_and_notify(chat_id, "system", &prompt, None, None, event_sender)?;
}
```

#### 9c: `inject_role_system_prompt()` (lines 272-307)

**Before:**
```rust
pub fn inject_role_system_prompt<P: ProjectProvider>(
    db: &Arc<ChatDb>,
    project_provider: &Arc<P>,
    chat_id: i64,
    project_name: &str,
    role_name: &str,
    event_sender: &broadcast::Sender<ChatEvent>,
) -> Result<(), ChatError> {
    // ...
    let system_message_id = db.add_message(chat_id, "system", &prompt, None, None)?;
    let system_message = rhd_db::Message { /* ... */ };
    let _ = event_sender.send(ChatEvent::MessageAdded { chat_id, message: system_message });
}
```

**After:**
```rust
pub fn inject_role_system_prompt<P: ProjectProvider>(
    manager: &ChatManager<P>,
    chat_id: i64,
    project_name: &str,
    role_name: &str,
    event_sender: &broadcast::Sender<ChatEvent>,
) -> Result<(), ChatError> {
    let db = manager.db();
    let project_provider = manager.project_provider();
    // ...
    manager.add_message_and_notify(chat_id, "system", &prompt, None, None, event_sender)?;
}
```

#### 9d: `inject_pending_role_prompt()` (lines 309-334)

Update to pass `manager` instead of `db` and `project_provider`:

**Before:**
```rust
pub fn inject_pending_role_prompt<P: ProjectProvider>(
    db: &Arc<ChatDb>,
    project_provider: &Arc<P>,
    chat_id: i64,
    event_sender: &broadcast::Sender<ChatEvent>,
) -> Result<(), ChatError> {
    // ...
    inject_role_system_prompt(db, project_provider, chat_id, &project_name, &role_name, event_sender)?;
}
```

**After:**
```rust
pub fn inject_pending_role_prompt<P: ProjectProvider>(
    manager: &ChatManager<P>,
    chat_id: i64,
    event_sender: &broadcast::Sender<ChatEvent>,
) -> Result<(), ChatError> {
    let db = manager.db();
    // ...
    inject_role_system_prompt(manager, chat_id, &project_name, &role_name, event_sender)?;
}
```

### Step 10: Update call sites in `stream.rs` for projects.rs functions

**File:** `packages/rhd_chat/src/stream.rs`

Update all call sites to pass `manager` instead of `manager.db()` and `manager.project_provider()`:

**Before (lines 124-145):**
```rust
projects::inject_system_prompts(
    manager.db(),
    manager.project_provider(),
    chat_id,
    &event_sender,
)
.await?;

projects::inject_roles_prompt(
    manager.db(),
    manager.project_provider(),
    chat_id,
    &event_sender,
)
.await?;

projects::inject_pending_role_prompt(
    manager.db(),
    manager.project_provider(),
    chat_id,
    &event_sender,
)?;
```

**After:**
```rust
projects::inject_system_prompts(manager, chat_id, &event_sender).await?;
projects::inject_roles_prompt(manager, chat_id, &event_sender).await?;
projects::inject_pending_role_prompt(manager, chat_id, &event_sender)?;
```

Same changes needed in `edit_and_resend()` (lines 352-373).

### Step 11: Update Tests

Update tests in `tools.rs`, `stream.rs`, and `projects.rs` that call the refactored functions to pass the correct parameters.

## Files to Modify

1. `packages/rhd_chat/src/manager.rs` — Add `add_message_and_notify()` method
2. `packages/rhd_chat/src/stream.rs` — Update 4 call sites + 6 project function calls
3. `packages/rhd_chat/src/tools.rs` — Update 5 call sites + function signature
4. `packages/rhd_chat/src/projects.rs` — Refactor 4 functions to use `ChatManager`

## Benefits

1. **Single source of truth** — All message addition goes through one method
2. **Impossible to forget events** — The method always emits the event
3. **Less code duplication** — No more repetitive `Message` struct construction
4. **Easier to maintain** — Changes to message addition logic only need to happen in one place
5. **Type safety** — Returns a `Message` struct, not just an ID
6. **Full consistency** — All modules use the same pattern

## Testing Strategy

1. Run existing tests to ensure no regressions
2. Add a test that verifies `inject_todo_list_message()` emits `MessageAdded` events
3. Verify frontend receives system messages in real-time without page reload
