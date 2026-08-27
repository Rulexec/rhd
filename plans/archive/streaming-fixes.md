# Streaming Fixes Plan

## Executive Summary

Investigation revealed **3 root causes** for the 5 reported streaming symptoms. No deadlocks were found in the code. The most critical issue is that the AI client buffers the entire response before yielding any chunks, which explains why no streaming messages appear in the network panel and why content fires all at once.

---

## Root Causes

### 1. AI Client Buffers Entire Response (Critical)

**File**: [`packages/rhd_ai_client/src/client.rs:63`](packages/rhd_ai_client/src/client.rs:63)

```rust
pub async fn chat_completion_stream(
    &self,
    mut request: ChatCompletionRequest,
) -> Result<impl futures_util::Stream<Item = Result<StreamChunk, AiClientError>>, AiClientError>
{
    request.stream = true;
    // ... send request ...
    
    // BUG: Reads entire response before returning stream
    let text = response.text().await?;
    let chunks = parse_sse_text(&text);
    
    Ok(futures_util::stream::iter(chunks))
}
```

**Impact**: 
- No streaming messages appear in network panel during generation
- All content appears at once when the entire response is received
- "Generating response..." persists until the full response is ready

**Fix**: Use `response.bytes_stream()` to actually stream the response body as it arrives, and parse SSE events incrementally using a streaming parser.

---

### 2. `getStreamContent()` Keys by `chatId` Instead of `messageId`

**File**: [`frontend/src/lib/components/ChatView.svelte:103`](frontend/src/lib/components/ChatView.svelte:103)

```typescript
function getStreamContent(message: MessageType): StreamSubscription | null {
  return streamSubscriptions.get(message.chatId) || null;  // BUG: chatId, not messageId
}
```

**Impact**: 
- When a new streaming message arrives, ALL messages in the same chat receive the same `streamContent`
- Previous messages appear empty because `displayContent` returns `streamContent.content` (which starts empty)

**Fix**: Key the `streamSubscriptions` map by `message.id` instead of `message.chatId`.

---

### 3. Duplicate `streamChunk` Event Delivery

**Files**: 
- [`packages/rhd_chat_server/src/handlers/stream.rs:42`](packages/rhd_chat_server/src/handlers/stream.rs:42) — `broadcast_to_chat` in `stream_push`
- [`packages/rhd_chat_server/src/handlers/stream.rs:115`](packages/rhd_chat_server/src/handlers/stream.rs:115) — spawned task in `stream_subscribe`

**Impact**: 
- Frontend receives `streamChunk` via TWO paths:
  1. Via `broadcast_to_chat` (because frontend is a chat subscriber)
  2. Via the spawned task (because frontend called `streamSubscribe`)
- This causes duplicate events and potential content doubling

**Fix**: Remove the `broadcast_to_chat` call in `stream_push`. Rely only on the spawned task in `stream_subscribe` for delivering stream chunks to subscribers.

---

## Implementation Plan

### Phase 1: Fix AI Client Streaming (Critical)

**Goal**: Make the AI client actually stream the response as it arrives.

**Steps**:
1. Replace `response.text().await` with `response.bytes_stream()` to get a streaming body
2. Implement an incremental SSE parser that processes bytes as they arrive
3. Yield `StreamChunk` objects as soon as each SSE event is complete
4. Test with a slow AI provider to verify streaming works

**Files to modify**:
- `packages/rhd_ai_client/src/client.rs`

---

### Phase 2: Fix Frontend Stream Subscription Keying

**Goal**: Ensure each message gets its own stream subscription.

**Steps**:
1. Change `streamSubscriptions` map key from `chatId` to `messageId`
2. Update `ensureStreamSubscription()` to use `message.id`
3. Update `getStreamContent()` to use `message.id`
4. Update `onStreamChunk` and `onStreamFinished` handlers to find the correct message

**Files to modify**:
- `frontend/src/lib/components/ChatView.svelte`

---

### Phase 3: Remove Duplicate Event Delivery

**Goal**: Ensure `streamChunk` events are delivered only once per subscriber.

**Steps**:
1. Remove the `broadcast_to_chat` calls in `stream_push` handler
2. Keep the spawned task in `stream_subscribe` as the sole delivery mechanism
3. Verify that frontend still receives all stream events

**Files to modify**:
- `packages/rhd_chat_server/src/handlers/stream.rs`

---

## Testing Strategy

### Manual Testing
1. Send a message and verify that streaming content appears incrementally in the UI
2. Verify that previous messages remain visible during streaming
3. Verify that the final message content is displayed correctly
4. Check the network panel to confirm streaming messages appear during generation

### Automated Testing
1. Add integration tests for the AI client streaming
2. Add frontend tests for stream subscription keying
3. Add backend tests for duplicate event removal

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| AI client streaming change breaks existing functionality | Medium | High | Test with mock AI provider first |
| Frontend subscription keying change breaks message display | Low | Medium | Test with multiple concurrent streams |
| Removing duplicate events breaks some clients | Low | Medium | Verify all clients use `streamSubscribe` |

---

## Dependencies

- Phase 1 (AI client streaming) is independent and should be done first
- Phase 2 (frontend subscription keying) is independent
- Phase 3 (duplicate events) is independent

All phases can be done in parallel, but Phase 1 is the most critical for fixing the reported symptoms.

---

## Success Criteria

1. Streaming content appears incrementally in the UI as the AI generates it
2. Previous messages remain visible during streaming
3. The final message content is displayed correctly
4. Network panel shows streaming messages during generation
5. No duplicate `streamChunk` events are received by the frontend
