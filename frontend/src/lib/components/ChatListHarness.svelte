<script lang="ts">
  import ChatList from './ChatList.svelte';
  import { setAppStore } from '../../context.js';
  import type { AppStore } from '../../stores/AppStore.js';

  interface Props {
    appStore: AppStore;
    selectedChatId?: number | null;
    onChatSelect?: (detail: { chatId: number }) => void;
  }

  let { appStore, selectedChatId = null, onChatSelect }: Props = $props();

  // Provide the mock store via Svelte context before the target renders.
  (() => {
    setAppStore(appStore);
  })();

  function handleChatSelect(event: CustomEvent<{ chatId: number }>) {
    onChatSelect?.(event.detail);
  }
</script>

<ChatList {selectedChatId} on:chatSelect={handleChatSelect} />