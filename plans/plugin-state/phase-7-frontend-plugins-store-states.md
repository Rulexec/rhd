# Phase 7: Frontend — PluginsStore State Management

> Parent plan: [`plans/plugin-state-exposure-plan.md`](../plugin-state-exposure-plan.md)

## Overview

Extend `PluginsStore` to hold all plugin states reactively: initial load via
`getPluginStates`, race-free subscription via `subscribePluginStates(refs)`,
version-gated application of live events, and computed getters that Phases 8–9
consume (`statesFor`, `statesWithSchema`, `hasMcpStatus`).

**Scope in:** `frontend/src/stores/PluginsStore.ts` + its tests.
**Scope out:** components (Phases 8–9).

**Dependencies:** Phase 6 (API functions + types). Blocks Phases 8–9.

## Files to Modify

### 1. `frontend/src/stores/PluginsStore.ts` (modify)

Imports: add `PluginState`, `MCP_STATUS_SCHEMA` from `../lib/api/schemas.js`
and `PluginStateEventHandlers` from `../lib/api/chatApiImpl.js` (type-only).

New public observable field (next to `_plugins` — MUST be public for MobX):

```typescript
  /** Raw plugin states (all plugins). Public so MobX can observe it. */
  _states: PluginState[] = [];
```

New private cleanup slot next to `#cleanupEvents`:

```typescript
  #cleanupStateEvents: (() => void) | null = null;
```

Extend `init()` — after the existing plugins-list setup and load:

```typescript
  *init(): Generator<unknown, void, unknown> {
    this.error = null;

    try {
      // Existing: subscribePluginsList + onPluginListEvents + loadPlugins
      ...existing code unchanged...

      // Register state event listeners BEFORE the snapshot read so no live
      // update is missed between load and subscribe (version gating makes
      // duplicates harmless).
      this.#cleanupStateEvents = this.#chatApi.onPluginStateEvents({
        onPluginStateChanged: ({ state }) => this.#applyState(state),
        onPluginStateRemoved: ({ pluginId, key, version }) =>
          this.#applyStateRemoval(pluginId, key, version)
      });

      // Race-free catch-up: snapshot first, then subscribe with the versions
      // we hold; the server returns anything that changed in between.
      const { states } = yield* yieldPromise(this.#chatApi.getPluginStates());
      this._states = states;
      const refs = states.map(s => ({ pluginId: s.pluginId, key: s.key, version: s.version }));
      const catchUp = yield* yieldPromise(this.#chatApi.subscribePluginStates(refs));
      for (const state of catchUp.states) {
        this.#applyState(state);
      }
    } catch (error) {
      this.error = error instanceof Error ? error.message : String(error);
    }
  }
```

Version-gated appliers (private methods; replace-array pattern like `_plugins`):

```typescript
  /** Apply a state change only if it is newer than what we hold. */
  #applyState(state: PluginState): void {
    const existing = this._states.find(
      s => s.pluginId === state.pluginId && s.key === state.key
    );
    if (existing && existing.version >= state.version) {
      return; // stale or duplicate (catch-up vs live event overlap)
    }
    if (existing) {
      this._states = this._states.map(s =>
        s.pluginId === state.pluginId && s.key === state.key ? state : s
      );
    } else {
      this._states = [...this._states, state];
    }
  }

  /** Drop a state only if the removal is at least as new as what we hold. */
  #applyStateRemoval(pluginId: string, key: string, version: number): void {
    const existing = this._states.find(s => s.pluginId === pluginId && s.key === key);
    if (existing && existing.version > version) {
      return; // a newer update already arrived — removal is stale
    }
    this._states = this._states.filter(
      s => !(s.pluginId === pluginId && s.key === key)
    );
  }
```

Hook plugin removal into the existing `#handlePluginRemoved`:

```typescript
  #handlePluginRemoved(pluginId: string): void {
    this._plugins = this._plugins.filter(p => p.pluginId !== pluginId);
    // Server cascades plugin_states on removePlugin; mirror locally.
    this._states = this._states.filter(s => s.pluginId !== pluginId);
  }
```

New computed getters:

