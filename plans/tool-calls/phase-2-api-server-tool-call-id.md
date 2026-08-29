# Phase 2: `toolCallId` Through the API + Server (milestone P2 + P3)

## Overview

Carry `tool_call_id` end-to-end through the WebSocket layer:

1. **API types** (`rhd_chat_api`): `Message.tool_call_id` (wire name `toolCallId`) and
   optional `toolCallId` params on `addMessage` / `addQueueMessage`.
2. **Server** (`rhd_chat_server`): all three `rhd_db::Message → rhd_chat_api::Message`
   converters pass the field through; `add_message` / `add_queue_message` handlers forward
   it to the DB; **`role: "tool"` without `toolCallId` is rejected as `invalid_request`** —
   fail fast at the boundary rather than storing an unusable row.

Wire compatibility: the field is `Option` + skipped when absent, so existing clients and
stored rows are unaffected.

## Dependencies

- **Requires phase-1** (`rhd_db::Message.tool_call_id` + the new trailing DB parameters).
- **Blocks phase-4** (resolution matches on `Message.tool_call_id`) and **phase-5**
  (converter reads `m.tool_call_id`).

## Files to Modify

### A. API types — `packages/rhd_chat_api`

#### 1. `packages/rhd_chat_api/src/common.rs`

**Modify `Message`** (line 62) — add the field after `content`:

```rust
pub struct Message {
    /// Unique identifier for the message.
    pub id: i64,
    /// ID of the chat this message belongs to.
    pub chat_id: i64,
    /// Role of the message author (e.g., "user", "assistant").
    pub role: String,
    /// Main content of the message.
    pub content: String,
    /// For `tool`-role messages: the id of the assistant tool call this message answers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// Timestamp when the message was created.
    pub created_at: DateTime<Utc>,
    // ... unchanged fields ...
}
```

**Update the doc `# Example JSON`** above the struct to include `"toolCallId": "call_abc123"`
in the example (keep it on a tool-role example or note it applies to `tool` messages).

**Update the two test literals** (`test_message_serialization` line 233,
`test_message_with_reasoning_content` line 262): add `tool_call_id: None,`.

**Add serialization tests:**

```rust
#[test]
fn test_message_tool_call_id_serialization() {
    let message = Message {
        id: 456,
        chat_id: 123,
        role: "tool".to_string(),
        content: "Sunny, 22C".to_string(),
        tool_call_id: Some("call_abc123".to_string()),
        created_at: "2026-08-20T18:00:00Z".parse().unwrap(),
        reasoning_content: None,
        tags: vec![],
        is_finished: true,
        is_streaming: false,
        tool_calls: vec![],
    };

    let json = serde_json::to_string(&message).unwrap();
    assert!(json.contains("\"toolCallId\":\"call_abc123\""));

    let deserialized: Message = serde_json::from_str(&json).unwrap();
    assert_eq!(message, deserialized);
}

#[test]
fn test_message_omits_tool_call_id_when_none() {
    let json = r#"{"id":1,"chatId":1,"role":"user","content":"Hi","createdAt":"2026-08-20T18:00:00Z","isFinished":true,"isStreaming":false}"#;
    let message: Message = serde_json::from_str(json).unwrap();
    assert_eq!(message.tool_call_id, None);

    let out = serde_json::to_string(&message).unwrap();
    assert!(!out.contains("toolCallId"));
}
```

#### 2. `packages/rhd_chat_api/src/methods/add_message.rs`

**Modify `AddMessageParams`** (line 25) — add after `content`:

```rust
    /// For `tool`-role messages: the id of the assistant tool call being answered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
```

Update the doc example JSON to show `"toolCallId": "call_abc123"`. Update the existing
test literal (line 67) with `tool_call_id: None,` and extend
`test_add_message_params_optional_fields` with `assert!(params.tool_call_id.is_none());`.
Add a test that `{"chatId":1,"role":"tool","content":"ok","toolCallId":"call_1"}`
deserializes with `tool_call_id == Some("call_1")`.

#### 3. `packages/rhd_chat_api/src/methods/add_queue_message.rs`

Same change on `AddQueueMessageParams` (line 21): optional `tool_call_id` after `content`,
doc example updated, test literal (line 57) gets `tool_call_id: None,`, optional-fields
assertion extended.

#### 4. Remaining `rhd_chat_api::Message {` literals (compile fixes in tests)

Add `tool_call_id: None,` to each:

- `packages/rhd_chat_api/src/events/message_added.rs:45`
- `packages/rhd_chat_api/src/events/message_updated.rs:46`
- `packages/rhd_chat_api/src/events/queue_message_added.rs:43`
- `packages/rhd_chat_api/src/events/queue_message_updated.rs:43`
- `packages/rhd_chat_api/src/methods/get_queue_messages.rs:66`

### B. Server — `packages/rhd_chat_server`

#### 5. `packages/rhd_chat_server/src/handlers/message.rs`

**`convert_message_to_api`** (line 19) — pass through:

```rust
    Ok(rhd_chat_api::Message {
        id: msg.id,
        chat_id: msg.chat_id,
        role: msg.role,
        content: msg.content,
        tool_call_id: msg.tool_call_id,
        created_at,
        // ... unchanged fields ...
    })
```

**`add_message` handler** — insert validation after the chat-exists check and before the DB
call, then replace the phase-1 `None` placeholder with the real pass-through:

