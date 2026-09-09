<script lang="ts">
  import { marked } from 'marked';
  import { getAppStore } from '../../context.js';
  import { mobxObservable } from '../../util/mobxObservable.svelte.js';
  import type { PluginState } from '../api/schemas.js';
  import commonStyles from '../styles/common.module.css';

  const appStore = getAppStore();
  const pluginsStore = appStore.plugins;

  // Bridge MobX observables to Svelte reactivity.
  // mobxObservable returns a getter and must be invoked at component top level.
  const pluginsGetter = mobxObservable(() => pluginsStore.plugins);
  const hasPluginsGetter = mobxObservable(() => pluginsStore.hasPlugins);
  const pluginsLoadingGetter = mobxObservable(() => pluginsStore.loading);
  const pluginsErrorGetter = mobxObservable(() => pluginsStore.error);
  const activePluginsCountGetter = mobxObservable(() => pluginsStore.activePluginsCount);
  // Single subscription over the raw states array; per-plugin lists are derived
  // below via a pure helper so we never register mobxObservable inside an each-block.
  const statesSnapshotGetter = mobxObservable(() => pluginsStore._states);

  let plugins = $derived(pluginsGetter());
  let hasPlugins = $derived(hasPluginsGetter());
  let pluginsLoading = $derived(pluginsLoadingGetter());
  let pluginsError = $derived(pluginsErrorGetter());
  let activePluginsCount = $derived(activePluginsCountGetter());
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
</script>

<div class="plugin-list">
  <div class="plugin-list-header">
    <h2>Plugins</h2>
    {#if hasPlugins}
      <span class="plugin-count {commonStyles['text-muted']}">
        {activePluginsCount} / {plugins.length} active
      </span>
    {/if}
  </div>

  {#if pluginsLoading}
    <div class="plugin-list-loading">
      <span class="{commonStyles['text-muted']}">Loading plugins...</span>
    </div>
  {:else if pluginsError}
    <div class="plugin-list-error">
      <span class="{commonStyles['text-error']}">{pluginsError}</span>
    </div>
  {:else if !hasPlugins}
    <div class="plugin-list-empty">
      <div class="empty-icon">🔌</div>
      <p class="{commonStyles['text-muted']}">No plugins registered</p>
      <p class="{commonStyles['text-muted']} text-sm">
        Plugins extend the chat system with additional functionality.
        Start a plugin to see it here.
      </p>
    </div>
  {:else}
    <ul class="{commonStyles['list']} plugin-list-items">
      {#each plugins as plugin (plugin.pluginId)}
        <!-- {@const} must be a direct child of the each-block (Svelte 5) -->
        {@const states = statesFor(plugin.pluginId)}
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

  .plugin-states {
    margin-top: var(--spacing-sm);
    padding-left: var(--spacing-md);
    border-top: 1px solid var(--color-border);
  }

  .plugin-states-summary {
    padding: var(--spacing-xs) 0;
    color: var(--color-text-muted);
    font-size: var(--font-size-sm);
    cursor: pointer;
    user-select: none;
  }

  .plugin-state {
    padding: var(--spacing-sm) 0;
    border-top: 1px dashed var(--color-border);
  }

  .plugin-state-header {
    display: flex;
    align-items: center;
    gap: var(--spacing-sm);
    flex-wrap: wrap;
  }

  .plugin-state-key {
    font-weight: 500;
    font-size: var(--font-size-sm);
  }

  .plugin-state-badge {
    padding: 1px var(--spacing-sm);
    border-radius: var(--radius-full);
    background: var(--color-bg-tertiary);
    color: var(--color-text-secondary);
    font-size: var(--font-size-xs);
  }

  .plugin-state-schema {
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  }

  .plugin-state-version {
    color: var(--color-text-muted);
    font-size: var(--font-size-xs);
  }

  .plugin-state-stale {
    color: var(--color-text-muted);
    font-size: var(--font-size-xs);
    font-style: italic;
  }

  .plugin-state-content {
    margin-top: var(--spacing-xs);
    font-size: var(--font-size-sm);
  }

  .plugin-state-json {
    padding: var(--spacing-sm);
    margin: var(--spacing-xs) 0 0;
    border-radius: var(--radius-sm);
    background: var(--color-bg-secondary);
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: var(--font-size-xs);
    overflow-x: auto;
    white-space: pre;
  }
</style>
