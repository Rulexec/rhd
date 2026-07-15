# Fix Tool Calls Display After Page Reload

## Problem

When viewing a chat during live streaming, tool calls are displayed correctly in the `tool-calls-container` UI component. However, after page reload, the same messages display as plain JSON instead of the proper tool call UI.

## Root Cause

**During live streaming:**
- Frontend processes `chatMessageAdded` events in `handleChatEvent()`
- JSON content is parsed to extract `toolCalls` array (lines 390-410 and 440-461 in `chatWs.ts`)
- Tool result messages (role: "tool") are merged into assistant's `toolCalls` (lines 350-382)
- Messages are displayed with proper `toolCalls` property

**After page reload:**
- `selectChat()` calls `messages.set(response.data.messages || [])` directly (line 76)
- No parsing of JSON content occurs
- Tool result messages remain as separate messages
- Assistant messages have `content` as raw JSON string instead of parsed `toolCalls`

## Solution

Extract shared helper functions that both `selectChat()` and `handleChatEvent()` can use, ensuring consistent behavior between live streaming and page reload.

## Implementation

### File: `frontend/src/lib/chatWs.ts`

#### Step 1: Add shared helper functions at module level

```typescript
/**
 * Parses an assistant message's JSON content to extract toolCalls.
 * Returns the message with parsed content and toolCalls, or the original message if not JSON.
 */
function parseAssistantMessage(msg: any): any {
  if (msg.role !== 'assistant') return msg;
  
  try {
    const json = JSON.parse(msg.content);
    if (json.content !== undefined && json.toolCalls) {
      const toolCalls = json.toolCalls.length > 0 ? json.toolCalls.map((tc: any) => ({
        id: tc.id,
        name: tc.function?.name || tc.name,
        arguments: tc.function?.arguments || tc.arguments,
        status: tc.status || 'pending',
        mcpId: tc.mcpId || (tc.function?.name || tc.name || '').split('/')[0],
        result: tc.result,
      })) : undefined;
      
      return {
        ...msg,
        content: json.content,
        toolCalls,
      };
    }
  } catch {
    // Not JSON, use as-is
  }
  
  return msg;
}

/**
 * Parses a tool result message and returns the parsed data.
 * Returns null if the content is not valid tool result JSON.
 */
function parseToolResult(content: string): { toolCallId: string; result: string; isError: boolean } | null {
  try {
    const toolResult = JSON.parse(content);
    if (toolResult.toolCallId) {
      return {
        toolCallId: toolResult.toolCallId,
        result: toolResult.result,
        isError: toolResult.isError || false,
      };
    }
  } catch {
    // Not JSON
  }
  return null;
}

/**
 * Merges tool results into assistant messages' toolCalls.
 * Filters out separate tool messages from the result.
 * If a tool message is not valid JSON, it is kept as a regular message.
 */
function mergeToolResults(messages: any[]): any[] {
  // First pass: collect tool results by toolCallId
  const toolResults = new Map<string, { result: string; isError: boolean }>();
  
  for (const msg of messages) {
    if (msg.role === 'tool') {
      const parsed = parseToolResult(msg.content);
      if (parsed) {
        toolResults.set(parsed.toolCallId, { result: parsed.result, isError: parsed.isError });
      }
    }
  }
  
  // Second pass: process messages, merging tool results and filtering out tool messages
  const result: any[] = [];
  for (const msg of messages) {
    // Skip tool messages that were successfully parsed (they're merged into assistant)
    if (msg.role === 'tool') {
      const parsed = parseToolResult(msg.content);
      if (parsed) {
        continue; // Skip - will be merged
      }
      // Not JSON - keep as regular message
    }
    
    // Parse assistant messages to extract toolCalls
    let processed = parseAssistantMessage(msg);
    
    // Merge tool results into toolCalls
    if (processed.toolCalls && processed.toolCalls.length > 0) {
      const updatedToolCalls = processed.toolCalls.map((tc: any) => {
        const toolResult = toolResults.get(tc.id);
        if (toolResult) {
          return {
            ...tc,
            result: toolResult.result,
            status: toolResult.isError ? 'failed' as const : 'completed' as const,
          };
        }
        return tc;
      });
      processed = { ...processed, toolCalls: updatedToolCalls };
    }
    
    result.push(processed);
  }
  
  return result;
}
```

#### Step 2: Update `selectChat()` to use `mergeToolResults()`

```typescript
export async function selectChat(chatId: number): Promise<WsResponse> {
  const id = generateRequestId();
  const response = await sendRequest({ type: 'getChat', id, chatId });
  if (response.success) {
    currentChatId.set(chatId);
    messages.set(mergeToolResults(response.data.messages || []));  // <-- Use shared function
    // ... rest unchanged
  }
  return response;
}
```

#### Step 3: Refactor `handleChatEvent()` to use shared helpers

Replace the duplicated JSON parsing logic in `chatMessageAdded` handler:

**Before (lines 390-410 and 440-461):**
```typescript
// Parse JSON content to extract toolCalls if present
let parsedContent = added.message.content;
let toolCalls: any[] | undefined;

try {
  const json = JSON.parse(added.message.content);
  if (json.content !== undefined && json.toolCalls) {
    parsedContent = json.content;
    toolCalls = json.toolCalls.length > 0 ? json.toolCalls.map((tc: any) => ({
      id: tc.id,
      name: tc.function?.name || tc.name,
      arguments: tc.function?.arguments || tc.arguments,
      status: tc.status || 'completed',
      mcpId: tc.mcpId || (tc.function?.name || tc.name || '').split('/')[0],
      result: tc.result,
    })) : undefined;
  }
} catch {
  // Not JSON, use as-is
}
```

**After:**
```typescript
const parsed = parseAssistantMessage(added.message);
const parsedContent = parsed.content;
const toolCalls = parsed.toolCalls;
```

**Before (lines 350-382 for tool result handling):**
```typescript
if (added.message.role === 'tool') {
  try {
    const toolResult = JSON.parse(added.message.content);
    const toolCallId = toolResult.toolCallId;
    // ... merge logic
  } catch {
    return [...list, added.message as any];
  }
}
```

**After:**
```typescript
if (added.message.role === 'tool') {
  const parsed = parseToolResult(added.message.content);
  if (parsed) {
    // ... merge logic using parsed.toolCallId, parsed.result, parsed.isError
  } else {
    // Not JSON - add as regular message
    return [...list, added.message as any];
  }
}
```

## Testing

1. Create a chat and send a message that triggers a tool call
2. Verify tool calls display correctly during streaming
3. Reload the page
4. Verify tool calls still display correctly after reload
5. Verify the `tool-calls-container` UI is shown (not plain JSON)
6. Verify tool results are merged correctly (status shows "completed" or "failed")
7. Verify non-JSON tool messages are still displayed as regular messages

## Files to Modify

- `frontend/src/lib/chatWs.ts` - Add shared helpers and refactor both `selectChat()` and `handleChatEvent()`
