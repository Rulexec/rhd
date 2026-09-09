# Phase 6: Frontend — Plugin State Zod Schemas & API Functions

> Parent plan: [`plans/plugin-state-exposure-plan.md`](../plugin-state-exposure-plan.md)

## Overview

Mirror the wire contract (Phase 1) in the frontend API layer: zod schemas for
states/events/`mcpStatus:1` payloads, request functions and an event-handler
helper in `chatApiImpl.ts`, and the `ChatApi` DI interface additions. No store
or UI changes in this phase.

**Scope in:** `schemas.ts`, `chatApiImpl.ts`, `ChatApi.ts`.
**Scope out:** `PluginsStore` (Phase 7), components (Phases 8–9).

**Dependencies:** Phase 1 (wire shapes). Blocks Phases 7–9.

## Files to Modify

### 1. `frontend/src/lib/api/schemas.ts` (modify)

Add after `PluginSummarySchema` (data-structures section):

```typescript
/**
 * Plugin state format discriminator (lowercase on the wire).
 */
export const PluginStateFormatSchema = z.enum(['markdown', 'json']);

/**
 * A single named state exposed by a plugin.
 * `version` is server-assigned and monotonic per (pluginId, key) —
 * consumers apply only strictly-newer versions.
 * `updatedAt` is an opaque SQLite datetime string ("YYYY-MM-DD HH:MM:SS" UTC).
 */
export const PluginStateSchema = z.object({
  pluginId: z.string(),
  key: z.string(),
  content: z.string(),
  format: PluginStateFormatSchema,
  schema: z.string(),
  version: z.number(),
  updatedAt: z.string()
});

/**
 * One entry of subscribePluginStates params: the version the client holds.
 */
export const StateVersionRefSchema = z.object({
  pluginId: z.string(),
  key: z.string(),
  version: z.number()
});
```

Add event-data schemas after `PluginRemovedDataSchema`:

```typescript
/**
 * Plugin state created/updated event data.
 */
export const PluginStateChangedDataSchema = z.object({
  state: PluginStateSchema
});

/**
 * Plugin state removed (tombstoned) event data.
 */
export const PluginStateRemovedDataSchema = z.object({
  pluginId: z.string(),
  key: z.string(),
  version: z.number()
});
```

Add method-result schemas after `GetPluginsResultSchema`:

```typescript
/**
 * Get plugin states result.
 */
export const GetPluginStatesResultSchema = z.object({
  states: z.array(PluginStateSchema)
});

/**
 * Subscribe plugin states result — catch-up states newer than requested.
 */
export const SubscribePluginStatesResultSchema = z.object({
  states: z.array(PluginStateSchema)
});
```

Add the well-known `mcpStatus:1` payload schema (new section after Tools):

```typescript
// ============================================================================
// Well-Known State Schemas
// ============================================================================

/** Schema id published by rhd_plugin_mcp. */
export const MCP_STATUS_SCHEMA = 'mcpStatus:1';

/**
 * One MCP server entry inside an mcpStatus:1 state's content.
 */
export const McpStatusEntrySchema = z.object({
  id: z.string(),
  name: z.string(),
  status: z.enum(['ok', 'error']),
  error: z.string().optional()
});

/**
 * Parsed content of an mcpStatus:1 state.
 */
export const McpStatusPayloadSchema = z.object({
  mcp: z.array(McpStatusEntrySchema)
});
```

Type exports (append to the Type Exports section):

```typescript
export type PluginStateFormat = z.infer<typeof PluginStateFormatSchema>;
export type PluginState = z.infer<typeof PluginStateSchema>;
export type StateVersionRef = z.infer<typeof StateVersionRefSchema>;
export type GetPluginStatesResult = z.infer<typeof GetPluginStatesResultSchema>;
export type SubscribePluginStatesResult = z.infer<typeof SubscribePluginStatesResultSchema>;
export type PluginStateChangedData = z.infer<typeof PluginStateChangedDataSchema>;
export type PluginStateRemovedData = z.infer<typeof PluginStateRemovedDataSchema>;
export type McpStatusEntry = z.infer<typeof McpStatusEntrySchema>;
export type McpStatusPayload = z.infer<typeof McpStatusPayloadSchema>;
```

