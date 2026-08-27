# Phase 5: PluginsStore Migration

## Overview

This phase migrates plugins list management from Svelte stores to MobX store. The PluginsStore will manage the list of plugins, loading state, error state, and provide methods for plugin list operations (init, load).

## Files to Create

### 1. `frontend/src/stores/PluginsStore.ts`

**Purpose**: MobX store for plugins list management.

**Additions**:
```typescript
import { makeAutoObservable, flow } from 'mobx';
import { yieldPromise } from '../util/async.js';
import type { ChatApi } from '../lib/api/ChatApi.js';
import type { PluginSummary } from '../lib/api/schemas.js';

/**
 * MobX store for plugins list management.
 */
export class PluginsStore {
  #chatApi: ChatApi;
  #plugins: PluginSummary[] = [];
  #cleanupEvents: (() => void) | null = null;

  loading: boolean = false;
  error: string | null = null;

  constructor(options: { chatApi: ChatApi }) {
    this.#chatApi = options.chatApi;
    makeAutoObservable(this);
  }

  /**
   * Get plugins sorted alphabetically by pluginId.
   */
  get plugins(): PluginSummary[] {
    return [...this.#plugins].sort((a, b) => a.pluginId.localeCompare(b.pluginId));
  }

  /**
   * Check if there are any plugins.
   */
  get hasPlugins(): boolean {
    return this.#plugins.length > 0;
  }

  /**
   * Get count of active plugins.
   */
  get activePluginsCount(): number {
    return this.#plugins.filter(p => p.isActive).length;
  }

  /**
   * Initialize the store: subscribe to events and load initial data.
   * Should be called once on app startup.
   */
  *init(): Generator {
    this.error = null;

    try {
      // Subscribe to plugin list events
      yield* yieldPromise(this.#chatApi.subscribePluginsList());

      // Register event listeners
      this.#cleanupEvents = this.#chatApi.onPluginListEvents({
        onPluginRegistered: ({ plugin }) => {
          this.#handlePluginRegistered(plugin);
        },
        onPluginUpdated: ({ plugin }) => {
          this.#handlePluginUpdated(plugin);
        },
        onPluginRemoved: ({ pluginId }) => {
          this.#handlePluginRemoved(pluginId);
        }
      });

      // Load initial plugin list
      yield* yieldPromise(this.loadPlugins());
    } catch (error) {
      this.error = error instanceof Error ? error.message : String(error);
    }
  }

  /**
   * Load plugins from server.
   */
  *loadPlugins(): Generator {
    this.loading = true;
    this.error = null;

    try {
      const result = yield* yieldPromise(this.#chatApi.getPlugins());
      this.#plugins = result.plugins;
    } catch (error) {
      this.error = error instanceof Error ? error.message : String(error);
    } finally {
      this.loading = false;
    }
  }

  /**
   * Clear the store state.
   */
  clear(): void {
    if (this.#cleanupEvents) {
      this.#cleanupEvents();
      this.#cleanupEvents = null;
    }
    this.#plugins = [];
    this.loading = false;
    this.error = null;
  }

  /**
   * Handle plugin registered event.
   */
  #handlePluginRegistered(plugin: PluginSummary): void {
    // Don't add if already exists
    if (this.#plugins.some(p => p.pluginId === plugin.pluginId)) {
      this.#plugins = this.#plugins.map(p => p.pluginId === plugin.pluginId ? plugin : p);
    } else {
      this.#plugins = [...this.#plugins, plugin];
    }
  }

  /**
   * Handle plugin updated event.
   */
  #handlePluginUpdated(plugin: PluginSummary): void {
    this.#plugins = this.#plugins.map(p => p.pluginId === plugin.pluginId ? plugin : p);
  }

  /**
   * Handle plugin removed event.
   */
  #handlePluginRemoved(pluginId: string): void {
    this.#plugins = this.#plugins.filter(p => p.pluginId !== pluginId);
  }
}
```

## Files to Modify

### 1. `frontend/src/stores/AppStore.ts`

**Modifications**: Add `plugins` lazy getter.

**Changes**: Add the following to AppStore class:

```typescript
import { PluginsStore } from './PluginsStore.js';

export class AppStore {
  #chatApi: ChatApi;
  #chatsListStore?: ChatsListStore;
  #chatStore?: ChatStore;
  #pluginsStore?: PluginsStore;
  // ... other fields

  constructor(options: { chatApi: ChatApi }) {
    this.#chatApi = options.chatApi;
    makeAutoObservable(this);
  }

  /**
   * Chat list store (lazy initialized).
   */
  get chatsList(): ChatsListStore {
    if (!this.#chatsListStore) {
      this.#chatsListStore = new ChatsListStore({ chatApi: this.#chatApi });
    }
    return this.#chatsListStore;
  }

  /**
   * Current chat store (lazy initialized).
   */
  get chat(): ChatStore {
    if (!this.#chatStore) {
      this.#chatStore = new ChatStore({ chatApi: this.#chatApi });
    }
    return this.#chatStore;
  }

  /**
   * Plugins store (lazy initialized).
   */
  get plugins(): PluginsStore {
    if (!this.#pluginsStore) {
      this.#pluginsStore = new PluginsStore({ chatApi: this.#chatApi });
    }
    return this.#pluginsStore;
  }

  // ... other getters
}
```

## Tests

### Unit Tests for PluginsStore

