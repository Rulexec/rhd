# Fix Chat Streaming and E2E Test Plan

## Goal

1. Filter out empty `chatStreamChunk` events on backend (no need to send `content: ""`)
2. Show loader until first chunk arrives, then show assistant message with content + animated "..." indicator
3. Animated dots cycle: "." → ".." → "..." → "." every 200ms while streaming
4. Write e2e test that controls streaming via control server, emits chunks, verifies real-time rendering, finishes message, checks final rendering

## Current Status

### ✅ Completed

1. **Backend: Empty chunk filtering** - Implemented in `packages/rhd_app/src/chat.rs`
   - Both `send_message` and `edit_and_resend` now filter out empty content chunks
   - Only non-empty chunks are emitted as `StreamChunk` events

2. **Frontend: Two-phase streaming display** - Implemented
   - Added `streamingMessageId` store to track optimistic message
   - `chatWs.ts` creates optimistic message on first chunk, updates on subsequent chunks
   - `MessageList.svelte` shows loader only when `isStreaming && !streamingMessageId`
   - `Message.svelte` shows animated dots indicator for streaming messages (CSS animation)
   - `ChatMessage` type updated to support `number | string` IDs

3. **Backend test: Controlled streaming infrastructure** - Implemented
   - Mock server uses `mpsc` channel for controlled streaming
   - Control server endpoints: `/stream-chunk`, `/stream-finish`, `/stream-ready`
   - All endpoints return JSON responses with status checking
   - SSE stream uses `async_stream::stream!` with keep-alive

4. **Frontend test: Test utilities and e2e test** - Implemented
   - `testUtils.ts` has `emitStreamChunk`, `finishStream`, `waitForStreamReady`
   - All control server methods check response status and throw errors
   - `chat-streaming.test.ts` created with full test flow

### ✅ E2E Test Fixed

**Root Cause**: `handle_send_message` was blocking the WebSocket select loop.

The WebSocket handler uses `tokio::select!` to multiplex between:
1. Client messages (read.next())
2. Execution events (events_rx.recv())
3. Chat events (chat_events_rx.recv())

When `handle_send_message` was called, it awaited the entire streaming process synchronously. This blocked the select loop, preventing it from processing chat events from the broadcast channel.

**Fix**: Spawn `send_message` in a separate task so the select loop can continue processing events while streaming.

```rust
// Before (blocking):
async fn handle_send_message(...) -> WsResponse {
    match state.chat_manager.send_message(...).await {
        // ...
    }
}

// After (non-blocking):
async fn handle_send_message(...) -> WsResponse {
    let chat_manager = Arc::clone(&state.chat_manager);
    let models = state.models.clone();
    let event_sender = state.chat_event_sender.clone();
    
    tokio::spawn(async move {
        chat_manager.send_message(...).await;
    });
    
    WsResponse::success(id, serde_json::json!({ "status": "sending" }))
}
```

**Result**: E2E test now passes. Chat events are received and forwarded to frontend while streaming continues.

## Root Cause Analysis

Comparing [`sse_test.rs`](packages/rhd_test/src/sse_test.rs:1) (passes) vs [`mock_server.rs`](packages/rhd_test/src/mock_server.rs:1) (fails):

### Key Differences

1. **Missing keep-alive**: [`mock_server.rs:242`](packages/rhd_test/src/mock_server.rs:242) creates `Sse::new(stream).into_response()` WITHOUT `.keep_alive()`. The tool-call branch at line 176 HAS keep-alive. Without keep-alive, axum may not properly handle the delay between connection and first event.

2. **Two-channel indirection**: mock_server uses control channel → spawned task → event channel → SSE. sse_test directly sends events. The delay waiting for control server chunks may cause connection issues.

3. **Error wrapping is too generic**: [`client.rs:493`](packages/rhd_ai/src/client.rs:493) wraps all stream errors as `AiError::Network` with just `reqwest::Error`. "error decoding response body" doesn't indicate WHERE in the stream pipeline the error occurred.

## Investigation and Fix Strategy

### Phase 0: Modify sse_test to mimic real use-case (isolate SSE issue) ✅ COMPLETED

**Result**: Modified sse_test with two-channel indirection + 2s delay **PASSED**.

**Conclusion**: Two-channel pattern and delay are NOT the issue. The issue is specific to mock_server or e2e test setup.

**Next**: Need detailed error wrapping (Phase 2) and control channel logging (Phase 3) to identify the actual difference between mock_server and sse_test.

### Phase 1: Add keep-alive to mock_server SSE response

**File**: `packages/rhd_test/src/mock_server.rs`

**Change**: Add `.keep_alive(KeepAlive::new().interval(Duration::from_secs(1)))` to line 242:

```rust
// Before:
Sse::new(stream).into_response()

// After:
Sse::new(stream)
    .keep_alive(KeepAlive::new().interval(Duration::from_secs(1)))
    .into_response()
```

**Rationale**: sse_test sends events immediately without delay. mock_server waits for control server chunks, creating a gap where the connection may timeout without keep-alive pings.

### Phase 2: Add detailed error wrapping in rhd_ai client

