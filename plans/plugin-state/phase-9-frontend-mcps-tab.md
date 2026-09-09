# Phase 9: Frontend — Conditional `MCPs` Tab

> Parent plan: [`plans/plugin-state-exposure-plan.md`](../plugin-state-exposure-plan.md)

## Overview

Add a third top-level tab **MCPs**, rendered only while at least one plugin
publishes a state with `schema: "mcpStatus:1"` (`pluginsStore.hasMcpStatus`).
The tab lists every such state (grouped by plugin), parses its content against
`McpStatusPayloadSchema`, and shows each MCP server with an ok/error status and
the error message when present.

**Scope in:** `TabView.svelte`, `App.svelte`, new `McpStatusList.svelte` + test.
**Scope out:** store/API changes (Phases 6–7).

**Dependencies:** Phases 6 (`MCP_STATUS_SCHEMA`, payload schemas) and 7
(`statesWithSchema`, `hasMcpStatus`).

## Files to Modify/Create

### 1. `frontend/src/lib/components/TabView.svelte` (modify)

```typescript
  type TabType = 'chats' | 'plugins' | 'mcps';

  interface Props {
    activeTab?: TabType;
    /** Show the MCPs tab only while some plugin publishes mcpStatus:1. */
    showMcps?: boolean;
  }

  let { activeTab = 'chats', showMcps = false }: Props = $props();
```

After the Plugins button:

```svelte
    {#if showMcps}
      <button
        class="tab-button"
        class:active={activeTab === 'mcps'}
        role="tab"
        aria-selected={activeTab === 'mcps'}
        onclick={() => selectTab('mcps')}
      >
        MCPs
      </button>
    {/if}
```

Export `TabType` (`export type TabType = ...`) so `App.svelte` and tests share it.

### 2. `frontend/src/lib/components/McpStatusList.svelte` (new)

```svelte
<script lang="ts">
  import { getAppStore } from '../../context.js';
  import { mobxObservable } from '../../util/mobxObservable.svelte.js';
  import {
    McpStatusPayloadSchema,
    MCP_STATUS_SCHEMA,
    type McpStatusEntry,
    type PluginState
  } from '../api/schemas.js';
  import commonStyles from '../styles/common.module.css';

  const appStore = getAppStore();
  const pluginsStore = appStore.plugins;

  const statesGetter = mobxObservable(() => pluginsStore.statesWithSchema(MCP_STATUS_SCHEMA));
  const pluginsGetter = mobxObservable(() => pluginsStore.plugins);
  let states = $derived(statesGetter());
  let plugins = $derived(pluginsGetter());

  interface ParsedGroup {
    pluginId: string;
    isActive: boolean;
    state: PluginState;
    entries: McpStatusEntry[] | null; // null = content failed to parse
    rawContent: string;
  }

  function parseGroups(states: PluginState[], plugins: { pluginId: string; isActive: boolean }[]): ParsedGroup[] {
    return states.map(state => {
      let entries: McpStatusEntry[] | null = null;
      try {
        entries = McpStatusPayloadSchema.parse(JSON.parse(state.content)).mcp;
      } catch {
        entries = null;
      }
      return {
        pluginId: state.pluginId,
        isActive: plugins.find(p => p.pluginId === state.pluginId)?.isActive ?? false,
        state,
        entries,
        rawContent: state.content
      };
    });
  }

  let groups = $derived(parseGroups(states, plugins));
</script>

<div class="mcp-status-list">
  <div class="mcp-status-header">
    <h2>MCP Servers</h2>
    <span class="{commonStyles['text-muted']}">{groups.length} source(s)</span>
  </div>

  {#if groups.length === 0}
    <div class="mcp-status-empty">
      <div class="empty-icon">🔌</div>
      <p class="{commonStyles['text-muted']}">No plugin is publishing MCP status</p>
    </div>
  {:else}
    {#each groups as group (group.pluginId + ':' + group.state.key)}
      <section class="mcp-group">
        <h3 class="mcp-group-title">
          {group.pluginId}
          <span class="mcp-group-version">v{group.state.version}</span>
          {#if !group.isActive}
            <span class="mcp-group-stale">last known</span>
          {/if}
        </h3>
        {#if group.entries === null}
          <div class="mcp-parse-error">
            <p class="{commonStyles['text-error']}">Unrecognized mcpStatus:1 payload — raw content:</p>
            <pre>{group.rawContent}</pre>
          </div>
        {:else}
          <ul class="{commonStyles['list']}">
            {#each group.entries as entry (entry.id)}
              <li class="{commonStyles['list-item']} mcp-entry" class:mcp-entry-error={entry.status === 'error'}>
                <span class="mcp-entry-name">{entry.name}</span>
                <span class="mcp-entry-id {commonStyles['text-muted']}">{entry.id}</span>
                <span class="mcp-entry-status" class:ok={entry.status === 'ok'} class:error={entry.status === 'error'}>
                  <span class="{commonStyles['status-dot']} {entry.status === 'ok' ? commonStyles['status-dot-active'] : commonStyles['status-dot-inactive']}"></span>
                  {entry.status === 'ok' ? 'OK' : 'Error'}
                </span>
                {#if entry.error}
                  <div class="mcp-entry-error-msg">{entry.error}</div>
                {/if}
              </li>
            {/each}
          </ul>
        {/if}
      </section>
    {/each}
  {/if}
</div>
```

