# Phase 4: App Shell, Chats List, Refresh Button

## Overview

Build the visible application frame on top of the Phase 3 store:

- **`App.svelte`** — real shell replacing the Phase 1 placeholder: header with the global
  **Refresh** button, two-pane layout (chats sidebar + main pane), store creation + context
  provision, initial `loadChats` on mount.
- **`ChatsList.svelte`** — the sidebar: chat summaries (title, model, relative time, request
  count), selection highlight, loading / error / empty states, `chatSelect` event.
- **`RefreshButton.svelte`** — the header button that calls `store.refresh()`, disabled + labeled
  while a refresh is running.
- **`StatusMessage.svelte`** — shared loading/error/empty display used by the sidebar now and by
  the main pane in Phase 5.

After this phase the tool is usable for monitoring **which chats passed through the proxy**; the
main pane still shows a placeholder where Phase 5 mounts the chat detail.

**Component conventions (verified in `frontend/src/lib/components/ChatList.svelte`):** props via
`let { ... }: Props = $props()`; events via `createEventDispatcher`; store access via
`getProxyLogsStore()` from context; MobX bridging via top-level `mobxObservable(() => ...)` calls
bound with `let x = $derived(xGetter())`; shared classes from
`frontend-proxy/src/lib/styles/common.module.css` (`btn`, `btn-primary`, `btn-sm`, `list`,
`list-item`, `active`, `text-muted`, `text-error`, `truncate`); Svelte-scoped styles for the
rest; `onclick` (Svelte 5 event attribute form).

## Files to Create

### 1. `frontend-proxy/src/lib/components/StatusMessage.svelte`

```svelte
<script lang="ts">
  interface Props {
    loading: boolean;
    error: string | null;
    empty: boolean;
    loadingText?: string;
    emptyText?: string;
  }

  let {
    loading,
    error,
    empty,
    loadingText = 'Loading...',
    emptyText = 'Nothing here yet'
  }: Props = $props();
</script>

{#if loading}
  <div class="status-message" data-testid="status-loading">
    <span class="text-muted">{loadingText}</span>
  </div>
{:else if error}
  <div class="status-message status-error" data-testid="status-error">
    <span class="text-error">{error}</span>
  </div>
{:else if empty}
  <div class="status-message" data-testid="status-empty">
    <span class="text-muted">{emptyText}</span>
  </div>
{/if}

<style>
  .status-message {
    padding: var(--spacing-lg);
    text-align: center;
  }

  .status-error {
    padding: var(--spacing-sm) var(--spacing-md);
    background: var(--color-error-bg);
    border-bottom: 1px solid var(--color-error);
    font-size: var(--font-size-sm);
  }
</style>
```

Props are plain values (not store-bound) so both store-driven parents and Phase 5's
request-detail area can reuse it. Parents render it **inside** an `{#if}` chain ahead of
content, exactly like `ChatList.svelte`'s loading/error/empty cascade.

### 2. `frontend-proxy/src/lib/components/RefreshButton.svelte`

```svelte
<script lang="ts">
  import { flowResult } from 'mobx';
  import { getProxyLogsStore } from '../../context.js';
  import { mobxObservable } from '../../util/mobxObservable.svelte.js';
  import commonStyles from '../styles/common.module.css';

  const store = getProxyLogsStore();

  const refreshingGetter = mobxObservable(() => store.refreshing);
  let refreshing = $derived(refreshingGetter());

  function handleRefresh(): void {
    // Fire-and-forget: errors land in store.chatsError and render as banners.
    void flowResult(store.refresh()).catch(() => {});
  }
</script>

<button
  class="{commonStyles['btn']} {commonStyles['btn-primary']} {commonStyles['btn-sm']}"
  disabled={refreshing}
  onclick={handleRefresh}
  data-testid="refresh-button"
>
  {refreshing ? 'Refreshing…' : '↻ Refresh'}
</button>
```

### 3. `frontend-proxy/src/lib/components/ChatsList.svelte`

Modeled directly on `frontend/src/lib/components/ChatList.svelte` (event dispatch, formatTime,
list markup, cascade states) minus the create/delete actions — this list is read-only.