### 2. `frontend/src/lib/api/chatApiImpl.ts` (modify)

Extend the schemas import list with the new schemas/types. After the
`onPluginListEvents` function (before "Tools API Methods") add:

```typescript
// ============================================================================
// Plugin State API Methods
// ============================================================================

/**
 * Get plugin states, optionally filtered by pluginId and/or schema.
 */
export async function getPluginStates(filters?: {
  pluginId?: string;
  schema?: string;
}): Promise<GetPluginStatesResult> {
  const data = await websocket.request('getPluginStates', filters ?? {});
  return GetPluginStatesResultSchema.parse(data);
}

/**
 * Subscribe to plugin state events and atomically catch up on states newer
 * than the passed versions. Pass `[]` to subscribe without catch-up.
 * The server registers the subscription before snapshotting, so no update
 * can be lost between getPluginStates and this call (duplicates are handled
 * by version-gating in the store).
 */
export async function subscribePluginStates(
  refs: StateVersionRef[]
): Promise<SubscribePluginStatesResult> {
  const data = await websocket.request('subscribePluginStates', { states: refs });
  return SubscribePluginStatesResultSchema.parse(data);
}

/**
 * Unsubscribe from plugin state events.
 */
export async function unsubscribePluginStates(): Promise<void> {
  await websocket.request('unsubscribePluginStates', {});
}

export interface PluginStateEventHandlers {
  onPluginStateChanged?: (data: PluginStateChangedData) => void;
  onPluginStateRemoved?: (data: PluginStateRemovedData) => void;
}

/**
 * Register event listeners for plugin state events.
 * Returns cleanup function.
 */
export function onPluginStateEvents(handlers: PluginStateEventHandlers): () => void {
  const unsubs: Array<() => void> = [];

  if (handlers.onPluginStateChanged) {
    unsubs.push(websocket.on('pluginStateChanged', (data) => {
      const parsed = PluginStateChangedDataSchema.parse(data);
      handlers.onPluginStateChanged!(parsed);
    }));
  }

  if (handlers.onPluginStateRemoved) {
    unsubs.push(websocket.on('pluginStateRemoved', (data) => {
      const parsed = PluginStateRemovedDataSchema.parse(data);
      handlers.onPluginStateRemoved!(parsed);
    }));
  }

  return () => {
    unsubs.forEach(unsub => unsub());
  };
}
```

### 3. `frontend/src/lib/api/ChatApi.ts` (modify)

Interface additions (after `onPluginListEvents`):

```typescript
  getPluginStates: typeof chatApi.getPluginStates;
  subscribePluginStates: typeof chatApi.subscribePluginStates;
  unsubscribePluginStates: typeof chatApi.unsubscribePluginStates;
  onPluginStateEvents: typeof chatApi.onPluginStateEvents;
```

`defaultChatApi` additions (same position):

```typescript
  getPluginStates: chatApi.getPluginStates,
  subscribePluginStates: chatApi.subscribePluginStates,
  unsubscribePluginStates: chatApi.unsubscribePluginStates,
  onPluginStateEvents: chatApi.onPluginStateEvents,
```

## Tests

`chatApiImpl` has no direct unit tests today (only `websocket.test.ts`). Verify:

```bash
cd frontend && nvm use && npm run type-check
```

(No new runtime tests in this phase; Phase 7 tests exercise these functions via
the mocked `ChatApi`.)

## Implementation Notes

1. **`MCP_STATUS_SCHEMA` constant lives in `schemas.ts`** so both the store
   getter and `McpStatusList` import the same literal — no drift.
2. **Event handlers parse with zod** exactly like `onPluginListEvents`; a
   malformed payload throws inside the websocket listener (existing behavior —
   `handleEvent` catches nothing, so keep the throw; the store must not be
   corrupted because handlers run after parse).
3. **`filters ?? {}`** — the server accepts `{}` (Phase 1 `#[serde(default)]`).
4. **Do not add `updatePluginState`/`removePluginState`** to the frontend API —
   the UI never writes plugin state; keep the interface minimal.

## Dependencies

- Depends on: Phase 1 wire contract (source of truth for field names).
- Blocks: Phase 7 (store), Phase 9 (payload parsing).
