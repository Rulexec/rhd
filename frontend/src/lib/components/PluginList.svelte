<script lang="ts">
  import { plugins, hasPlugins, pluginsLoading, pluginsError, activePluginsCount } from '../stores/plugins.js';
  import commonStyles from '../styles/common.module.css';
</script>

<div class="plugin-list">
  <div class="plugin-list-header">
    <h2>Plugins</h2>
    {#if $hasPlugins}
      <span class="plugin-count {commonStyles['text-muted']}">
        {$activePluginsCount} / {$plugins.length} active
      </span>
    {/if}
  </div>

  {#if $pluginsLoading}
    <div class="plugin-list-loading">
      <span class="{commonStyles['text-muted']}">Loading plugins...</span>
    </div>
  {:else if $pluginsError}
    <div class="plugin-list-error">
      <span class="{commonStyles['text-error']}">{$pluginsError}</span>
    </div>
  {:else if !$hasPlugins}
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
</style>