```svelte
<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { getProxyLogsStore } from '../../context.js';
  import { mobxObservable } from '../../util/mobxObservable.svelte.js';
  import StatusMessage from './StatusMessage.svelte';
  import commonStyles from '../styles/common.module.css';

  interface Props {
    selectedChatId?: number | null;
  }

  let { selectedChatId = null }: Props = $props();

  const dispatch = createEventDispatcher<{
    chatSelect: { chatId: number };
  }>();

  const store = getProxyLogsStore();

  // Bridge MobX observables to Svelte reactivity (top-level calls required).
  const chatsGetter = mobxObservable(() => store.chats);
  const hasChatsGetter = mobxObservable(() => store.hasChats);
  const loadingGetter = mobxObservable(() => store.chatsLoading);
  const errorGetter = mobxObservable(() => store.chatsError);

  let chats = $derived(chatsGetter());
  let hasChats = $derived(hasChatsGetter());
  let loading = $derived(loadingGetter());
  let error = $derived(errorGetter());

  function handleChatClick(chatId: number): void {
    dispatch('chatSelect', { chatId });
  }

  function formatTime(dateString: string): string {
    const date = new Date(dateString);
    const diffMs = Date.now() - date.getTime();
    const diffMins = Math.floor(diffMs / 60000);
    const diffHours = Math.floor(diffMs / 3600000);
    const diffDays = Math.floor(diffMs / 86400000);
    if (diffMins < 1) return 'just now';
    if (diffMins < 60) return `${diffMins}m ago`;
    if (diffHours < 24) return `${diffHours}h ago`;
    if (diffDays < 7) return `${diffDays}d ago`;
    return date.toLocaleDateString();
  }
</script>

<div class="chats-list">
  <div class="chats-list-header">
    <span class="text-muted">Chats</span>
  </div>

  <StatusMessage
    {loading}
    {error}
    empty={!hasChats}
    loadingText="Loading chats..."
    emptyText="No logged chats yet — send traffic through rhd_ai_proxy and press Refresh"
  />

  {#if !loading && !error && hasChats}
    <ul class="{commonStyles['list']} chats-list-items">
      {#each chats as chat (chat.id)}
        <li
          class="{commonStyles['list-item']} {selectedChatId === chat.id ? commonStyles['active'] : ''}"
          onclick={() => handleChatClick(chat.id)}
          onkeydown={(e) => e.key === 'Enter' && handleChatClick(chat.id)}
          role="button"
          tabindex="0"
          aria-selected={selectedChatId === chat.id}
        >
          <div class="chat-item-content">
            <div class="chat-item-title truncate">{chat.title}</div>
            <div class="chat-item-meta">
              {#if chat.model}
                <span class="chat-item-model truncate">{chat.model}</span>
              {/if}
              <span class="text-muted chat-item-count">{chat.requestCount} req</span>
              <span class="text-muted chat-item-time">{formatTime(chat.updatedAt)}</span>
            </div>
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .chats-list {
    display: flex;
    flex-direction: column;
    height: 100%;
    border-right: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
  }

  .chats-list-header {
    padding: var(--spacing-md);
    border-bottom: 1px solid var(--color-border);
    font-size: var(--font-size-sm);
    font-weight: 500;
  }

  .chats-list-items {
    flex: 1;
    overflow-y: auto;
  }

  .chat-item-content {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-xs);
  }

  .chat-item-title {
    font-weight: 500;
    font-size: var(--font-size-sm);
  }

  .chat-item-meta {
    display: flex;
    align-items: center;
    gap: var(--spacing-sm);
    font-size: var(--font-size-xs);
  }

  .chat-item-model {
    max-width: 140px;
    color: var(--color-text-secondary);
  }

  .chat-item-count,
  .chat-item-time {
    white-space: nowrap;
  }
</style>
```

### 4. `frontend-proxy/src/App.svelte` — replace the Phase 1 placeholder

