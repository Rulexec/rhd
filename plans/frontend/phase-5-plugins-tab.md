# Phase 5: Plugins Tab

## Overview

Implement the Plugins tab that displays a list of registered plugins with their active/inactive status. This phase provides visibility into the plugin ecosystem.

**Scope:**
- Plugins store with real-time updates
- Plugin API methods (list, subscribe)
- Plugin list component with status indicators
- Integration into tab navigation

**Out of Scope:**
- Connection status UI (Phase 6)
- Advanced error handling polish (Phase 6)
- Plugin configuration UI (not in requirements)

## Dependencies

- **Requires**: Phase 1 (WebSocket foundation)
- **Blocks**: Phase 6 (needs plugin list for polish)

## Files to Create/Modify

### 1. `frontend/src/lib/api/chatApi.js`

**Modify:** Add methods for plugin operations.

```javascript
// Add these imports at the top
import {
  // ... existing imports ...
  GetPluginsResultSchema,
  PluginRegisteredDataSchema,
  PluginUpdatedDataSchema,
  PluginRemovedDataSchema
} from './schemas.js';

/**
 * Subscribe to plugin list events (pluginRegistered, pluginUpdated, pluginRemoved).
 * @returns {Promise<void>}
 */
export async function subscribePluginsList() {
  await websocket.request('subscribePluginsList', {});
}

/**
 * Unsubscribe from plugin list events.
 * @returns {Promise<void>}
 */
export async function unsubscribePluginsList() {
  await websocket.request('unsubscribePluginsList', {});
}

/**
 * Get list of all registered plugins.
 * @returns {Promise<import('./schemas.js').GetPluginsResult>}
 */
export async function getPlugins() {
  const data = await websocket.request('getPlugins', {});
  return GetPluginsResultSchema.parse(data);
}

/**
 * Register event listeners for plugin list events.
 * @param {Object} handlers
 * @param {Function} handlers.onPluginRegistered
 * @param {Function} handlers.onPluginUpdated
 * @param {Function} handlers.onPluginRemoved
 * @returns {Function} - Cleanup function
 */
export function onPluginListEvents({ onPluginRegistered, onPluginUpdated, onPluginRemoved }) {
  const unsubs = [];

  if (onPluginRegistered) {
    unsubs.push(websocket.on('pluginRegistered', (data) => {
      const parsed = PluginRegisteredDataSchema.parse(data);
      onPluginRegistered(parsed);
    }));
  }

  if (onPluginUpdated) {
    unsubs.push(websocket.on('pluginUpdated', (data) => {
      const parsed = PluginUpdatedDataSchema.parse(data);
      onPluginUpdated(parsed);
    }));
  }

  if (onPluginRemoved) {
    unsubs.push(websocket.on('pluginRemoved', (data) => {
      const parsed = PluginRemovedDataSchema.parse(data);
      onPluginRemoved(parsed);
    }));
  }

  return () => {
    unsubs.forEach(unsub => unsub());
  };
}
```

### 2. `frontend/src/lib/stores/plugins.js`

**Purpose:** Svelte store for plugin list with reactive updates.

```javascript
import { writable, derived } from 'svelte/store';
import { subscribePluginsList, getPlugins, onPluginListEvents } from '../api/chatApi.js';

/**
 * Internal store for raw plugin list.
 * @type {import('svelte/store').Writable<import('../api/schemas.js').PluginSummary[]>}
 */
const _plugins = writable([]);

/**
 * Loading state.
 * @type {import('svelte/store').Writable<boolean>}
 */
export const pluginsLoading = writable(false);

/**
 * Error state.
 * @type {import('svelte/store').Writable<string | null>}
 */
export const pluginsError = writable(null);

/**
 * Derived store: plugins sorted alphabetically by pluginId.
 */
export const plugins = derived(_plugins, ($plugins) => {
  return [...$plugins].sort((a, b) => a.pluginId.localeCompare(b.pluginId));
});

/**
 * Whether there are any plugins.
 */
export const hasPlugins = derived(plugins, ($plugins) => $plugins.length > 0);

/**
 * Derived store: count of active plugins.
 */
export const activePluginsCount = derived(plugins, ($plugins) => {
  return $plugins.filter(p => p.isActive).length;
});

/**
 * Initialize the plugins store: subscribe to events and load initial data.
 * Should be called once on app startup.
 */
export async function initPlugins() {
  pluginsError.set(null);

  try {
    // Subscribe to plugin list events
    await subscribePluginsList();

    // Register event listeners
    onPluginListEvents({
      onPluginRegistered: ({ plugin }) => {
        _plugins.update(current => {
          // Don't add if already exists
          if (current.some(p => p.pluginId === plugin.pluginId)) {
            return current.map(p => p.pluginId === plugin.pluginId ? plugin : p);
          }
          return [...current, plugin];
        });
      },
      onPluginUpdated: ({ plugin }) => {
        _plugins.update(current =>
          current.map(p => p.pluginId === plugin.pluginId ? plugin : p)
        );
      },
      onPluginRemoved: ({ pluginId }) => {
        _plugins.update(current =>
          current.filter(p => p.pluginId !== pluginId)
        );
      }
    });

    // Load initial plugin list
    await loadPlugins();
  } catch (error) {
    pluginsError.set(error.message);
  }
}

/**
 * Load plugins from server.
 */
export async function loadPlugins() {
  pluginsLoading.set(true);
  pluginsError.set(null);

  try {
    const result = await getPlugins();
    _plugins.set(result.plugins);
  } catch (error) {
    pluginsError.set(error.message);
  } finally {
    pluginsLoading.set(false);
  }
}

/**
 * Clear the plugins store.
 */
export function clearPlugins() {
  _plugins.set([]);
  pluginsLoading.set(false);
  pluginsError.set(null);
}
```

