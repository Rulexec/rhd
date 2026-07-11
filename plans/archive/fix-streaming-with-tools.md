# Fix: Enable Streaming for Tool Calls

## Problem

When a chat has attached MCP tools, ALL AI calls use non-streaming mode — even when the AI responds with plain text (no tool calls). The raw log shows `"object":"chat.completion"` instead of `"chat.completion.chunk"`.

## Root Cause

[`tool_loop()`](packages/rhd_chat/src/tools.rs:52) calls [`chat_with_tools()`](packages/rhd_ai/src/client.rs:195) which hardcodes `stream: false`.

## Solution

Enable streaming mode for `chat_with_tools()` calls. OpenAI API supports `stream: true` with tools — tool calls arrive as delta chunks that need accumulation.

### Implementation

1. **Add `chat_stream_with_tools()` to `OpenAiClient`**
   - Similar to `chat_stream_cancellable()` but includes tools in request
   - Parse tool call deltas from streaming chunks (id, type, function name, arguments)
   - Accumulate tool call arguments across chunks (arguments come as string fragments)
   - Return `ChatResult` with complete tool calls OR stream result

2. **Modify `tool_loop()` to use streaming**
   - Replace `chat_with_tools()` with `chat_stream_with_tools()`
   - Stream content chunks to frontend via `StreamChunk` events
   - Stream thinking/reasoning via `ThinkingChunk` events
   - When tool calls detected in stream, execute them and continue loop
   - When final response (no tool calls), stream completes naturally

3. **Update raw logging**
   - Log streaming chunks for tool loop calls
   - Maintain existing log format

### Files to Modify

- `packages/rhd_ai/src/client.rs` — add `chat_stream_with_tools()`
- `packages/rhd_chat/src/tools.rs` — update `tool_loop()` to use streaming
- `packages/rhd_chat/src/chat_log.rs` — ensure raw logging works for streaming tool calls

### Testing

- Verify streaming works when tools attached but AI responds with text only
- Verify tool calls still execute correctly with streaming
- Verify raw logs show SSE chunks instead of single JSON response
- Test with mock MCP server in E2E tests

## Benefits

- Real-time streaming feedback even with tools attached
- Consistent UX across all chat scenarios
- Better perceived performance for long responses
