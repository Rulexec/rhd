# Phase 8: Frontend — Plugin States Section on the Plugins Tab

> Parent plan: [`plans/plugin-state-exposure-plan.md`](../plugin-state-exposure-plan.md)

## Overview

Under each plugin entry in `PluginList.svelte`, render that plugin's states in a
**collapsed-by-default** section. Each state shows `key`, format + schema +
version badges, and content rendered per `format`: markdown via `marked`,
JSON pretty-printed (parse-guarded). States of inactive plugins are marked
"last known". Reactivity comes from the store (Phase 7) — no direct API calls.

**Scope in:** `PluginList.svelte` (+ small addition to `common.module.css` if
needed), new `PluginList.test.ts` + `PluginListHarness.svelte`.
**Scope out:** MCPs tab (Phase 9).

**Dependencies:** Phase 7 (`statesFor`). Blocks: nothing (parallel with Phase 9).

## Files to Modify/Create

### 1. `frontend/src/lib/components/PluginList.svelte` (modify)

Script additions (top level, per the mobxObservable rule — must run during
component init):

```typescript
  import { marked } from 'marked';
  import type { PluginState } from '../api/schemas.js';
  import { getAppStore } from '../../context.js';

  // Per-plugin state lists are derived inside the each-block via a helper that
  // reads the observable store; mobxObservable wraps the whole component read:
  const statesSnapshotGetter = mobxObservable(() => pluginsStore._states);
  let allStates = $derived(statesSnapshotGetter());

  function statesFor(pluginId: string): PluginState[] {
    return allStates
      .filter(s => s.pluginId === pluginId)
      .sort((a, b) => a.key.localeCompare(b.key));
  }

  function renderMarkdown(content: string): string {
    try {
      return marked.parse(content, { breaks: true }) as string;
    } catch {
      return content;
    }
  }

  function prettyJson(content: string): string {
    try {
      return JSON.stringify(JSON.parse(content), null, 2);
    } catch {
      return content; // malformed json — show raw rather than crash
    }
  }
```

(Reading `pluginsStore._states` once and filtering per plugin keeps a single
`mobxObservable` subscription; `statesFor` is then a pure derived helper.)

Template — inside the `{#each plugins as plugin (plugin.pluginId)}` item, after
the existing `.plugin-item-content` div, still within the `<li>`:

```svelte
          {@const states = statesFor(plugin.pluginId)}
          {#if states.length > 0}
            <details class="plugin-states">
              <summary class="plugin-states-summary">
                State ({states.length})
              </summary>
              {#each states as state (state.key)}
                <div class="plugin-state">
                  <div class="plugin-state-header">
                    <span class="plugin-state-key">{state.key}</span>
                    <span class="plugin-state-badge">{state.format}</span>
                    {#if state.schema}
                      <span class="plugin-state-badge plugin-state-schema">{state.schema}</span>
                    {/if}
                    <span class="plugin-state-version">v{state.version}</span>
                    {#if !plugin.isActive}
                      <span class="plugin-state-stale">last known</span>
                    {/if}
                  </div>
                  {#if state.format === 'markdown'}
                    <div class="plugin-state-content markdown">{@html renderMarkdown(state.content)}</div>
                  {:else}
                    <pre class="plugin-state-content plugin-state-json">{prettyJson(state.content)}</pre>
                  {/if}
                </div>
              {/each}
            </details>
          {/if}
```

`<details>` with no `open` attribute ⇒ collapsed by default (native behavior,
no Svelte state needed).

Styles (scoped, following existing conventions in the file): `.plugin-states`
(padding-left indent, border-top), `.plugin-states-summary` (muted, cursor
pointer), `.plugin-state-badge` (small pill), `.plugin-state-version` /
`.plugin-state-stale` (muted small), `.plugin-state-json` (monospace,
`overflow-x: auto`, `background: var(--color-bg-secondary)`).

### 2. `frontend/src/lib/components/PluginListHarness.svelte` (new)

Mirror `ChatListHarness.svelte`:

```svelte
<script lang="ts">
  import PluginList from './PluginList.svelte';
  import { setAppStore } from '../../context.js';
  import type { AppStore } from '../../stores/AppStore.js';

  interface Props {
    appStore: AppStore;
  }

  let { appStore }: Props = $props();

  (() => {
    setAppStore(appStore);
  })();
</script>

<PluginList />
```

### 3. `frontend/src/lib/components/PluginList.test.ts` (new)

Pattern: mock `AppStore` with a fake `plugins` substore object exposing
`_states`, `plugins`, `hasPlugins`, `loading`, `error`,
`activePluginsCount` as plain fields (component reads `_states` through
`mobxObservable`, which works with plain objects too — verify; if the bridge
requires observability, wrap the mock with `makeObservable` or return a real
`PluginsStore` built with a mocked `ChatApi`, as `PluginsStore.test.ts` does —
**preferred: real `PluginsStore` + mocked ChatApi** for fidelity).

Cases:

1. `renders no state section for plugin without states` — `getPluginStates`
   resolves `[]` → no `<details>`.
2. `state section collapsed by default` — one markdown state →
   `details:not([open])`; summary text `State (1)`.
3. `markdown content rendered after expand` — set content
   `# Hello` → after `details.open = true` (or `fireEvent.click(summary)`),
   `.markdown h1` exists with text `Hello`.
4. `json content pretty-printed` — content `{"a":1}` → `<pre>` contains
   `{\n  "a": 1\n}`.
5. `malformed json falls back to raw` — content `not-json` → `<pre>` shows
   `not-json` (no throw).
6. `badges show format, schema and version` — `json`, `mcpStatus:1`, `v3`
   present.
7. `inactive plugin states marked last known` — plugin `isActive: false` →
   `.plugin-state-stale` present.
8. `reactive update` — after initial render, push a new state into the store's
   `_states` (via mocked `onPluginStateEvents` handler captured in the mock) →
   section appears without re-render setup.

## Validation

```bash
cd frontend && nvm use && npm test -- PluginList
nvm use && ./node_modules/.bin/eslint --fix src/lib/components/PluginList.svelte src/lib/components/PluginList.test.ts
npm run type-check
```

## Implementation Notes

1. **`{@html}` safety**: plugin-authored content is trusted-local (same threat
   model as chat `Message.svelte`, which already uses `marked` + `{@html}` with
   no sanitizer). Do not introduce DOMPurify here; note it as a future hardening
   item if untrusted plugins ever appear.
2. **`marked.parse` with `breaks: true`** — exact parity with
   [`Message.svelte`](../../frontend/src/lib/components/Message.svelte:69).
3. **Keyed each** (`(state.key)`) — state identity is `(pluginId, key)`; within
   one plugin, `key` is unique.
4. **One `mobxObservable` on `_states`** instead of per-plugin getters — avoids
   registering N subscriptions inside an each-block (mobxObservable must be
   top-level; see frontend MEMORY rules).
5. **`{@const}` inside the `<li>`** keeps `statesFor(plugin.pluginId)` computed
   once per item.

## Dependencies

- Depends on: Phase 7 (store shape).
- Blocks: nothing.