```svelte
<script lang="ts">
  import { onMount } from 'svelte';
  import { defaultProxyLogsApi } from './lib/api/ProxyLogsApi.js';
  import { ProxyLogsStore } from './stores/ProxyLogsStore.js';
  import { setProxyLogsStore } from './context.js';
  import ChatsList from './lib/components/ChatsList.svelte';
  import RefreshButton from './lib/components/RefreshButton.svelte';

  const store = new ProxyLogsStore({ api: defaultProxyLogsApi });
  setProxyLogsStore(store);

  let selectedChatId: number | null = $state(null);

  onMount(() => {
    void store.loadChats();
  });

  async function handleChatSelect(event: CustomEvent<{ chatId: number }>) {
    selectedChatId = event.detail.chatId;
    await store.openChat(event.detail.chatId);
  }
</script>

<div class="app">
  <header class="app-header">
    <h1>RHD Proxy Logs</h1>
    <RefreshButton />
  </header>

  <main class="app-main">
    <div class="viewer-layout">
      <aside class="viewer-sidebar">
        <ChatsList {selectedChatId} on:chatSelect={handleChatSelect} />
      </aside>
      <section class="viewer-content">
        {#if selectedChatId === null}
          <div class="viewer-placeholder">
            <span class="text-muted">Select a chat to inspect its conversation and raw logs.</span>
          </div>
        {:else}
          <!-- ChatDetailView mounts here in Phase 5. -->
          <div class="viewer-placeholder">
            <span class="text-muted">Chat detail view arrives in Phase 5.</span>
          </div>
        {/if}
      </section>
    </div>
  </main>
</div>

<style>
  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: var(--color-bg);
    color: var(--color-text);
  }

  .app-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--spacing-md);
    border-bottom: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
  }

  .app-header h1 {
    margin: 0;
    font-size: var(--font-size-lg);
  }

  .app-main {
    flex: 1;
    overflow: hidden;
  }

  .viewer-layout {
    display: flex;
    height: 100%;
  }

  .viewer-sidebar {
    width: 300px;
    flex-shrink: 0;
  }

  .viewer-content {
    flex: 1;
    overflow: hidden;
  }

  .viewer-placeholder {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
  }
</style>
```

### 5. Test harnesses (pattern from `frontend/src/lib/components/ChatListHarness.svelte`)

**`frontend-proxy/src/lib/components/ChatsListHarness.svelte`:**

```svelte
<script lang="ts">
  import ChatsList from './ChatsList.svelte';
  import { setProxyLogsStore } from '../../context.js';
  import type { ProxyLogsStore } from '../../stores/ProxyLogsStore.js';

  interface Props {
    store: ProxyLogsStore;
    selectedChatId?: number | null;
    onChatSelect?: (detail: { chatId: number }) => void;
  }

  let { store, selectedChatId = null, onChatSelect }: Props = $props();

  // Provide the mock store via Svelte context before the target renders.
  (() => {
    setProxyLogsStore(store);
  })();
</script>

<ChatsList {selectedChatId} on:chatSelect={(e) => onChatSelect?.(e.detail)} />
```

**`frontend-proxy/src/lib/components/RefreshButtonHarness.svelte`:**

```svelte
<script lang="ts">
  import RefreshButton from './RefreshButton.svelte';
  import { setProxyLogsStore } from '../../context.js';
  import type { ProxyLogsStore } from '../../stores/ProxyLogsStore.js';

  interface Props {
    store: ProxyLogsStore;
  }

  let { store }: Props = $props();

  (() => {
    setProxyLogsStore(store);
  })();
</script>

<RefreshButton />
```

### 6. `frontend-proxy/src/lib/components/ChatsList.test.ts`

Pattern from `frontend/src/lib/components/ChatList.test.ts`: build a mock store **object** with
plain getters over static data (mobxObservable's autorun runs once against plain getters, which
is enough for static render assertions), cast `as unknown as ProxyLogsStore`, render the harness.

```typescript
import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, cleanup, fireEvent } from '@testing-library/svelte';
import ChatsListHarness from './ChatsListHarness.svelte';
import type { ProxyLogsStore } from '../../stores/ProxyLogsStore.js';
import type { ChatSummary } from '../api/schemas.js';

const chat = (id: number, overrides: Partial<ChatSummary> = {}): ChatSummary => ({
  id,
  title: `Chat ${id}`,
  model: 'z-ai/glm-5.3',
  createdAt: '2026-09-27T10:00:00Z',
  updatedAt: '2026-09-27T10:05:00Z',
  requestCount: 3,
  ...overrides
});

function createMockStore(options: {
  chats: ChatSummary[];
  loading?: boolean;
  error?: string | null;
}) {
  const store = {
    _chats: options.chats,
    chatsLoading: options.loading ?? false,
    chatsError: options.error ?? null,
    get chats() {
      return [...this._chats];
    },
    get hasChats() {
      return this._chats.length > 0;
    }
  } as unknown as ProxyLogsStore;
  return { store };
}

afterEach(() => {
  cleanup();
});
```

**Test cases:**

