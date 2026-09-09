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

  // Bridge MobX observables to Svelte reactivity.
  // mobxObservable returns a getter and must be invoked at component top level.
  const statesGetter = mobxObservable(() => pluginsStore.statesWithSchema(MCP_STATUS_SCHEMA));
  const pluginsGetter = mobxObservable(() => pluginsStore.plugins);
  let states = $derived(statesGetter());
  let plugins = $derived(pluginsGetter());

  interface ParsedGroup {
    pluginId: string;
    isActive: boolean;
    state: PluginState;
    // null = content failed to parse; an empty array is a valid zero-server payload
    entries: McpStatusEntry[] | null;
    rawContent: string;
  }

  function parseGroups(
    states: PluginState[],
    plugins: { pluginId: string; isActive: boolean }[]
  ): ParsedGroup[] {
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

<style>
  .mcp-status-list {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--color-bg);
  }

  .mcp-status-header {
    padding: var(--spacing-md) var(--spacing-lg);
    border-bottom: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .mcp-status-header h2 {
    margin: 0;
    font-size: var(--font-size-lg);
  }

  .mcp-status-empty {
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

  .mcp-group {
    padding: var(--spacing-md) var(--spacing-lg);
    border-bottom: 1px solid var(--color-border);
  }

  .mcp-group-title {
    display: flex;
    align-items: center;
    gap: var(--spacing-sm);
    margin: 0 0 var(--spacing-sm);
    font-size: var(--font-size-md);
    font-weight: 500;
  }

  .mcp-group-version {
    color: var(--color-text-muted);
    font-size: var(--font-size-xs);
    font-weight: 400;
  }

  .mcp-group-stale {
    color: var(--color-text-muted);
    font-size: var(--font-size-xs);
    font-style: italic;
    font-weight: 400;
  }

  .mcp-parse-error pre {
    padding: var(--spacing-sm);
    margin: var(--spacing-xs) 0 0;
    border-radius: var(--radius-sm);
    background: var(--color-bg-secondary);
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: var(--font-size-xs);
    overflow-x: auto;
    white-space: pre-wrap;
    word-break: break-all;
  }

  .mcp-entry {
    display: flex;
    align-items: center;
    gap: var(--spacing-md);
    flex-wrap: wrap;
    cursor: default;
  }

  .mcp-entry-name {
    font-weight: 500;
  }

  .mcp-entry-id {
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: var(--font-size-xs);
  }

  .mcp-entry-status {
    display: flex;
    align-items: center;
    gap: var(--spacing-xs);
    font-size: var(--font-size-sm);
    margin-left: auto;
  }

  .mcp-entry-status.ok {
    color: var(--color-success);
  }

  .mcp-entry-status.error {
    color: var(--color-error);
  }

  .mcp-entry.mcp-entry-error {
    background: var(--color-bg-secondary);
  }

  .mcp-entry-error-msg {
    flex-basis: 100%;
    color: var(--color-error);
    font-size: var(--font-size-sm);
  }
</style>
