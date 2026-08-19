# Frontend E2E Stream Finish Fix Plan

## Problem Summary

All 9 failing frontend E2E tests share a common root cause: **tests timeout waiting for `isStreaming` to become `false`**. The streaming response never completes because the `chatStreamFinished` event is not being received by the frontend.

**Affected Tests:**
- `chat-state.test.ts`: 4 failures (lines 159, 193, 311, 633)
- `chat-mcp-tools.test.ts`: 3 failures (lines 153, 249, 324)
- `roles-integration.test.ts`: 2 failures (lines 143, 295)

## Root Cause Analysis

### Event Flow Architecture

```
Mock Server (SSE) → Backend (rhd_chat) → ChatEvent::StreamFinished 
    → WebSocket Event (chatStreamFinished) → Frontend (ws.ts) 
    → Zod Validation → dispatch(chatStreamFinished) → isStreaming = false
```

### Identified Issues

#### Issue 1: Schema Mismatch (Low Priority)
**Location:** `frontend/src/lib/types/ws.ts:55-62`

Backend sends `finish_reason` field in `ChatStreamFinishedEvent`:
```rust
// packages/rhd_api/src/chat.rs:23-27
pub struct ChatStreamFinishedEvent {
    pub chat_id: i64,
    pub message_id: i64,
    pub finish_reason: String,  // ← Backend sends this
}
```

Frontend schema doesn't expect it:
```typescript
// frontend/src/lib/types/ws.ts:55-62
export const ChatStreamFinishedEventSchema = z.object({
  type: z.literal('event'),
  event: z.literal('chatStreamFinished'),
  data: z.object({
    messageId: z.number(),
    chatId: z.number(),
    // finishReason is missing
  }),
});
```

**Impact:** Zod's `z.object()` strips unknown keys by default, so this shouldn't cause validation failure. However, it's a schema inconsistency that should be fixed for clarity.

#### Issue 2: Missing Debug Logging (High Priority)
**Location:** Multiple files

No debug logging exists to trace where the event is being dropped:
- Mock server doesn't log when SSE events are sent
- Backend doesn't log when `StreamFinished` event is sent
- Frontend doesn't log when WebSocket messages are received
- WebSocket proxy doesn't log event forwarding

#### Issue 3: Test Setup Inconsistency (Medium Priority)
**Location:** `frontend/src/tests/e2e/*.test.ts`

Tests call `configureMock()` which sets `auto_stream = true`, but don't explicitly call `setAutoStream(true)`. While this should work, it's not explicit and could be a source of confusion.

## Investigation Plan

### Phase 1: Add Comprehensive Debug Logging

Add `DBG:` prefixed logging at each stage of the event flow to identify where the event is being dropped.

#### 1.1 Mock Server Logging
**File:** `packages/rhd_test/src/mock_server/handlers.rs`

Add logging when SSE events are sent:
```rust
// Line 131-135 (auto-stream mode)
eprintln!("DBG: mock_server sending auto-stream events, content={}", content);
let events: Vec<Result<Event, Infallible>> = vec![
    Ok(Event::default().data(format!(r#"{{"choices":[{{"delta":{{"content":"{}"}},"finish_reason":null}}]}}"#, content))),
    Ok(Event::default().data(r#"{"choices":[{"delta":{"content":null},"finish_reason":"stop"}]}"#)),
    Ok(Event::default().data("[DONE]")),
];
eprintln!("DBG: mock_server sent {} events", events.len());
```

#### 1.2 Backend Event Logging
**File:** `packages/rhd_chat/src/stream/send/utils.rs`

Add logging when `StreamFinished` event is sent:
```rust
// Line 86-90
eprintln!("DBG: backend sending StreamFinished event, chat_id={}, message_id={}, finish_reason={}", 
    chat_id, assistant_message_id, finish_reason);
let _ = event_sender.send(ChatEvent::StreamFinished {
    chat_id,
    message_id: assistant_message_id,
    finish_reason,
});
```

#### 1.3 WebSocket Event Conversion Logging
**File:** `packages/rhd_app/src/ws/events.rs`

Add logging when converting `ChatEvent` to `WsEvent`:
```rust
// Line 27-29
ChatEvent::StreamFinished { chat_id, message_id, finish_reason } => {
    eprintln!("DBG: ws converting StreamFinished to chatStreamFinished, chat_id={}, message_id={}", 
        chat_id, message_id);
    let payload = ChatStreamFinishedEvent { chat_id, message_id, finish_reason };
    Some(WsEvent::new("chatStreamFinished", serde_json::to_value(&payload).ok()?))
}
```