1. Renders empty state text (`No logged chats yet`) when no chats and not loading.
2. Renders loading state (`Loading chats...`) when `loading: true` (and the list hidden).
3. Renders error text when `error` set.
4. Renders chats with title, model, and `3 req` count.
5. Highlights the selected chat (`aria-selected="true"` on the row whose id matches
   `selectedChatId`).
6. Clicking a chat row dispatches `chatSelect` with `{ chatId }` (assert via harness
   `onChatSelect` prop, mirroring the frontend's test).
7. A chat with `model: null` renders without the model span (no crash).

### 7. `frontend-proxy/src/lib/components/RefreshButton.test.ts`

```typescript
import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, cleanup, fireEvent } from '@testing-library/svelte';
import RefreshButtonHarness from './RefreshButtonHarness.svelte';
import { ProxyLogsStore } from '../../stores/ProxyLogsStore.js';
import { defaultProxyLogsApi } from '../../lib/api/ProxyLogsApi.js';
```

**Test cases:**

1. Renders enabled with label `↻ Refresh` when idle.
2. Clicking calls the real store's `refresh` — spy approach: construct a real `ProxyLogsStore`
   with `{ api: defaultProxyLogsApi }`, monkey-patch
   `store.refresh = vi.fn().mockReturnValue(Promise.resolve())`-style mock (cast as needed), render
   harness with it, click, assert called. (Simplest reliable route; alternatively mock the api.)
3. While `refreshing` is true the button is disabled and shows `Refreshing…` — drive by rendering
   with a mock store object `{ refreshing: true, refresh: vi.fn() }` cast to the store type.

### 8. `frontend-proxy/src/lib/components/StatusMessage.test.ts`

**Test cases (plain props, no context needed — render the component directly):**

1. `loading: true` → shows loading text; custom `loadingText` used when provided.
2. `error: 'boom'` → shows the error text (with `data-testid="status-error"`).
3. `empty: true` (loading false, error null) → shows empty text; custom `emptyText` respected.
4. All flags false/empty-false → renders nothing (container query returns empty).

## Files to Modify

None — `App.svelte` is fully replaced (it was a placeholder), everything else is new.

## Implementation Notes

1. **`mobxObservable` with mock stores:** autorun against plain-object getters does not track
   changes (no MobX observables) — fine for static render tests; for behavior that changes over
   time (refresh spinning), render with the desired static state instead of mutating.
2. **Refresh is fire-and-forget:** `void flowResult(...)` in the button; failures already land in
   `store.chatsError` / `detailError` and render as banners — components never `alert()`.
3. **Global refresh placement:** the button lives in the header (not the sidebar) because
   `refresh()` re-fetches both the list and the open selection — it is a whole-app action.
4. **`selectedChatId` mirrors the store:** App keeps a local `$state` copy for prop-passing to
   `ChatsList` (the store remains the source of truth for detail data; the local copy avoids
   bridging boilerplate for one prop). Phase 5 may read `store.selectedChatId` directly instead —
   either is acceptable, but pick one and keep it (prefer the local mirror to match the frontend's
   App/ChatList prop flow).
5. **Keyboard access:** chat rows keep `role="button"` + `tabindex` + Enter handling, copied from
   the frontend's `ChatList.svelte` accessibility pattern.
6. **Svelte 5 event form:** use `onclick={...}` attributes (runes mode), `createEventDispatcher`
   only for the outward `chatSelect`/`requestSelect` events to match the existing codebase style.

## Dependencies

- **Depends on:** Phase 3 (store, context, `mobxObservable` util) and transitively Phase 2
  (endpoints) and Phase 1 (scaffold, common.module.css).
- **Blocks:** Phase 5 mounts `ChatDetailView` into the `viewer-content` section defined here.
- **Parallel with:** Phase 5's leaf components (`ConversationView`, `RawBodyView`) depend only on
  Phase 3 types and can be built alongside this phase; the `ChatDetailView`/`RequestDetailView`
  composition needs this phase's shell.

## Verification Summary

```sh
npm --prefix frontend-proxy run test     # all component tests green
npm --prefix frontend-proxy run check    # svelte-check green
# manual: with the proxy logging dir:
VITE_PROXY_LOGS_PATH=<dir> npm --prefix frontend-proxy run dev
# → http://localhost:5174 — chats appear in the sidebar; Refresh re-fetches;
#   clicking a chat shows the Phase 5 placeholder.
```
