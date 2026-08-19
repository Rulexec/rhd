# Frontend E2E Test Failures Analysis

**Date**: 2026-08-19  
**Test Run**: `mise run test-frontend-e2e`  
**Result**: 9 failed, 15 passed, 1 skipped

## Executive Summary

All 9 failing tests share a common root cause: **tests are timing out waiting for `isStreaming` to become `false`**. The streaming response never completes, causing tests to hang until timeout (5-15 seconds).

## Failure Pattern

### Common Symptom
All failures occur at `waitFor(() => expect(get(isStreaming)).toBe(false))` with timeout errors:
- `chat-state.test.ts`: 4 failures (lines 159, 193, 311, 633)
- `chat-mcp-tools.test.ts`: 3 failures (lines 153, 249, 324)
- `roles-integration.test.ts`: 2 failures (lines 143, 295)

### Expected Flow
1. Test calls `configureMock(responseContent)` to set up mock AI response
2. Test dispatches `sendMessage` action
3. Frontend sets `isStreaming = true`
4. Backend processes message and streams response via WebSocket
5. Backend sends `chatStreamFinished` event
6. Frontend receives event and sets `isStreaming = false`
7. Test assertion passes

### Actual Flow
Steps 1-3 complete successfully, but step 5 never occurs. The `chatStreamFinished` event is never received by the frontend, causing `isStreaming` to remain `true` indefinitely.

## Root Cause Analysis

### 1. Mock Server Auto-Stream Configuration

**Location**: `packages/rhd_test/src/mock_server/handlers.rs:56-139`

The mock server has two modes:
- **Auto-stream mode** (default): Immediately sends configured response
- **Manual control mode**: Waits for control server to send chunks

**Issue**: The test setup in `frontend/src/tests/testUtils.ts` only calls `configureMock()` to set the response content, but does NOT explicitly enable auto-stream mode. The mock server defaults to `auto_stream = true`, but this may not be properly propagated in the test environment.

**Code Evidence**:
```rust
// handlers.rs:56
let use_auto_stream = *auto_stream.lock().unwrap();
if use_auto_stream {
    // Auto-stream mode: send configured response immediately
    let content = response_content.clone();
    let events: Vec<Result<Event, Infallible>> = vec![
        Ok(Event::default().data(format!(r#"{{"choices":[{{"delta":{{"content":"{}"}},"finish_reason":null}}]}}"#, content))),
        Ok(Event::default().data(r#"{"choices":[{"delta":{"content":null},"finish_reason":"stop"}]}"#)),
        Ok(Event::default().data("[DONE]")),
    ];
    // ...
}
```

The mock server should send the stream events, but the frontend never receives them.

### 2. WebSocket Event Flow

**Location**: `frontend/src/lib/ws.ts:50-71`

The WebSocket handler validates incoming messages with Zod schema:
```typescript
socket.onmessage = (event: MessageEvent) => {
  const raw = JSON.parse(event.data);
  const result = WsMessageSchema.safeParse(raw);

  if (!result.success) {
    console.error('Invalid WebSocket message:', JSON.stringify(raw, null, 2));
    console.error('Validation errors:', JSON.stringify(result.error.issues, null, 2));
    return;  // <-- Message is dropped if validation fails
  }
  // ...
};
```

**Potential Issue**: If the backend sends a `chatStreamFinished` event that doesn't match the expected schema, it will be silently dropped.

**Schema Definition** (`frontend/src/lib/types/ws.ts:55-62`):
```typescript
export const ChatStreamFinishedEventSchema = z.object({
  type: z.literal('event'),
  event: z.literal('chatStreamFinished'),
  data: z.object({
    messageId: z.number(),
    chatId: z.number(),
  }),
});
```

**Backend Event Structure** (`packages/rhd_api/src/chat.rs:23-27`):
```rust
pub struct ChatStreamFinishedEvent {
    pub chat_id: i64,
    pub message_id: i64,
    pub finish_reason: String,
}
```

**Mismatch**: The backend sends `finish_reason` but the frontend schema doesn't expect it. However, Zod's `z.object()` by default strips unknown keys, so this shouldn't cause validation failure.

### 3. Backend Stream Processing

**Location**: `packages/rhd_chat/src/stream/send/utils.rs:67-87`

The backend sends `StreamFinished` event after processing the AI response:
```rust
let _ = event_sender.send(ChatEvent::StreamFinished {
    chat_id,
    message_id,
    finish_reason,
});
```

**Potential Issue**: The event might not be sent if:
- The AI client doesn't properly signal stream completion
- The event sender channel is closed or full
- An error occurs during stream processing

### 4. Test Environment Setup

**Location**: `frontend/src/tests/e2e/chat-state.test.ts:54-94`

The test spawns `rhd_test frontend` which starts:
- Mock AI server
- Control server
- WebSocket proxy server
- RHD daemon

**Potential Issue**: The WebSocket proxy might not be properly forwarding events from the daemon to the frontend test client.

## Specific Test Failures

### 1. chat-state.test.ts

#### Test: "sends message and receives streaming response from daemon" (line 136)
- **Failure**: Line 159 - `isStreaming` stays `true`
- **Expected**: Stream completes after mock response
- **Actual**: Stream never completes

#### Test: "handles multiple messages in sequence" (line 183)
- **Failure**: Line 193 - `isStreaming` stays `true` after first message
- **Expected**: First message stream completes
- **Actual**: First message stream never completes

#### Test: "edits message and re-streams response" (line 277)
- **Failure**: Line 311 - `isStreaming` stays `true`
- **Expected**: Edited message stream completes
- **Actual**: Stream never completes

