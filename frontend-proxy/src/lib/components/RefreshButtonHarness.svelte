<script lang="ts">
  import RefreshButton from './RefreshButton.svelte';
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

<RefreshButton />