Scoped styles mirroring `PluginList.svelte` (header bar, list layout, error row
highlight with `var(--color-error)` if the token exists — otherwise reuse
`commonStyles['text-error']`).

### 3. `frontend/src/App.svelte` (modify)

```typescript
  import McpStatusList from './lib/components/McpStatusList.svelte';

  // Bridge hasMcpStatus for tab visibility (top-level call — required).
  const hasMcpStatusGetter = mobxObservable(() => appStore.plugins.hasMcpStatus);
  let hasMcpStatus = $derived(hasMcpStatusGetter());
```

```svelte
  <TabView {activeTab} showMcps={hasMcpStatus} on:tabChange={handleTabChange} />
```

Main content:

```svelte
    {:else if activeTab === 'plugins'}
      <PluginList />
    {:else if activeTab === 'mcps'}
      <McpStatusList />
    {/if}
```

Edge case: if the last `mcpStatus:1` state disappears while the MCPs tab is
open, `showMcps` flips false and the tab button vanishes, but `activeTab`
remains `'mcps'` ⇒ main renders nothing. Add a guard effect:

```typescript
  $effect(() => {
    if (activeTab === 'mcps' && !hasMcpStatus) {
      activeTab = 'plugins';
    }
  });
```

### 4. `frontend/src/lib/components/McpStatusList.test.ts` (new)

Use the harness pattern (new `McpStatusListHarness.svelte` copying
`PluginListHarness.svelte`) with a real `PluginsStore` backed by a mocked
`ChatApi` (same approach as Phase 8 tests).

Cases:

1. `renders server rows from parsed payload` — one state
   `{"mcp":[{"id":"fs","name":"filesystem","status":"ok"},
   {"id":"bad","name":"broken","status":"error","error":"spawn failed"}]}` →
   two rows; "Error" text + error message present for the second; absent for first.
2. `multiple plugins grouped` — two states from plugins `mcp` and `mcp2` →
   two `<section>`s with plugin ids.
3. `unparseable content falls back to raw view` — content `garbage` →
   `.mcp-parse-error` with raw text.
4. `reactive to state update` — push v2 state via captured event handler →
   rows re-render (e.g. previously-error server now ok).
5. `inactive plugin marked last known`.

`TabView.test.ts` (new or extend): `showMcps=false` → no MCPs button;
`showMcps=true` → button present and dispatches `tabChange {tab:'mcps'}`.

## Validation

```bash
cd frontend && nvm use && npm test -- McpStatusList TabView
nvm use && ./node_modules/.bin/eslint --fix src/lib/components/McpStatusList.svelte src/App.svelte src/lib/components/TabView.svelte
npm run type-check
```

## Implementation Notes

1. **Visibility is derived, not stored** — `hasMcpStatus` comes from live store
   state, so the tab appears/disappears reactively with state pushes/removals
   (no extra subscription).
2. **Parse per render is cheap** (few servers); `$derived` caches until inputs
   change. No memoization needed.
3. **Group key `pluginId + ':' + key`** — matches the server-side identity.
4. **`entries === null` distinguishes "parse failed" from empty list** —
   `{"mcp":[]}` is valid (plugin running, zero servers configured healthy —
   shouldn't happen since config requires ≥1, but handle: renders empty list).
5. **The tab label is `MCPs`** exactly as requested; aria/role parity with
   existing buttons.

## Dependencies

- Depends on: Phases 6, 7.
- Blocks: nothing.