#### Test: "aborts during AI call and resumes without aborted message" (line 598)
- **Failure**: Line 633 - `isPaused` stays `true`
- **Expected**: Pause state is set correctly
- **Actual**: Pause state never set

### 2. chat-mcp-tools.test.ts

All 3 tests fail with the same pattern:
- **Failure**: `isStreaming` stays `true` after tool execution
- **Expected**: Stream completes after tool call and final response
- **Actual**: Stream never completes after tool execution

**Note**: These tests involve MCP tool calls, which add complexity to the stream flow. The mock server returns tool calls, the backend executes them, then requests a final response. The stream should complete after the final response, but it doesn't.

### 3. roles-integration.test.ts

#### Test: "Scenario 1: Basic Role Selection" (line 109)
- **Failure**: Line 143 - `isStreaming` stays `true`
- **Expected**: Stream completes after role selection
- **Actual**: Stream never completes

#### Test: "Scenario 5: Project Attached After Chat Started" (line 241)
- **Failure**: Line 295 - `isStreaming` stays `true`
- **Expected**: Stream completes after project attachment
- **Actual**: Stream never completes

## Hypotheses

### Hypothesis 1: Mock Server Not Sending Stream Events
The mock server might not be properly configured to send stream events in the test environment. The `auto_stream` flag might not be set correctly, or the SSE events might not be properly formatted.

**Verification Needed**: Add logging to mock server to confirm it's sending events.

### Hypothesis 2: WebSocket Proxy Not Forwarding Events
The WebSocket proxy server (started by `rhd_test frontend`) might not be properly forwarding events from the RHD daemon to the frontend test client.

**Verification Needed**: Add logging to WebSocket proxy to confirm it's receiving and forwarding events.

### Hypothesis 3: Backend Not Sending StreamFinished Event
The backend might not be sending the `StreamFinished` event due to an error in stream processing or the event sender channel being closed.

**Verification Needed**: Add logging to backend stream processing to confirm `StreamFinished` event is sent.

### Hypothesis 4: Frontend Not Receiving Events
The frontend WebSocket connection might be closed or not properly connected when events are sent.

**Verification Needed**: Add logging to frontend WebSocket handler to confirm connection state.

### Hypothesis 5: Event Schema Validation Failure
The `chatStreamFinished` event might be failing Zod validation and being silently dropped.

**Verification Needed**: Check console output for "Invalid WebSocket message" errors.

## Recommended Investigation Steps

### Step 1: Add Comprehensive Logging
Add logging at each stage of the stream flow:
1. Mock server: Log when SSE events are sent
2. WebSocket proxy: Log when events are received and forwarded
3. Backend: Log when `StreamFinished` event is sent
4. Frontend: Log when WebSocket messages are received and validated

### Step 2: Verify Mock Server Behavior
Run a single test with mock server logging enabled to confirm:
- Mock server receives the chat completion request
- Mock server sends SSE events (chunk + finish)
- SSE events are properly formatted

### Step 3: Verify WebSocket Event Flow
Add logging to confirm:
- Backend sends `chatStreamFinished` event
- WebSocket proxy receives and forwards the event
- Frontend receives the event
- Event passes Zod validation

### Step 4: Check Test Environment Setup
Verify that `rhd_test frontend` properly initializes:
- Mock server with auto-stream enabled
- WebSocket proxy with proper event forwarding
- RHD daemon with proper event emission

### Step 5: Simplify Test Case
Create a minimal test case that:
1. Creates a chat
2. Sends a message
3. Waits for stream to complete
4. Logs all WebSocket messages received

This will help isolate whether the issue is in the mock server, backend, or frontend.

## Potential Fixes

### Fix 1: Ensure Mock Server Auto-Stream is Enabled
Add explicit call to enable auto-stream in test setup:
```typescript
export async function enableAutoStream(): Promise<void> {
  const response = await fetch(`${CONTROL_URL}/set-auto-stream`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ enabled: true }),
  });
  await checkControlResponse(response, 'enableAutoStream');
}
```

Call this in `beforeEach` or after `configureMock()`.

### Fix 2: Add Event Schema Flexibility
Update `ChatStreamFinishedEventSchema` to accept optional `finish_reason`:
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

### Fix 3: Add Timeout and Retry Logic
Add retry logic to wait for stream completion with better error messages:
```typescript
await waitFor(() => {
  const streaming = get(isStreaming);
  if (streaming) {
    console.log('Still streaming, waiting...');
  }
  expect(streaming).toBe(false);
}, { timeout: 5000, interval: 100 });
```

### Fix 4: Verify WebSocket Connection State
Add assertion to verify WebSocket is connected before sending message:
```typescript
import { wsConnected } from '@/lib/stores';

await waitFor(() => {
  expect(get(wsConnected)).toBe(true);
}, { timeout: 5000 });
```

## Conclusion

The root cause is that the streaming response never completes, causing `isStreaming` to remain `true`. This is likely due to:
1. Mock server not properly sending stream events, OR
2. WebSocket proxy not forwarding events, OR
3. Backend not sending `StreamFinished` event

The most likely cause is **Hypothesis 1 or 2**: the mock server or WebSocket proxy is not properly configured in the test environment.

**Next Steps**: Implement the recommended investigation steps to identify the exact failure point, then apply the appropriate fix.

## Appendix: Test Execution Details

- **Total Tests**: 25
- **Passed**: 15
- **Failed**: 9
- **Skipped**: 1
- **Duration**: 46.87s

**Failed Test Files**:
- `src/tests/e2e/chat-state.test.ts` (4 failures)
- `src/tests/e2e/chat-mcp-tools.test.ts` (3 failures)
- `src/tests/e2e/roles-integration.test.ts` (2 failures)

**Common Timeout**: 5000ms (most tests), 15000ms (MCP tools tests)
