# Fix Chat Message Duplication and Streaming State Bug

## Problem

When creating a new chat, sending a message, and waiting for the response:
1. Message field remains blocked (textarea disabled)
2. Abort button remains visible
3. User message is duplicated
4. Assistant response message is duplicated

## Root Cause

Backend sends events in this order for a successful message:
1. `chatMessageAdded` (user message, with real DB ID)
2. `chatStreamChunk` events (streaming content)
3. `chatMessageAdded` (assistant message, with real DB ID)
4. `chatStreamFinished` (stream completed)

Frontend has three bugs in [`handleChatEvent()`](frontend/src/lib/chatWs.ts:146):

### Bug 1: User message duplication
[`sendMessage()`](frontend/src/lib/chatWs.ts:82) adds user message with `tempId = Date.now()`. When `chatMessageAdded` arrives with real DB ID, the dedup check (`list.some(m => m.id === added.message.id)`) fails because `tempId ≠ realId`, so a duplicate is added.

### Bug 2: Assistant message duplication
`chatMessageAdded` (event #3) adds the assistant message. Then `chatStreamFinished` (event #4) adds it **again** unconditionally — no dedup check in that handler.

### Bug 3: Streaming state not reset
The `chatStreamFinished` handler adds a duplicate assistant message **before** setting `isStreaming.set(false)`. This creates a race condition where the duplicate message addition might interfere with the state reset. The safest fix is to remove the duplicate message addition from `chatStreamFinished` and rely solely on `chatMessageAdded` for adding messages.

## Solution

### Fix 1: Remove optimistic user message addition
Don't add the user message in `sendMessage()`. Let the `chatMessageAdded` event handle it. The event arrives quickly, so the delay is minimal.

**File**: `frontend/src/lib/chatWs.ts`
- Remove lines 86-95 (user message creation and addition to store)
- Keep `isStreaming.set(true)`, `streamingContent.set('')`, `streamError.set(null)`

### Fix 2: Remove assistant message addition from `chatStreamFinished`
Don't add the assistant message in `chatStreamFinished`. The `chatMessageAdded` event (which arrives before `chatStreamFinished`) already adds it.

**File**: `frontend/src/lib/chatWs.ts`
- In `chatStreamFinished` case, remove the `messages.update()` call
- Keep `isStreaming.set(false)`, `streamingContent.set('')`, `streamError.set(null)`

### Fix 3: Add e2e tests
Write e2e tests to verify:
1. User message appears exactly once after sending
2. Assistant message appears exactly once after response
3. Message field is unblocked after response
4. Abort button disappears after response

**File**: `frontend/src/tests/e2e/chat-message-flow.test.ts` (new file)
- Each e2e test in a separate file per project convention
- Test the full message send/receive flow

## Implementation Steps

1. **Fix `sendMessage()` in `chatWs.ts`**
   - Remove optimistic user message addition
   - Keep streaming state initialization

2. **Fix `chatStreamFinished` handler in `chatWs.ts`**
   - Remove assistant message addition
   - Keep streaming state cleanup

3. **Add e2e test in `chat-messageflow.test.ts`**
   - Create new chat
   - Configure mock AI response
   - Send message
   - Wait for response
   - Verify user message appears exactly once
   - Verify assistant message appears exactly once
   - Verify textarea is enabled
   - Verify Send button is visible (not Abort)

4. **Run tests**
   - `mise run test-frontend-e2e` to verify fixes

## Files to Modify

- `frontend/src/lib/chatWs.ts` — fix duplication bugs
- `frontend/src/tests/e2e/chat-messageflow.test.ts` — add e2e tests (new file)

## Risks

- Removing optimistic user message addition means the user message won't appear instantly. It will appear when the `chatMessageAdded` event arrives. This should be fast (milliseconds), but there might be a slight delay.
- If the `chatMessageAdded` event is not received for some reason, the message won't appear. However, this is unlikely given the current architecture.

## Success Criteria

- User message appears exactly once after sending
- Assistant message appears exactly once after response
- Message field is unblocked after response
- Abort button disappears after response
- E2E tests pass