```typescript
  /** Live states of one plugin, sorted by key. */
  statesFor(pluginId: string): PluginState[] {
    return this._states
      .filter(s => s.pluginId === pluginId)
      .sort((a, b) => a.key.localeCompare(b.key));
  }

  /** All live states published under a well-known schema (e.g. 'mcpStatus:1'). */
  statesWithSchema(schema: string): PluginState[] {
    return this._states.filter(s => s.schema === schema);
  }

  /** True while any plugin publishes an mcpStatus:1 state (drives the MCPs tab). */
  get hasMcpStatus(): boolean {
    return this._states.some(s => s.schema === MCP_STATUS_SCHEMA);
  }
```

Note `statesFor`/`statesWithSchema` are plain (non-getter) methods returning
derived arrays — MobX tracks the underlying `_states` read inside them when used
from `mobxObservable(() => store.statesFor(id))`. Keep `hasMcpStatus` a getter
(computed).

Extend `clear()`:

```typescript
  clear(): void {
    ...existing...
    if (this.#cleanupStateEvents) {
      this.#cleanupStateEvents();
      this.#cleanupStateEvents = null;
    }
    this._states = [];
  }
```

## Tests — `frontend/src/stores/PluginsStore.test.ts` (modify)

Extend the shared `mockChatApi` with the four new members (the existing file
casts `as unknown as ChatApi`, so add at least the ones exercised):

```typescript
      getPluginStates: vi.fn().mockResolvedValue({ states: [] }),
      subscribePluginStates: vi.fn().mockResolvedValue({ states: [] }),
      unsubscribePluginStates: vi.fn().mockResolvedValue(undefined),
      onPluginStateEvents: vi.fn().mockReturnValue(() => {}),
```

Fixture:

```typescript
  const mcpState: PluginState = {
    pluginId: 'mcp', key: 'status', content: '{"mcp":[]}',
    format: 'json', schema: 'mcpStatus:1', version: 1, updatedAt: '2026-09-05 22:41:07'
  };
```

New cases (capture handlers via `mockImplementation` like existing tests):

1. `loads states on init` — `getPluginStates` resolves `[mcpState]` →
   `store.statesFor('mcp')` contains it; `hasMcpStatus` true.
2. `subscribes with held versions` — after init,
   `subscribePluginStates` called with `[{pluginId:'mcp', key:'status', version:1}]`.
3. `applies catch-up response` — `subscribePluginStates` resolves a v2 state →
   store shows v2.
4. `applies live change event` — captured `onPluginStateChanged({state: v2})` →
   store updates to v2.
5. `ignores stale events` — after v2, deliver v1 → still v2.
6. `applies removal event` — `onPluginStateRemoved({pluginId, key, version:3})` →
   gone; `hasMcpStatus` false.
7. `ignores stale removal` — hold v5, removal v4 → state kept.
8. `drops states when plugin removed` — `onPluginRemoved({pluginId:'mcp'})` →
   `statesFor('mcp')` empty.
9. `clear() resets states and unsubscribes state events` — cleanup fn called,
   `_states` empty.
10. `init error path sets store.error` — `getPluginStates` rejects.

## Validation

```bash
cd frontend && nvm use && npm test -- PluginsStore
nvm use && ./node_modules/.bin/eslint --fix src/stores/PluginsStore.ts src/stores/PluginsStore.test.ts
npm run type-check
```

## Implementation Notes

1. **Order inside `init()` matters**: event listeners first, then snapshot, then
   subscribe-with-versions. This is the client half of the race-free guarantee;
   the server half (register-before-snapshot) is Phase 3.
2. **Version gating lives only in the store** — components render `store` state
   and never compare versions themselves.
3. **Removal gating uses `>` not `>=`**: a removal at version N supersedes a
   change at N (the remove bumps the version, Phase 2 rule 3), while a strictly
   newer change (N+1) wins over the removal.
4. **`_states` replaced immutably** (`map`/`filter`/spread) — MobX
   `makeAutoObservable` needs reference changes to notify, matching `_plugins`.
5. **`statesFor` as a method, not a getter** — a parameterized getter can't be
   a computed; components wrap calls in `mobxObservable(() => ...)` (Phase 8).

## Dependencies

- Depends on: Phase 6.
- Blocks: Phases 8, 9.
