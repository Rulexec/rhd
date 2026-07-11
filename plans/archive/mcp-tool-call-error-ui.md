# MCP Tool Call Error UI Plan

## Problem Statement

When an MCP tool call fails (e.g., "Access denied - path outside allowed directories"), the frontend does not display the error state properly. The `ToolCallMessage` component already has UI logic for error states (✗ icon and red styling), but the status is never set to 'failed' because the backend's `is_error` flag is not propagated to the frontend.

## Root Cause Analysis

1. **Backend has error information**: The `ToolResult` struct in `rhd_mcp_client/src/lib.rs` has an `is_error: Option<bool>` field that is set to `Some(true)` when a tool call fails.

2. **Error info not propagated**: In `tools.rs`, when emitting `ChatEvent::ToolCallCompleted`, only the `result` string is sent, not the `is_error` flag.

3. **Event struct needs update**: The `ToolCallCompletedEvent` in `rhd_api/src/lib.rs` only has `chat_id`, `tool_call_id`, and `result` fields - no `is_error` field.

4. **Frontend always sets status to 'completed'**: In `chatWs.ts`, the `chatToolCallCompleted` handler always sets `status: 'completed'` regardless of whether the tool call succeeded or failed.

5. **Component already has error UI**: The `ToolCallMessage.svelte` component already has logic to display '✗' icon and 'status-failed' class when status is 'failed', and the CSS already has red color for `.status-failed`.

## Implementation Plan

### Step 1: Update Backend Event Types

**File: `packages/rhd_chat/src/event.rs`**
- Add `is_error: bool` field to `ChatEvent::ToolCallCompleted` variant

**File: `packages/rhd_api/src/lib.rs`**
- Add `is_error: bool` field to `ToolCallCompletedEvent` struct

### Step 2: Update Backend Event Emission

**File: `packages/rhd_chat/src/tools.rs`**
- Update the `ToolCallCompleted` event emission to include `is_error: tool_result.is_error.unwrap_or(false)`

**File: `packages/rhd_app/src/ws.rs`**
- Update the `ToolCallCompletedEvent` construction to include the `is_error` field from the `ChatEvent`

### Step 3: Update Frontend Event Handling

**File: `frontend/src/lib/chatWs.ts`**
- Update the `chatToolCallCompleted` handler to check the `is_error` field
- Set `status: 'failed'` when `is_error` is true, otherwise `status: 'completed'`

### Step 4: Add Red Border Styling for Failed Tool Calls

**File: `frontend/src/components/ToolCallMessage.svelte`**
- Add a reactive class binding to apply a `failed` class to the `.tool-call` div when `toolCall.status === 'failed'`
- Add CSS for `.tool-call.failed` with red border styling

## Files to Modify

1. `packages/rhd_chat/src/event.rs` - Add `is_error` field to `ToolCallCompleted`
2. `packages/rhd_api/src/lib.rs` - Add `is_error` field to `ToolCallCompletedEvent`
3. `packages/rhd_chat/src/tools.rs` - Pass `is_error` when emitting event
4. `packages/rhd_app/src/ws.rs` - Include `is_error` in payload
5. `frontend/src/lib/chatWs.ts` - Check `is_error` and set status accordingly
6. `frontend/src/components/ToolCallMessage.svelte` - Add red border styling for failed state

## Testing Considerations

- Verify that successful tool calls still show green checkmark and normal border
- Verify that failed tool calls show red X and red border
- Verify that the error message is still displayed in the result section
- Test with MCP tools that return errors (e.g., file system access denied)