### 3. `frontend/src/lib/components/PluginList.svelte`

**Purpose:** Plugin list display component with status indicators.

```svelte
<script>
  import { plugins, hasPlugins, pluginsLoading, pluginsError, activePluginsCount } from '../stores/plugins.js';
  import commonStyles from '../styles/common.css?module';
</script>

<div class="plugin-list">
  <div class="plugin-list-header">
    <h2>Plugins</h2>
    {#if $hasPlugins}
      <span class="plugin-count text-muted">
        {$activePluginsCount} / {$plugins.length} active
      </span>
    {/if}
  </div>

  {#if $pluginsLoading}
    <div class="plugin-list-loading">
      <span class="text-muted">Loading plugins...</span>
    </div>
  {:else if $pluginsError}
    <div class="plugin-list-error">
      <span class="text-error">{$pluginsError}</span>
    </div>
  {:else if !$hasPlugins}
    <div class="plugin-list-empty">
      <div class="empty-icon">🔌</div>
      <p class="text-muted">No plugins registered</p>
      <p class="text-muted text-sm">
        Plugins extend the chat system with additional functionality.
        Start a plugin to see it here.
      </p>
    </div>
  {:else}
    <ul class="{commonStyles['list']} plugin-list-items">
      {#each $plugins as plugin (plugin.pluginId)}
        <li class="{commonStyles['list-item']} plugin-item">
          <div class="plugin-item-content">
            <div class="plugin-info">
              <span class="plugin-id">{plugin.pluginId}</span>
              <span class="plugin-status" class:active={plugin.isActive} class:inactive={!plugin.isActive}>
                <span class="{commonStyles['status-dot']} {plugin.isActive ? commonStyles['status-dot-active'] : commonStyles['status-dot-inactive']}"></span>
                {plugin.isActive ? 'Active' : 'Inactive'}
              </span>
            </div>
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .plugin-list {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--color-bg);
  }

  .plugin-list-header {
    padding: var(--spacing-md) var(--spacing-lg);
    border-bottom: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .plugin-list-header h2 {
    margin: 0;
    font-size: var(--font-size-lg);
  }

  .plugin-count {
    font-size: var(--font-size-sm);
  }

  .plugin-list-loading,
  .plugin-list-error {
    padding: var(--spacing-lg);
    text-align: center;
  }

  .plugin-list-empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: var(--spacing-xl);
    text-align: center;
  }

  .empty-icon {
    font-size: 48px;
    margin-bottom: var(--spacing-md);
  }

  .text-sm {
    font-size: var(--font-size-sm);
  }

  .plugin-list-items {
    flex: 1;
    overflow-y: auto;
  }

  .plugin-item {
    cursor: default;
  }

  .plugin-item:hover {
    background: var(--color-bg-secondary);
  }

  .plugin-item-content {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .plugin-info {
    display: flex;
    align-items: center;
    gap: var(--spacing-md);
  }

  .plugin-id {
    font-weight: 500;
    font-size: var(--font-size-md);
  }

  .plugin-status {
    display: flex;
    align-items: center;
    gap: var(--spacing-xs);
    font-size: var(--font-size-sm);
    color: var(--color-text-secondary);
  }

  .plugin-status.active {
    color: var(--color-success);
  }

  .plugin-status.inactive {
    color: var(--color-text-muted);
  }

  .text-muted {
    color: var(--color-text-muted);
  }

  .text-error {
    color: var(--color-error);
  }
</style>
```

### 4. `frontend/src/App.svelte`

**Modify:** Integrate PluginList in Plugins tab and initialize plugins store.

