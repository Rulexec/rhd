<script lang="ts">
  let { activeTab = $bindable() }: { activeTab: 'scenarios' | 'chats' } = $props();

  const tabs = [
    { id: 'scenarios' as const, label: 'Scenarios' },
    { id: 'chats' as const, label: 'Chats' },
  ];
</script>

<nav class="tab-nav">
  {#each tabs as tab}
    <button
      class="tab-button"
      class:active={activeTab === tab.id}
      onclick={() => activeTab = tab.id}
    >
      {tab.label}
    </button>
  {/each}
</nav>

<style>
  .tab-nav {
    display: flex;
    gap: 0;
    border-bottom: 2px solid var(--color-border);
  }

  .tab-button {
    padding: 12px 24px;
    border: none;
    background: none;
    cursor: pointer;
    font-size: 14px;
    color: var(--color-text-muted);
    border-bottom: 2px solid transparent;
    margin-bottom: -2px;
    transition: color 0.15s, border-color 0.15s;
  }

  .tab-button:hover {
    color: var(--color-text);
  }

  .tab-button.active {
    color: var(--color-primary);
    border-bottom-color: var(--color-primary);
    font-weight: 600;
  }
</style>