```typescript
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { PluginsStore } from './PluginsStore.js';
import type { ChatApi } from '../lib/api/ChatApi.js';
import type { PluginSummary } from '../lib/api/schemas.js';

describe('PluginsStore', () => {
  let store: PluginsStore;
  let mockChatApi: ChatApi;

  const mockPlugin: PluginSummary = {
    pluginId: 'test-plugin',
    isActive: true
  };

  beforeEach(() => {
    mockChatApi = {
      subscribePluginsList: vi.fn().mockResolvedValue(undefined),
      getPlugins: vi.fn().mockResolvedValue({ plugins: [mockPlugin] }),
      onPluginListEvents: vi.fn().mockReturnValue(() => {}),
      // ... other methods
    } as unknown as ChatApi;

    store = new PluginsStore({ chatApi: mockChatApi });
  });

  it('should initialize with empty state', () => {
    expect(store.plugins).toEqual([]);
    expect(store.hasPlugins).toBe(false);
    expect(store.activePluginsCount).toBe(0);
    expect(store.loading).toBe(false);
    expect(store.error).toBe(null);
  });

  it('should load plugins', async () => {
    await store.loadPlugins();
    
    expect(mockChatApi.getPlugins).toHaveBeenCalled();
    expect(store.plugins).toEqual([mockPlugin]);
    expect(store.hasPlugins).toBe(true);
    expect(store.loading).toBe(false);
  });

  it('should handle load error', async () => {
    vi.mocked(mockChatApi.getPlugins).mockRejectedValueOnce(new Error('Load failed'));
    
    await store.loadPlugins();
    
    expect(store.error).toBe('Load failed');
    expect(store.loading).toBe(false);
  });

  it('should sort plugins alphabetically by pluginId', async () => {
    const plugin1: PluginSummary = { pluginId: 'zebra-plugin', isActive: true };
    const plugin2: PluginSummary = { pluginId: 'alpha-plugin', isActive: true };
    
    vi.mocked(mockChatApi.getPlugins).mockResolvedValueOnce({ plugins: [plugin1, plugin2] });
    await store.loadPlugins();
    
    expect(store.plugins[0].pluginId).toBe('alpha-plugin');
    expect(store.plugins[1].pluginId).toBe('zebra-plugin');
  });

  it('should count active plugins', async () => {
    const activePlugin: PluginSummary = { pluginId: 'active', isActive: true };
    const inactivePlugin: PluginSummary = { pluginId: 'inactive', isActive: false };
    
    vi.mocked(mockChatApi.getPlugins).mockResolvedValueOnce({ 
      plugins: [activePlugin, inactivePlugin] 
    });
    await store.loadPlugins();
    
    expect(store.activePluginsCount).toBe(1);
  });

  it('should handle plugin registered event', async () => {
    let onPluginRegisteredHandler: ((data: { plugin: PluginSummary }) => void) | undefined;
    vi.mocked(mockChatApi.onPluginListEvents).mockImplementation((handlers) => {
      onPluginRegisteredHandler = handlers.onPluginRegistered;
      return () => {};
    });

    await store.init();
    
    const newPlugin: PluginSummary = { pluginId: 'new-plugin', isActive: true };
    onPluginRegisteredHandler!({ plugin: newPlugin });
    
    expect(store.plugins.some(p => p.pluginId === 'new-plugin')).toBe(true);
  });

  it('should handle plugin updated event', async () => {
    let onPluginUpdatedHandler: ((data: { plugin: PluginSummary }) => void) | undefined;
    vi.mocked(mockChatApi.onPluginListEvents).mockImplementation((handlers) => {
      onPluginUpdatedHandler = handlers.onPluginUpdated;
      return () => {};
    });

    await store.loadPlugins();
    
    const updatedPlugin: PluginSummary = { pluginId: 'test-plugin', isActive: false };
    onPluginUpdatedHandler!({ plugin: updatedPlugin });
    
    expect(store.plugins[0].isActive).toBe(false);
  });

  it('should handle plugin removed event', async () => {
    let onPluginRemovedHandler: ((data: { pluginId: string }) => void) | undefined;
    vi.mocked(mockChatApi.onPluginListEvents).mockImplementation((handlers) => {
      onPluginRemovedHandler = handlers.onPluginRemoved;
      return () => {};
    });

    await store.loadPlugins();
    expect(store.plugins.length).toBe(1);
    
    onPluginRemovedHandler!({ pluginId: 'test-plugin' });
    
    expect(store.plugins.length).toBe(0);
  });

  it('should cleanup on clear', async () => {
    const cleanup = vi.fn();
    vi.mocked(mockChatApi.onPluginListEvents).mockReturnValue(cleanup);

    await store.init();
    store.clear();
    
    expect(cleanup).toHaveBeenCalled();
    expect(store.plugins).toEqual([]);
  });
});
```

## Implementation Notes

1. **Generator Functions**: All async methods use generator syntax (`*methodName()`) with `yield* yieldPromise()` for proper typing with MobX flow.

2. **Private Fields**: 
   - `#plugins` stores the raw plugin list (unsorted)
   - `#chatApi` holds the API reference
   - `#cleanupEvents` holds the event listener cleanup function

3. **Computed Getters**: 
   - `plugins` returns sorted list (computed on access)
   - `hasPlugins` derives from `#plugins.length`
   - `activePluginsCount` filters and counts active plugins

4. **Event Handling**: Private methods `#handlePluginRegistered`, `#handlePluginUpdated`, `#handlePluginRemoved` handle WebSocket events and update state.

5. **Error Handling**: All async methods catch errors and store them in `error` property.

6. **Immutability**: Array updates create new arrays (`[...this.#plugins, plugin]`, `this.#plugins.map(...)`, `this.#plugins.filter(...)`) to ensure MobX detects changes.

7. **Cleanup**: The `clear()` method calls the event cleanup function and resets state.

## Dependencies

- Depends on Phase 1 for infrastructure (MobX, utilities, ChatApi interface).
- Must be completed before Phase 6 (Component Migration).
- Can be done in parallel with Phases 2, 3, 4.