```svelte
<script>
  import { onMount } from 'svelte';
  import { connectionStore } from './lib/stores/connection.js';
  import { websocket } from './lib/api/websocket.js';
  import { initChats } from './lib/stores/chats.js';
  import { initPlugins } from './lib/stores/plugins.js';
  import { selectChat, clearChat } from './lib/stores/chat.js';
  import TabView from './lib/components/TabView.svelte';
  import ChatList from './lib/components/ChatList.svelte';
  import ChatView from './lib/components/ChatView.svelte';
  import PluginList from './lib/components/PluginList.svelte';

  /** @type {'chats' | 'plugins'} */
  let activeTab = 'chats';

  /** @type {number | null} */
  let selectedChatId = null;

  onMount(() => {
    websocket.connect();

    // Wait for connection, then initialize stores
    const unsubscribe = connectionStore.subscribe(({ status }) => {
      if (status === 'connected') {
        initChats();
        initPlugins();
        unsubscribe();
      }
    });
  });

  function handleTabChange(event) {
    activeTab = event.detail.tab;
  }

  async function handleChatSelect(event) {
    selectedChatId = event.detail.chatId;
    await selectChat(selectedChatId);
  }
</script>

<div class="app">
  <header class="app-header">
    <h1>RHD Chat</h1>
  </header>

  <TabView {activeTab} on:tabChange={handleTabChange} />

  <main class="app-main">
    {#if activeTab === 'chats'}
      <div class="chats-layout">
        <aside class="chats-sidebar">
          <ChatList {selectedChatId} on:chatSelect={handleChatSelect} />
        </aside>
        <section class="chats-content">
          <ChatView />
        </section>
      </div>
    {:else if activeTab === 'plugins'}
      <PluginList />
    {/if}
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

  .chats-layout {
    display: flex;
    height: 100%;
  }

  .chats-sidebar {
    width: 300px;
    flex-shrink: 0;
  }

  .chats-content {
    flex: 1;
    overflow: hidden;
  }
</style>
```

## Tests

### Manual Testing

1. **Plugin List Display:**
   - Switch to Plugins tab
   - Verify plugin list loads and displays plugins
   - Verify plugins sorted alphabetically by pluginId
   - Verify active/inactive count shown in header

2. **Plugin Status:**
   - Verify each plugin shows pluginId and status
   - Verify active plugins show green dot and "Active" text
   - Verify inactive plugins show gray dot and "Inactive" text

3. **Real-Time Updates:**
   - Start a plugin (e.g., AI completions plugin)
   - Verify plugin appears in list automatically
   - Stop the plugin
   - Verify plugin disappears from list or status changes to inactive

4. **Empty State:**
   - Stop all plugins
   - Verify "No plugins registered" message shown
   - Verify helpful description text shown

5. **Loading State:**
   - Switch to Plugins tab
   - Verify loading indicator shown while fetching
   - Verify loading indicator disappears after load

6. **Error State:**
   - Disconnect WebSocket
   - Verify error message shown
   - Reconnect WebSocket
   - Verify error clears and data loads

7. **Tab Navigation:**
   - Switch between Chats and Plugins tabs
   - Verify state is preserved (chat selection, plugin list)
   - Verify no data loss when switching tabs

## Implementation Notes

1. **Plugin Sorting**: Plugins are sorted alphabetically by `pluginId` using derived store. This provides a consistent, predictable order.

2. **Status Indicator**: Uses the `status-dot` CSS class from `common.css` with color variants (active = green, inactive = gray).

3. **Active Count**: Header shows "X / Y active" count to give quick overview of plugin health.

4. **Empty State**: Provides helpful context about what plugins are and how to add them.

5. **Event Handling**: Plugin events are handled similarly to chat events - the store subscribes to events and updates reactively.

6. **Initialization**: `initPlugins()` is called once on app startup after WebSocket connects. It subscribes to events and loads initial data.

7. **No Plugin Selection**: Unlike chats, plugins don't have a "selected" state. The list is read-only for now.

## Dependencies

- **Requires**: Phase 1 (WebSocket foundation)
- **Blocks**: Phase 6 (needs plugin list for polish)

## Success Criteria

- [ ] Plugins tab displays list of all registered plugins
- [ ] Each plugin shows ID and active/inactive status
- [ ] Status indicator uses visual cue (green/gray dot)
- [ ] Plugin list updates in real-time when plugins register/remove
- [ ] Empty state shown when no plugins registered
- [ ] Plugins tab accessible via tab navigation
- [ ] Plugins sorted alphabetically by pluginId
- [ ] Active/inactive count shown in header
- [ ] Loading state shown while fetching plugins
- [ ] Error state shown when operations fail
- [ ] Plugin list state preserved when switching tabs
