import { makeAutoObservable, flowResult } from 'mobx';
import { yieldPromise } from '../util/async.js';
import type { ChatApi } from '../lib/api/ChatApi.js';
import type { PluginSummary } from '../lib/api/schemas.js';

/**
 * MobX store for plugins list management.
 *
 * Async operations are written as generator PROTOTYPE methods. `makeAutoObservable`
 * auto-wraps generator methods into flows (actions wrapping a promise chain), so
 * calling e.g. `store.loadPlugins()` returns a CancellablePromise, not a suspended
 * generator. Callers that want an explicitly-typed promise use `flowResult(...)`.
 *
 * State fields that drive reactive getters MUST be public (or underscore-prefixed)
 * observable fields: `#`-private class fields are NOT observable by MobX, so
 * `_plugins` is stored publicly while non-reactive dependencies (`#chatApi`,
 * `#cleanupEvents`) may stay private.
 */
export class PluginsStore {
  #chatApi: ChatApi;
  #cleanupEvents: (() => void) | null = null;

  /** Raw (unsorted) plugin list. Public so MobX can observe it. */
  _plugins: PluginSummary[] = [];

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
    return [...this._plugins].sort((a, b) => a.pluginId.localeCompare(b.pluginId));
  }

  /**
   * Check if there are any plugins.
   */
  get hasPlugins(): boolean {
    return this._plugins.length > 0;
  }

  /**
   * Get count of active plugins.
   */
  get activePluginsCount(): number {
    return this._plugins.filter(p => p.isActive).length;
  }

  /**
   * Initialize the store: subscribe to events and load initial data.
   * Should be called once on app startup.
   */
  *init(): Generator<unknown, void, unknown> {
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
      yield* yieldPromise(flowResult(this.loadPlugins()));
    } catch (error) {
      this.error = error instanceof Error ? error.message : String(error);
    }
  }

  /**
   * Load plugins from server.
   */
  *loadPlugins(): Generator<unknown, void, unknown> {
    this.loading = true;
    this.error = null;

    try {
      const result = yield* yieldPromise(this.#chatApi.getPlugins());
      this._plugins = result.plugins;
    } catch (error) {
      this.error = error instanceof Error ? error.message : String(error);
    } finally {
      this.loading = false;
    }
  }

  /**
   * Clear the store state and unsubscribe from events.
   */
  clear(): void {
    if (this.#cleanupEvents) {
      this.#cleanupEvents();
      this.#cleanupEvents = null;
    }
    this._plugins = [];
    this.loading = false;
    this.error = null;
  }

  /**
   * Handle plugin registered event.
   */
  #handlePluginRegistered(plugin: PluginSummary): void {
    // Don't add if already exists
    if (this._plugins.some(p => p.pluginId === plugin.pluginId)) {
      this._plugins = this._plugins.map(p => p.pluginId === plugin.pluginId ? plugin : p);
    } else {
      this._plugins = [...this._plugins, plugin];
    }
  }

  /**
   * Handle plugin updated event.
   */
  #handlePluginUpdated(plugin: PluginSummary): void {
    this._plugins = this._plugins.map(p => p.pluginId === plugin.pluginId ? plugin : p);
  }

  /**
   * Handle plugin removed event.
   */
  #handlePluginRemoved(pluginId: string): void {
    this._plugins = this._plugins.filter(p => p.pluginId !== pluginId);
  }
}