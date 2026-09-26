<script lang="ts">
  import ChatsList from './ChatsList.svelte';
  import { setProxyLogsStore } from '../../context.js';
  import type { ProxyLogsStore } from '../../stores/ProxyLogsStore.js';

  interface Props {
    store: ProxyLogsStore;
    selectedChatId?: number | null;
    onChatSelect?: (detail: { chatId: number }) => void;
  }

  let { store, selectedChatId = null, onChatSelect }: Props = $props();

  // Provide the mock store via Svelte context before the target renders.
  (() => {
    setProxyLogsStore(store);
  })();
</script>

<ChatsList {selectedChatId} on:chatSelect={(e) => onChatSelect?.(e.detail)} />