#### 1.4 Frontend WebSocket Logging
**File:** `frontend/src/lib/ws.ts`

Add logging when WebSocket messages are received:
```typescript
// Line 50-58
socket.onmessage = (event: MessageEvent) => {
  const raw = JSON.parse(event.data);
  console.log('DBG: ws received message:', JSON.stringify(raw));
  const result = WsMessageSchema.safeParse(raw);

  if (!result.success) {
    console.error('DBG: Invalid WebSocket message:', JSON.stringify(raw, null, 2));
    console.error('DBG: Validation errors:', JSON.stringify(result.error.issues, null, 2));
    return;
  }
  console.log('DBG: ws message validated, type=', result.data.type, 'event=', result.data.event);
  // ...
};
```

#### 1.5 Frontend Event Handler Logging
**File:** `frontend/src/lib/ws.ts`

Add logging in `handleEvent`:
```typescript
// Line 93-99
function handleEvent(message: WsEvent): void {
  const { event, data } = message;
  console.log('DBG: handleEvent called, event=', event, 'data=', data);

  if (event.startsWith('chat') || event.startsWith('project') || event === 'streamAborted' || event === 'messageQueued') {
    console.log('DBG: dispatching chat event:', event);
    dispatch({ type: event, payload: data } as ChatAction);
    return;
  }
  // ...
}
```

### Phase 2: Run Tests and Analyze Output

Run a single failing test with debug logging enabled:
```bash
mise run test-frontend-e2e -- --reporter=verbose --testNamePattern="sends message and receives streaming response"
```

Analyze the debug output to identify where the event flow breaks:
1. Does mock server send SSE events?
2. Does backend receive SSE events and send `StreamFinished`?
3. Does WebSocket conversion happen?
4. Does frontend receive WebSocket message?
5. Does message pass Zod validation?
6. Does event get dispatched?

### Phase 3: Fix Identified Issues

Based on the debug output, apply the appropriate fix:

#### Fix A: If Mock Server Not Sending Events
- Verify `auto_stream` is set correctly
- Check SSE event formatting
- Ensure mock server is receiving requests

#### Fix B: If Backend Not Sending StreamFinished
- Check AI client stream processing
- Verify event sender channel is not closed
- Check for errors during stream processing

#### Fix C: If Frontend Not Receiving Events
- Verify WebSocket connection state
- Check WebSocket proxy forwarding
- Ensure event is not being filtered

#### Fix D: If Zod Validation Failing
- Update `ChatStreamFinishedEventSchema` to accept `finishReason`:
```typescript
export const ChatStreamFinishedEventSchema = z.object({
  type: z.literal('event'),
  event: z.literal('chatStreamFinished'),
  data: z.object({
    messageId: z.number(),
    chatId: z.number(),
    finishReason: z.string().optional(),
  }),
});
```

### Phase 4: Clean Up Debug Logging

After identifying and fixing the root cause:
```bash
# Find all debug logs
rg "DBG:"

# Remove all debug logs
# (manually or with sed/ripgrep)
```

### Phase 5: Verify Fix

Run all frontend E2E tests to verify the fix:
```bash
mise run test-frontend-e2e
```

Expected result: All 25 tests pass (9 previously failing + 15 passing + 1 skipped).

## Implementation Checklist

- [ ] Add debug logging to mock server (`handlers.rs`)
- [ ] Add debug logging to backend stream processing (`utils.rs`)
- [ ] Add debug logging to WebSocket event conversion (`events.rs`)
- [ ] Add debug logging to frontend WebSocket handler (`ws.ts`)
- [ ] Add debug logging to frontend event handler (`ws.ts`)
- [ ] Run single failing test and analyze debug output
- [ ] Identify root cause from debug output
- [ ] Apply appropriate fix based on root cause
- [ ] Remove all debug logging
- [ ] Run all frontend E2E tests to verify fix
- [ ] Update schema to include `finishReason` (optional, for consistency)

## Risk Assessment

**Low Risk:**
- Adding debug logging (temporary, easily removable)
- Updating schema to include optional `finishReason`

**Medium Risk:**
- Modifying mock server behavior
- Changing WebSocket event flow

**High Risk:**
- Modifying backend stream processing logic
- Changing event sender/receiver channels

## Success Criteria

1. All 9 previously failing tests pass
2. No regression in 15 previously passing tests
3. Debug logging removed from codebase
4. Root cause documented in this plan

## Timeline

This is a debugging task with unknown duration. The investigation phase (Phase 1-2) should be completed first to identify the root cause before committing to a fix timeline.