**File**: `packages/rhd_ai/src/client.rs`

**Changes**:

1. Add new error variants to `AiError` enum:

```rust
#[derive(Debug, Error)]
pub enum AiError {
    #[error("network error calling {model}: {source}")]
    Network {
        model: String,
        source: reqwest::Error,
    },

    #[error("api error for {model}: status {status}, body: {body}")]
    Api {
        model: String,
        status: u16,
        body: String,
    },

    #[error("failed to parse response for {model}: {source}")]
    Parse {
        model: String,
        source: reqwest::Error,
    },

    #[error("failed to parse JSON for {model}: {source}")]
    JsonParse {
        model: String,
        source: serde_json::Error,
    },

    #[error("no choices in response for {model}")]
    NoChoices { model: String },

    #[error("streaming aborted for {model}")]
    Aborted { model: String },

    #[error("SSE stream error for {model} at event {event_index}: {message}")]
    StreamError {
        model: String,
        event_index: usize,
        message: String,
        source: reqwest::Error,
    },

    #[error("SSE decode error for {model} at event {event_index}: {message}")]
    StreamDecodeError {
        model: String,
        event_index: usize,
        message: String,
        source: reqwest::Error,
    },
}
```

2. Update `chat_stream_cancellable` to track event index and use specific error variants:

```rust
let mut event_index = 0;
while let Some(chunk_result) = stream.next().await {
    if cancel.is_cancelled() {
        return Err(AiError::Aborted {
            model: model.to_string(),
        });
    }

    println!("[rhd_ai] Received chunk_result from stream, event_index={}", event_index);
    let chunk = chunk_result.map_err(|source| {
        println!("[rhd_ai] Stream error at event {}: {:?}", event_index, source);
        AiError::StreamDecodeError {
            model: model.to_string(),
            event_index,
            message: format!("Failed to read SSE chunk: {}", source),
            source,
        }
    })?;
    println!("[rhd_ai] Chunk size: {} bytes", chunk.len());

    let text = String::from_utf8_lossy(&chunk);
    buffer.push_str(&text);

    // Process complete SSE events (separated by \n\n)
    while let Some(event_end) = buffer.find("\n\n") {
        let event = buffer[..event_end].to_string();
        buffer = buffer[event_end + 2..].to_string();

        println!("[rhd_ai] Processing SSE event {}: {}", event_index, event);

        for line in event.lines() {
            if line.starts_with("data: ") {
                let data = &line[6..];

                if data == "[DONE]" {
                    return Ok(StreamResult {
                        finish_reason,
                        usage,
                    });
                }

                let stream_response: StreamResponse =
                    serde_json::from_str(data).map_err(|source| {
                        println!("[rhd_ai] JSON parse error at event {}: {:?}", event_index, source);
                        AiError::JsonParse {
                            model: model.to_string(),
                            source,
                        }
                    })?;

                // ... rest of processing
            }
        }
        event_index += 1;
    }
}
```

**Rationale**: Current error "error decoding response body" doesn't tell us which event failed or what the actual SSE data was. With event_index tracking, we can see if it fails on event 0 (first chunk) or later events.

### Phase 3: Add control channel logging for stream state tracking

**File**: `packages/rhd_test/src/mock_server.rs`

**Changes**: Add more detailed logging in the spawned task:

```rust
tokio::spawn(async move {
    println!("[mock_server] SSE stream task started, waiting for chunks");
    let mut event_count = 0;
    loop {
        println!("[mock_server] Waiting for next chunk from control channel...");
        match rx.recv().await {
            Some(chunk) => {
                println!("[mock_server] Received chunk from channel: {:?}, event_count={}", chunk, event_count);
                match chunk {
                    Some(content) => {
                        let stream_resp = StreamResponse {
                            choices: vec![StreamChoice {
                                delta: StreamDelta { content: Some(content) },
                                finish_reason: None,
                            }],
                        };
                        let json = serde_json::to_string(&stream_resp).unwrap();
                        println!("[mock_server] Sending SSE event {}: {}", event_count, json);
                        match event_tx.send(Ok(Event::default().data(json))).await {
                            Ok(_) => {
                                println!("[mock_server] Event {} sent successfully", event_count);
                                event_count += 1;
                            }
                            Err(e) => {
                                println!("[mock_server] Failed to send event {}: {:?}", event_count, e);
                                break;
                            }
                        }
                    }
                    None => {
                        // Stream finished
                        println!("[mock_server] Stream finished, sending final event and [DONE]");
                        let final_resp = StreamResponse {
                            choices: vec![StreamChoice {
                                delta: StreamDelta { content: None },
                                finish_reason: Some("stop".to_string()),
                            }],
                        };
                        let json = serde_json::to_string(&final_resp).unwrap();
                        println!("[mock_server] Sending final SSE event: {}", json);
                        let _ = event_tx.send(Ok(Event::default().data(json))).await;
                        println!("[mock_server] Sending [DONE] sentinel");
                        let _ = event_tx.send(Ok(Event::default().data("[DONE]"))).await;
                        break;
                    }
                }
            }
            None => {
                println!("[mock_server] Control channel closed unexpectedly after {} events", event_count);
                break;
            }
        }
    }
    println!("[mock_server] SSE stream task ended - sent {} events", event_count);
});
```

