<script lang="ts">
  import { onMount } from 'svelte';
  import { defaultProxyLogsApi } from './lib/api/ProxyLogsApi.js';
  import { ProxyLogsStore } from './stores/ProxyLogsStore.js';
  import { setProxyLogsStore } from './context.js';
  import ChatsList from './lib/components/ChatsList.svelte';
  import ChatDetailView from './lib/components/ChatDetailView.svelte';
  import RefreshButton from './lib/components/RefreshButton.svelte';
  import commonStyles from './lib/styles/common.module.css';

  const store = new ProxyLogsStore({ api: defaultProxyLogsApi });
  setProxyLogsStore(store);

  let selectedChatId: number | null = $state(null);

  onMount(() => {
    void store.loadChats();
  });

  async function handleChatSelect(event: CustomEvent<{ chatId: number }>) {
    selectedChatId = event.detail.chatId;
    await store.openChat(event.detail.chatId);
  }
</script>

<div class="app">
  <header class="app-header">
    <h1>RHD Proxy Logs</h1>
    <RefreshButton />
  </header>

  <main class="app-main">
    <div class="viewer-layout">
      <aside class="viewer-sidebar">
        <ChatsList {selectedChatId} on:chatSelect={handleChatSelect} />
      </aside>
      <section class="viewer-content">
        {#if selectedChatId === null}
          <div class="viewer-placeholder">
            <span class={commonStyles['text-muted']}>
              Select a chat to inspect its conversation and raw logs.
            </span>
          </div>
        {:else}
          <ChatDetailView />
        {/if}
      </section>
    </div>
  </main>
</div>

<style>
  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: var(--color-bg);
    color: var(--color-text);
  }

  .app-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--spacing-md);
    border-bottom: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
  }

  .app-header h1 {
    margin: 0;
    font-size: var(--font-size-lg);
  }

  .app-main {
    flex: 1;
    overflow: hidden;
  }

  .viewer-layout {
    display: flex;
    height: 100%;
  }

  .viewer-sidebar {
    width: 300px;
    flex-shrink: 0;
  }

  .viewer-content {
    flex: 1;
    overflow: hidden;
  }

  .viewer-placeholder {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
  }
</style>
