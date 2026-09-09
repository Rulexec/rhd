import { makeAutoObservable, flowResult } from 'mobx';
import { yieldPromise } from '../util/async.js';
import type { ChatApi } from '../lib/api/ChatApi.js';
import type { PluginSummary, PluginState } from '../lib/api/schemas.js';
import { MCP_STATUS_SCHEMA } from '../lib/api/schemas.js';
import type { PluginStateEventHandlers } from '../lib/api/chatApiImpl.js';

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
  #cleanupStateEvents: (() => void) | null = null;

  /** Raw (unsorted) plugin list. Public so MobX can observe it. */
  _plugins: PluginSummary[] = [];

  /** Raw plugin states (all plugins). Public so MobX can observe it. */
  _states: PluginState[] = [];

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
   * Live states of one plugin, sorted by key.
   */
  statesFor(pluginId: string): PluginState[] {
    return this._states
      .filter(s => s.pluginId === pluginId)
      .sort((a, b) => a.key.localeCompare(b.key));
  }

  /**
   * All live states published under a well-known schema (e.g. 'mcpStatus:1').
   */
  statesWithSchema(schema: string): PluginState[] {
    return this._states.filter(s => s.schema === schema);
  }

  /**
   * True while any plugin publishes an mcpStatus:1 state (drives the MCPs tab).
   */
  get hasMcpStatus(): boolean {
    return this._states.some(s => s.schema === MCP_STATUS_SCHEMA);
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

      // Register state event listeners BEFORE the snapshot read so no live
      // update is missed between load and subscribe (version gating makes
      // duplicates harmless).
      const stateHandlers: PluginStateEventHandlers = {
        onPluginStateChanged: ({ state }) => this.#applyState(state),
        onPluginStateRemoved: ({ pluginId, key, version }) =>
          this.#applyStateRemoval(pluginId, key, version)
      };
      this.#cleanupStateEvents = this.#chatApi.onPluginStateEvents(stateHandlers);

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
    if (this.#cleanupStateEvents) {
      this.#cleanupStateEvents();
      this.#cleanupStateEvents = null;
    }
    this._plugins = [];
    this._states = [];
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
    // Server cascades plugin_states on removePlugin; mirror locally.
    this._states = this._states.filter(s => s.pluginId !== pluginId);
  }

  /**
   * Apply a state change only if it is newer than what we hold.
   */
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

  /**
   * Drop a state only if the removal is at least as new as what we hold.
   */
  #applyStateRemoval(pluginId: string, key: string, version: number): void {
    const existing = this._states.find(s => s.pluginId === pluginId && s.key === key);
    if (existing && existing.version > version) {
      return; // a newer update already arrived — removal is stale
    }
    this._states = this._states.filter(
      s => !(s.pluginId === pluginId && s.key === key)
    );
  }
}