**Rationale**: Track how many events were successfully sent before failure. If event_count=0 when error occurs, the issue is with the first event or connection setup.

### Phase 4: Test fix with e2e test

After implementing Phases 1-3:

1. Run `cargo build` to ensure compilation succeeds
2. Run the e2e test: `npm run test:e2e -- chat-streaming.test.ts`
3. Check logs for:
   - `[mock_server] Event 0 sent successfully` - confirms first event sent
   - `[rhd_ai] Processing SSE event 0` - confirms daemon received first event
   - If error occurs, check `event_index` to see which event failed

**Expected outcome**: With keep-alive added, the connection should remain stable during the delay waiting for control server chunks. The detailed error logging will pinpoint exactly where the failure occurs if it persists.

## Next Steps

1. **Implement Phase 0** - Modify sse_test to use two-channel indirection with delay
2. **Test Phase 0** - Run modified sse_test to confirm two-channel + delay is the issue
3. **Implement Phase 1** - Add keep-alive to mock_server SSE response
4. **Implement Phase 2** - Add detailed error wrapping in rhd_ai client
5. **Implement Phase 3** - Add control channel logging
6. **Test** - Run e2e test and analyze logs
7. **Iterate** - If error persists, use event_index to identify failure point and fix

## Files Modified

1. `packages/rhd_app/src/chat.rs` - Filter empty chunks (2 locations) ✅
2. `frontend/src/lib/chatStores.ts` - Add `streamingMessageId` store ✅
3. `frontend/src/lib/chatWs.ts` - Optimistic message creation on first chunk, updates on subsequent ✅
4. `frontend/src/lib/types/index.ts` - Updated `ChatMessage.id` type ✅
5. `frontend/src/components/MessageList.svelte` - Show loader only when no streaming message yet ✅
6. `frontend/src/components/Message.svelte` - Add animated dots indicator for streaming messages ✅
7. `packages/rhd_test/src/mock_server.rs` - Add controlled streaming support, add keep-alive (Phase 1)
8. `packages/rhd_test/src/control_server.rs` - Add streaming control endpoints ✅
9. `packages/rhd_test/src/frontend_test.rs` - Pass new shared state ✅
10. `packages/rhd_test/src/main.rs` - Updated destructuring ✅
11. `packages/rhd_test/Cargo.toml` - Added dependencies ✅
12. `frontend/src/tests/testUtils.ts` - Add streaming control functions ✅
13. `frontend/src/tests/e2e/chat-streaming.test.ts` - New e2e test file ✅
14. `mise.toml` - Added `build-rhd-test` task ✅
15. `memory/chat.md` - Updated with empty chunk filtering ✅
16. `memory/frontend.md` - Updated with streaming architecture ✅
17. `memory/development.md` - Updated with test utilities ✅
18. `packages/rhd_ai/src/client.rs` - Add detailed error wrapping (Phase 2)

## Files Kept

- `frontend/src/components/StreamingMessage.svelte` - Still used for loader phase ✅

## Current Issues

All resolved ✅

### Resolved: chat-messageflow test failure

**Root Cause**: `configureMock()` set response content but mock server always used manual streaming control mode (waiting for control server chunks). When `chat-messageflow` test called `configureMock('Test response from AI')`, the mock server created an SSE stream and waited indefinitely for chunks that never came, causing SSE decode timeout.

**Fix**: Added `auto_stream` flag to mock server:
- `configureMock()` sets `auto_stream=true` → mock server auto-sends configured response as SSE stream immediately
- `setAutoStream(false)` disables auto mode for manual chunk control (used by `chat-streaming` test)
- `emitStreamChunk()` also sets `auto_stream=false` to ensure manual mode

**Files Modified**:
1. `packages/rhd_test/src/mock_server.rs` - Added `SharedAutoStream` type, auto_stream parameter, auto-stream SSE branch
2. `packages/rhd_test/src/control_server.rs` - Added `auto_stream` to state, `set_mock_response` sets auto_stream=true, `emit_stream_chunk` sets auto_stream=false, added `/set-auto-stream` endpoint
3. `packages/rhd_test/src/frontend_test.rs` - Updated destructuring for new return value
4. `packages/rhd_test/src/main.rs` - Updated destructuring for new return value
5. `frontend/src/tests/testUtils.ts` - Added `setAutoStream()` function
6. `frontend/src/tests/e2e/chat-streaming.test.ts` - Added `setAutoStream(false)` call before sending message

## Success Criteria

1. Backend does not emit `chatStreamChunk` events with empty content ✅
2. Frontend shows loader initially when streaming starts ✅
3. First chunk hides loader and shows assistant message with content + animated dots ✅
4. Subsequent chunks append to content, animated dots remain visible ✅
5. Animated dots indicator cycles through ".", "..", "..." every 200ms during streaming ✅
6. Stream finish removes animated dots, shows final message ✅
7. E2E test passes, verifying loader → streaming → final flow ❌ (blocked by SSE issue)
