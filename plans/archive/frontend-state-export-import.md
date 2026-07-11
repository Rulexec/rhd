# Frontend State Export/Import

Expose `window.__exportState()` and `window.__importState(state)` on the frontend so the complete UI state can be serialized and restored.

## Goals

- `window.__exportState()` returns a plain, JSON-serializable object containing every piece of frontend state.
- `window.__importState(state)` restores all stores and the URL hash from such an object.
- Round-trip is idempotent: exporting after a successful import yields the same state (excluding volatile/derived values).
- Works from the browser console for debugging and testing.

## State to Capture

All writable stores across the frontend:

From `src/lib/stores.ts`:
- `activeScenarios` (Map → array of entries)
- `finishedScenarios`
- `pausedScenarios` (Map → array of entries)
- `lastKnownId`
- `wsConnected`

From `src/lib/chatStores.ts`:
- `chats`
- `currentChatId`
- `messages`
- `streamingContent`
- `streamingThinkingContent`
- `isStreaming`
- `streamError`
- `availableModels`
- `selectedModel`
- `streamingMessageId`
- `isPaused`
- `pendingToolCalls`

From `src/lib/projectStores.ts`:
- `projects`
- `mcpStatuses`
- `chatProjects`

Router state:
- Current `window.location.hash` (e.g. `#/chats/42` or `#/scenarios`).

## Proposed State Schema

```ts
interface ExportedState {
  version: 1;
  hash: string;
  scenarios: {
    active: Array<[string, ActiveScenario]>;
    finished: FinishedScenario[];
    paused: Array<[string, PausedScenario]>;
    lastKnownId: number;
    wsConnected: boolean;
  };
  chat: {
    chats: Chat[];
    currentChatId: number | null;
    messages: ChatMessage[];
    streamingContent: string;
    streamingThinkingContent: string;
    isStreaming: boolean;
    streamError: string | null;
    availableModels: string[];
    selectedModel: string | null;
    streamingMessageId: string | null;
    isPaused: boolean;
    pendingToolCalls: ToolCall[];
  };
  projects: {
    projects: Project[];
    mcpStatuses: McpStatus[];
    chatProjects: ChatProject[];
  };
}
```

## Implementation Steps

1. **Extend custom scenario stores**
   - Add `setAll(state: Map<string, ActiveScenario>)` to `activeScenarios` store.
   - Add `setAll(state: Map<string, PausedScenario>)` to `pausedScenarios` store.
   - These methods expose the internal `set` function needed for restoration.

2. **Create `src/lib/stateExport.ts`**
   - Define `ExportedState` interface.
   - Implement `exportState(): ExportedState` that reads every store with `get()` and serializes Maps to arrays.
   - Implement `importState(state: ExportedState): void` that:
     - Validates the object shape (basic version check + runtime guards).
     - Sets every writable store to the imported value.
     - Reconstructs `activeScenarios` and `pausedScenarios` from arrays via the new `setAll` methods.
     - Sets `window.location.hash` to the imported hash, triggering the existing router.
   - Expose both functions on `window` in a `setupStateExportImport()` helper.

3. **Wire into `src/main.ts`**
   - Import `setupStateExportImport` and call it after the app mounts so `window.__exportState` and `window.__importState` are available.

4. **Add unit tests**
   - Create `src/lib/stateExport.test.ts`.
   - Test that `exportState` captures a known set of store values.
   - Test that `importState` restores all stores and the hash.
   - Test round-trip identity for a representative state.
   - Test that invalid state objects are rejected gracefully.

## Open Questions

- Should `__importState` try to reconnect the WebSocket? The plan keeps `wsConnected` as a captured/restored value but does not initiate a connection; the existing auto-reconnect logic in `ws.ts` handles that.
- Should derived stores (`currentChat`, `isToolLoopRunning`, `activeScenariosList`) be exported? No — they are recomputed from the captured base stores.

## Acceptance Criteria

- `window.__exportState()` is callable from the console and returns a JSON-serializable object.
- `window.__importState(JSON.parse(JSON.stringify(window.__exportState())))` restores the UI to the same visual state.
- Unit tests pass with `cd frontend && ./node_modules/.bin/vitest run`.
