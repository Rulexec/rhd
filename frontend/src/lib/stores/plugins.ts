import { writable, derived, type Writable, type Readable } from 'svelte/store';
import { subscribePluginsList, getPlugins, onPluginListEvents } from '../api/chatApi.js';
import type { PluginSummary } from '../api/schemas.js';

/**
 * Internal store for raw plugin list.
 */
const _plugins: Writable<PluginSummary[]> = writable([]);

/**
 * Loading state.
 */
export const pluginsLoading: Writable<boolean> = writable(false);

/**
 * Error state.
 */
export const pluginsError: Writable<string | null> = writable(null);

/**
 * Derived store: plugins sorted alphabetically by pluginId.
 */
export const plugins: Readable<PluginSummary[]> = derived(_plugins, ($plugins) => {
  return [...$plugins].sort((a, b) => a.pluginId.localeCompare(b.pluginId));
});

/**
 * Whether there are any plugins.
 */
export const hasPlugins: Readable<boolean> = derived(plugins, ($plugins) => $plugins.length > 0);

/**
 * Derived store: count of active plugins.
 */
export const activePluginsCount: Readable<number> = derived(plugins, ($plugins) => {
  return $plugins.filter(p => p.isActive).length;
});

/**
 * Initialize the plugins store: subscribe to events and load initial data.
 * Should be called once on app startup.
 */
export async function initPlugins(): Promise<void> {
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
    pluginsError.set(error instanceof Error ? error.message : String(error));
  }
}

/**
 * Load plugins from server.
 */
export async function loadPlugins(): Promise<void> {
  pluginsLoading.set(true);
  pluginsError.set(null);

  try {
    const result = await getPlugins();
    _plugins.set(result.plugins);
  } catch (error) {
    pluginsError.set(error instanceof Error ? error.message : String(error));
  } finally {
    pluginsLoading.set(false);
  }
}

/**
 * Clear the plugins store.
 */
export function clearPlugins(): void {
  _plugins.set([]);
  pluginsLoading.set(false);
  pluginsError.set(null);
}
