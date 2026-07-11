# Fix: Empty messages sent to API when no project attached

## Problem

When sending the first message to a chat without an attached project, the AI API receives `"messages": []`, causing a 400 error:

```
"Role must be in [\"user\", \"assistant\", \"system\", ...] and the role in last message must be in [\"user\", \"function\", \"tool\"]"
```

## Root Cause

In [`stream.rs`](packages/rhd_chat/src/stream.rs:66), the `send_message` function fetches messages from the database **before** the user message is added:

```
Line 66:  let messages = manager.db().get_messages(chat_id)?;  // ← fetches [] (empty)
Line 68:  collect_tools_from_projects(...)                       // ← no project → no tools
Line 71:  tool_defs.is_empty() → true                           // ← skip tool loop
Line 104: manager.db().add_message(chat_id, "user", &content...) // ← user message added HERE
Line 120: build_chat_messages(&messages)                         // ← uses stale empty array!
Line 147: API call with messages: []                             // ← 400 error
```

The tools path (`send_message_with_tools` → `tool_loop`) is unaffected because [`tool_loop()`](packages/rhd_chat/src/tools.rs:93) re-fetches messages from DB inside the loop.

## Fix

Move the `get_messages()` call and user message insertion so that messages are fetched **after** the user message is persisted to the database. This ensures both the API call and logging include the current user message.

### Changes in `packages/rhd_chat/src/stream.rs` — `send_message()` function

**Before** (current order):
1. `get_messages()` → stale
2. `collect_tools_from_projects()`
3. If tools: log with stale messages, call `send_message_with_tools`
4. If no tools: add user message to DB, `build_chat_messages(&messages)` with stale array → **BUG**

**After** (fixed order):
1. Add user message to DB, emit `MessageAdded` event
2. `get_messages()` → now includes user message
3. `collect_tools_from_projects()`
4. If tools: log with correct messages, call `send_message_with_tools`
5. If no tools: `build_chat_messages(&messages)` with correct array → **FIXED**

### Detailed code changes

In `send_message()`, restructure the function body after the pause check (line 52) and model validation (line 54-56):

1. **Move user message creation** (currently lines 104-118) to right after model validation, before `get_messages()`
2. **Move `get_messages()`** (currently line 66) to after user message is added to DB
3. Keep `collect_tools_from_projects()` and the rest of the flow unchanged — they already use `messages` correctly, just need the fresh data

The `inject_system_prompts()` call (line 58-64) should remain before `get_messages()` since system prompt messages also need to be included.

## Verification

- Send first message to a chat without attached project → should succeed
- Send message to a chat with attached project → should still work (tool path re-fetches anyway)
- Check `raw.txt` log → messages array should include the user message
- Edit and resend → should still work (already fetches after DB update)