```rust
    // A tool message is unusable without the id of the call it answers — reject at the boundary.
    if params.role == "tool" && params.tool_call_id.is_none() {
        return Ok(serde_json::to_value(ErrorResponse::invalid_request(
            request_id,
            "role \"tool\" requires toolCallId".to_string(),
        ))?);
    }

    // Add message
    let (message_id, chat_version) = db.add_message(
        params.chat_id,
        &params.role,
        &params.content,
        None, // model
        params.reasoning_content.as_deref(),
        params.is_finished,
        params.is_streaming,
        params.tool_call_id.as_deref(),
    )?;
```

#### 6. `packages/rhd_chat_server/src/handlers/queue_message.rs`

Same two changes: `convert_message_to_api` (line 21) passes `tool_call_id: msg.tool_call_id`;
`add_queue_message` handler validates `role == "tool" && tool_call_id.is_none()` →
`invalid_request`, then forwards `params.tool_call_id.as_deref()` to `db.add_queue_message`.

#### 7. `packages/rhd_chat_server/src/handlers/chat.rs`

`convert_message_to_api` (line 68) — add `tool_call_id: msg.tool_call_id,`.

### C. Compile fixes in other crates (mechanical)

The `rhd_chat_api::Message` and params structs are constructed literally elsewhere; the
compiler will surface each site. Add `tool_call_id: None,` (or `None` for the params
literals) to:

- `plugins/rhd_plugin_ai_completions/src/ai_request.rs` — `create_message` fixture (line 564).
- `plugins/rhd_plugin_ai_completions/src/tool_resolution.rs` — `create_message` fixture (line 117).
- `plugins/rhd_plugin_ai_completions/src/trigger_detection.rs` — `create_message` fixture (line 64).
- `packages/rhd_app/src/commands/queue.rs:17` — `AddQueueMessageParams` literal.
- `packages/rhd_chat_server/tests/websocket_tests.rs` — `AddQueueMessageParams` literals
  (lines 173, 184) and `AddMessageParams` literals (lines 221, 277, 335, 704).
- `plugins/rhd_plugin_ai_completions/tests/integration_test.rs` — `AddQueueMessageParams`
  literals (lines 177, 246, 419, 512, 598).
- `packages/rhd_chat_client/src/lib.rs:27` — the `add_message(AddMessageParams { ... })`
  example in the crate-level doc comment is a **compiled doctest** (```` ```rust,no_run ````):
  add `tool_call_id: None,` or `mise run test-cargo` fails on doctests.

> The plugin fixtures are rewritten in phases 4–5; here we only keep them compiling.

## Tests

### `packages/rhd_chat_server/tests/websocket_tests.rs` — add

```rust
#[tokio::test]
async fn test_add_tool_message_requires_tool_call_id() {
    let (port, _handle) = start_test_server().await;
    let client = connect_client(port).await;

    let chat_id = client
        .create_chat(CreateChatParams { title: "t".into(), tags: vec![] })
        .await
        .unwrap()
        .chat_id;

    // Without toolCallId → invalid_request error
    let err = client
        .add_message(AddMessageParams {
            chat_id,
            role: "tool".to_string(),
            content: "result".to_string(),
            tool_call_id: None,
            reasoning_content: None,
            tags: vec![],
            is_finished: true,
            is_streaming: false,
        })
        .await
        .expect_err("tool message without toolCallId must be rejected");
    assert!(format!("{err:?}").contains("toolCallId") || format!("{err:?}").contains("Invalid"));

    // With toolCallId → stored and returned
    let ok = client
        .add_message(AddMessageParams {
            chat_id,
            role: "tool".to_string(),
            content: "result".to_string(),
            tool_call_id: Some("call_1".to_string()),
            reasoning_content: None,
            tags: vec![],
            is_finished: true,
            is_streaming: false,
        })
        .await
        .unwrap();

    let chat_result = client
        .get_chat(GetChatParams { chat_id, if_version_higher_than: None })
        .await
        .unwrap();
    let tool_msg = chat_result
        .messages
        .iter()
        .find(|m| m.id == ok.message_id)
        .expect("tool message present");
    assert_eq!(tool_msg.tool_call_id.as_deref(), Some("call_1"));
}
```

> Match the exact error-assertion style used by neighboring tests in this file (check how
> existing tests assert `invalid_request` responses — e.g. via `ClientError` variants) and
> reuse the file's existing `start_test_server` / connect helpers.

### Unit tests

- `rhd_chat_api`: the four new serialization/deserialization tests above.
- `rhd_chat_server`: the converter is `fn`-private; the websocket test covers it. If a
  direct converter test is desired, it must go through the handler — keep the websocket test.

## Implementation Notes

1. **`#[serde(default, skip_serializing_if = "Option::is_none")]`** on every new field:
   old clients that never send `toolCallId` keep working, and responses omit the field
   when absent (no wire noise, no frontend breakage — `ChatStore` tolerates extra fields).
2. **Validation lives in the handlers, not the DB** (D1): the DB column stays nullable
   because user/assistant/system messages legitimately have no id; only `tool` messages
   are required to carry one, and that is a protocol rule.
3. **`Eq` derive still holds** on `Message` / params structs — `Option<String>` is `Eq`.
4. **Events carry the field automatically**: `messageAdded` / `messageUpdated` /
   `queueMessage*` embed `rhd_chat_api::Message`, so the id reaches subscribers (the
   tool-executing plugin of the future) with zero extra code.
5. **Do not add `tool_call_id` to `UpdateMessageParams`** — immutability after creation (D1).

## Validation

```bash
mise run check-cargo
mise run test-cargo
```
