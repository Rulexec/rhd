<script lang="ts">
  import { createEventDispatcher } from 'svelte';

  type ChatTab = 'messages' | 'tools';

  interface Props {
    activeTab?: ChatTab;
    toolsCount?: number;
  }

  let { activeTab = $bindable('messages'), toolsCount = 0 }: Props = $props();

  const dispatch = createEventDispatcher<{
    tabChange: { tab: ChatTab };
  }>();

  function selectTab(tab: ChatTab) {
    activeTab = tab;
    dispatch('tabChange', { tab });
  }
</script>

<div class="chat-tab-nav">
  <nav class="tab-nav" role="tablist">
    <button
      class="tab-button"
      class:active={activeTab === 'messages'}
      role="tab"
      aria-selected={activeTab === 'messages'}
      onclick={() => selectTab('messages')}
    >
      Messages
    </button>
    <button
      class="tab-button"
      class:active={activeTab === 'tools'}
      role="tab"
      aria-selected={activeTab === 'tools'}
      onclick={() => selectTab('tools')}
    >
      Tools
      {#if toolsCount > 0}
        <span class="badge">{toolsCount}</span>
      {/if}
    </button>
  </nav>
</div>

<style>
  .chat-tab-nav {
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
    display: flex;
    align-items: center;
    gap: var(--spacing-xs);
  }

  .tab-button:hover {
    color: var(--color-text);
    background: var(--color-bg-tertiary);
  }

  .tab-button.active {
    color: var(--color-primary);
    border-bottom-color: var(--color-primary);
  }

  .badge {
    background: var(--color-primary);
    color: white;
    font-size: var(--font-size-xs);
    padding: 2px 6px;
    border-radius: var(--radius-sm);
    font-weight: 600;
  }
</style>
