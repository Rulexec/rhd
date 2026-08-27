<script lang="ts">
  import ConnectionStatus from './ConnectionStatus.svelte';
  import { setAppStore } from '../../context.js';
  import type { AppStore } from '../../stores/AppStore.js';

  interface Props {
    appStore: AppStore;
  }

  let { appStore }: Props = $props();

  // Provide the mock store via Svelte context before the target renders.
  (() => {
    setAppStore(appStore);
  })();
</script>

<ConnectionStatus />