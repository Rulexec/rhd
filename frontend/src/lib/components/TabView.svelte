<script lang="ts">
  import { createEventDispatcher } from 'svelte';

  export type TabType = 'chats' | 'plugins' | 'mcps';

  interface Props {
    activeTab?: TabType;
    /** Show the MCPs tab only while some plugin publishes mcpStatus:1. */
    showMcps?: boolean;
  }

  let { activeTab = 'chats', showMcps = false }: Props = $props();

  const dispatch = createEventDispatcher<{
    tabChange: { tab: TabType };
  }>();

  function selectTab(tab: TabType) {
    activeTab = tab;
    dispatch('tabChange', { tab });
  }
</script>

<div class="tab-view">
  <nav class="tab-nav" role="tablist">
    <button
      class="tab-button"
      class:active={activeTab === 'chats'}
      role="tab"
      aria-selected={activeTab === 'chats'}
      onclick={() => selectTab('chats')}
    >
      Chats
    </button>
    <button
      class="tab-button"
      class:active={activeTab === 'plugins'}
      role="tab"
      aria-selected={activeTab === 'plugins'}
      onclick={() => selectTab('plugins')}
    >
      Plugins
    </button>
    {#if showMcps}
      <button
        class="tab-button"
        class:active={activeTab === 'mcps'}
        role="tab"
        aria-selected={activeTab === 'mcps'}
        onclick={() => selectTab('mcps')}
      >
        MCPs
      </button>
    {/if}
  </nav>
</div>

<style>
  .tab-view {
    border-bottom: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
  }

  .tab-nav {
    display: flex;
    padding: 0 var(--spacing-md);
  }

  .tab-button {
    padding: var(--spacing-sm) var(--spacing-md);
    border: none;
    border-bottom: 2px solid transparent;
    background: transparent;
    color: var(--color-text-secondary);
    font-size: var(--font-size-sm);
    font-weight: 500;
    cursor: pointer;
    transition: all var(--transition-fast);
  }

  .tab-button:hover {
    color: var(--color-text);
    background: var(--color-bg-tertiary);
  }

  .tab-button.active {
    color: var(--color-primary);
    border-bottom-color: var(--color-primary);
  }
</style>
