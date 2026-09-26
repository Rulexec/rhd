<script lang="ts">
  import ChatDetailView from './ChatDetailView.svelte';
  import { setProxyLogsStore } from '../../context.js';
  import type { ProxyLogsStore } from '../../stores/ProxyLogsStore.js';

  interface Props {
    store: ProxyLogsStore;
  }

  let { store }: Props = $props();

  // Provide the mock store via Svelte context before the target renders.
  (() => {
    setProxyLogsStore(store);
  })();
</script>

<ChatDetailView />
