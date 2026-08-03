# Pause During AI Call Fix

## Problem

When a user pauses a chat during an AI call (while streaming), the current implementation has an issue: the pause behavior needs to be clarified and potentially fixed.

**Expected Behavior:**
- When pause is called during AI streaming, the streaming should continue until it finishes naturally
- After the AI call completes, the tool loop should NOT continue to execute tools
- Instead, the tool calls should be stored as pending and the chat should enter a paused state
- The user can then resume to continue with tool execution

**Current Implementation Analysis:**

### Current Flow

1. **`pause_chat` in [`manager.rs`](packages/rhd_chat/src/manager.rs:138)**:
   - Only works when state is `Running`
   - Transitions to `Paused` state
   - Does NOT cancel the token
   - Stores `pause_notify` for resuming

2. **`tool_loop` in [`tool_loop.rs`](packages/rhd_chat/src/tools/tool_loop.rs:72)**:
   - Checks `cancel_token.is_cancelled()` at top of loop (line 89)
   - Calls `chat_stream_with_tools` with `cancel_token.clone()` (line 126)
   - After AI call completes, checks if paused (lines 213-232)
   - If paused, stores tool calls as pending and returns `ToolLoopResult::Paused`

3. **`chat_stream_with_tools` in [`tools.rs`](packages/rhd_ai/src/client/stream/tools.rs:67)**:
   - Uses `tokio::select!` to race `stream.next()` against `cancel.cancelled()`
   - If cancel is detected, returns `AiError::Aborted`

### Analysis

Looking at the code, the current implementation appears to be correct:
- `pause_chat` does NOT cancel the token
- `chat_stream_with_tools` only aborts if `cancel.is_cancelled()` is true
- So the streaming should continue until it finishes naturally
- After AI call completes, `tool_loop` checks if paused and returns `ToolLoopResult::Paused`

However, the user reports that "pause aborts streaming instead of waiting until streaming will be finished". This suggests there may be an issue in the actual behavior that is not obvious from the code.

## Investigation Plan

### Step 1: Verify Current Behavior

Create a test to verify the current pause behavior during AI streaming:

1. Start a chat with streaming
2. Call pause while streaming is in progress
3. Verify that:
   - Streaming continues until it finishes
   - Tool calls are stored as pending
   - Chat enters paused state
   - No tools are executed

### Step 2: Identify the Issue

If the current behavior is incorrect, identify where the streaming is being aborted:

1. Check if `pause_chat` is somehow canceling the token
2. Check if there's a race condition in the state transition
3. Check if the frontend is sending an abort instead of pause
4. Check if there's a timeout or other mechanism that's canceling the stream

### Step 3: Fix the Issue

Based on the investigation, fix the issue to ensure:
- Pause does NOT cancel the streaming
- Streaming continues until it finishes naturally
- After streaming finishes, the tool loop pauses before executing tools
- The chat enters a proper paused state

### Step 4: Write Tests

Write comprehensive tests to verify:
- Pause during AI call (streaming phase)
- Pause during tool execution
- Resume after pause during AI call
- Resume after pause during tool execution
- Message queue during pause

## Files to Investigate

| File | What to Check |
|------|---------------|
| [`packages/rhd_chat/src/manager.rs`](packages/rhd_chat/src/manager.rs:138) | `pause_chat` implementation |
| [`packages/rhd_chat/src/tools/tool_loop.rs`](packages/rhd_chat/src/tools/tool_loop.rs:213) | Pause check after AI call |
| [`packages/rhd_ai/src/client/stream/tools.rs`](packages/rhd_ai/src/client/stream/tools.rs:67) | Cancel token handling in streaming |
| [`packages/rhd_app/src/ws/handlers/chat.rs`](packages/rhd_app/src/ws/handlers/chat.rs:176) | WebSocket pause handler |
| [`frontend/src/lib/chatWs/events.ts`](frontend/src/lib/chatWs/events.ts) | Frontend pause event handling |

## Success Criteria

1. When pausing during AI call:
   - Streaming continues until it finishes naturally
   - Tool calls are stored as pending
   - Chat enters paused state
   - No tools are executed
   - Frontend shows paused state with pending tool calls

2. When resuming after pause during AI call:
   - Tool execution continues from where it was paused
   - Streaming state is restored
   - Frontend shows streaming state

3. All existing tests continue to pass

## Risks

1. **Race conditions**: The pause state transition must be atomic to avoid race conditions with the streaming completion
2. **State consistency**: The paused state must be consistent across backend and frontend
3. **Tool call ordering**: Pending tool calls must be preserved in the correct order
4. **Message queue**: Messages queued during pause must be properly handled on resume
