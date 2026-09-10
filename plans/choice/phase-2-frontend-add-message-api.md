# Phase 2: Frontend — `addMessage` API & `ChatStore.addToolResult` Flow

> Parent plan: [`plans/rhd-plugin-choice-plan.md`](../rhd-plugin-choice-plan.md)

## Overview

Give the frontend the ability to post a message **directly** into a chat (bypassing
the queue), specifically to answer an assistant tool call with a `role: "tool"`
message carrying the `toolCallId`. Today the frontend only exposes
`addQueueMessage` (`frontend/src/lib/api/chatApiImpl.ts:272`); the server's
`addMessage` method is already connection-agnostic (no plugin registration
required — `packages/rhd_chat_server/src/handlers/mod.rs:59`) and rejects
`role: "tool"` without `toolCallId` (`handlers/message.rs:75-80`).

**Scope in:** `addMessage` API function, `ChatApi` interface entry, `ChatStore`
flow method, store tests.
**Scope out:** any UI (Phase 3), any backend/protocol changes (none needed).

**Dependencies:** none. Parallel with Phase 1. Required by Phase 3.

## API Contract (fixed — Phase 3 relies on this)

```ts
chatApi.addMessage(chatId, role, content, toolCallId?, tags?)  // → void
chatStore.addToolResult(chatId, toolCallId, content)           // flow → Promise<void>
```

`addToolResult` is the only call site the choice UI uses; it sends
`role: "tool"` and the given `toolCallId`. The content is plain text (the chosen
option's exact text or the user's typed message) — no JSON envelope.

## Files to Modify

### 1. `frontend/src/lib/api/chatApiImpl.ts`

**Add** immediately after the existing `addQueueMessage` function (ends at
line 284), mirroring its style (no result parsing; the server's
`AddMessageResult { messageId }` is not needed by the UI):

```ts
/**
 * Add a message directly to a chat (bypassing the queue).
 *
 * Used to answer tool calls: `role: "tool"` requires a `toolCallId`
 * (the server rejects it otherwise). The posted message is broadcast
 * back via `messageAdded`, so stores update through the event — no
 * optimistic local insert here.
 *
 * @param chatId - Chat ID
 * @param role - Message role ("tool" for tool results)
 * @param content - Message content
 * @param toolCallId - For tool-role messages: the assistant tool call id being answered
 * @param tags - Optional tags
 */
export async function addMessage(
  chatId: number,
  role: string,
  content: string,
  toolCallId?: string,
  tags: string[] = []
): Promise<void> {
  await websocket.request('addMessage', {
    chatId,
    role,
    content,
    ...(toolCallId !== undefined ? { toolCallId } : {}),
    tags
  });
}
```

Note: `toolCallId` is spread conditionally so non-tool calls send no `toolCallId`
key at all (the server's `AddMessageParams` uses `skip_serializing_if` / `Option`,
but explicit omission keeps the wire payload identical to the Rust default).

### 2. `frontend/src/lib/api/ChatApi.ts`

**Modify** the `ChatApi` interface — add after `addQueueMessage` (line 24):

```ts
  addMessage: typeof chatApi.addMessage;
```

**Modify** `defaultChatApi` — add after the `addQueueMessage` mapping (line 53):

```ts
  addMessage: chatApi.addMessage,
```

### 3. `frontend/src/stores/ChatStore.ts`

**Add** a generator flow right after `addQueueMessage` (ends at line 306). Same
error-handling shape as `addQueueMessage` (set `this.error`, rethrow):

```ts
  /**
   * Answer an assistant tool call by posting a tool-role message.
   *
   * The server broadcasts `messageAdded` for it; `toolResults` (and any UI
   * derived from it) updates reactively. Throws on failure so callers can
   * keep their controls enabled for a retry.
   *
   * @param chatId - Chat ID
   * @param toolCallId - The assistant tool call id being answered
   * @param content - Plain-text result (chosen option text or user message)
   */
  *addToolResult(chatId: number, toolCallId: string, content: string): Generator<unknown, void, unknown> {
    try {
      yield* yieldPromise(this.#chatApi.addMessage(chatId, 'tool', content, toolCallId));
    } catch (error) {
      this.error = error instanceof Error ? error.message : String(error);
      throw error;
    }
  }
```

(`makeAutoObservable` auto-wraps the generator into a flow — calling
`store.addToolResult(...)` returns a `CancellablePromise`; Phase 3 invokes it
with `flowResult(...)` from an event handler.)

## Tests

### `frontend/src/stores/ChatStore.test.ts` (modify)

The existing mock is cast `as unknown as ChatApi` (line 66), so adding an
interface member breaks nothing — but add the mock member next to
`addQueueMessage` (line 62) so it is explicit:

```ts
      addQueueMessage: vi.fn(),
      addMessage: vi.fn().mockResolvedValue(undefined),
```

**Add** a new `describe('addToolResult', ...)` block (place near the existing
`addQueueMessage` tests):

```ts
  describe('addToolResult', () => {
    it('should post a tool-role message with the toolCallId', async () => {
      await store.addToolResult(1, 'call_42', 'Option A');

      expect(mockChatApi.addMessage).toHaveBeenCalledWith(1, 'tool', 'Option A', 'call_42');
      expect(store.error).toBe(null);
    });

    it('should set error and rethrow on failure', async () => {
      vi.mocked(mockChatApi.addMessage).mockRejectedValueOnce(new Error('send failed'));

      await expect(store.addToolResult(1, 'call_42', 'x')).rejects.toThrow('send failed');
      expect(store.error).toBe('send failed');
    });
  });
```

No other test files need changes: `ChatsListStore.test.ts` / `PluginsStore.test.ts`
mocks are also partial-cast and never call `addMessage`.

## Verification

```bash
cd frontend && npm run check && npm test
```

(`npm run check` = `svelte-check`; `npm test` = `vitest run`.)

## Implementation Notes

1. **Why not reuse the queue?** A tool result must be a direct, non-queued
   message with `toolCallId` so `ai_completions` sees the tool call resolved and
   continues the loop (`trigger_detection.rs`: `ToolLoopContinuation` requires
   all tool calls answered). Queue messages would be promoted as user-role
   messages without the `toolCallId` linkage.
2. **No optimistic insert.** The `messageAdded` broadcast flows back into
   `ChatStore.messages` → `toolResults` getter (`ChatStore.ts:96-104`), which is
   the single source of truth for "answered". Phase 3's resolved state derives
   entirely from that map; the store needs no new fields.
3. **Rethrow on failure** so the UI can keep the controls enabled for a retry
   instead of silently swallowing a rejected answer (matches `addQueueMessage`).
4. **`tags` parameter kept for API symmetry** with `addQueueMessage`; the choice
   flow passes none.

## Dependencies

- Depends on: nothing (server method already exists).
- Blocks: Phase 3 (the choice UI calls `chatStore.addToolResult`).
