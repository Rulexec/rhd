<script lang="ts">
  import { flowResult } from 'mobx';
  import { getProxyLogsStore } from '../../context.js';
  import { mobxObservable } from '../../util/mobxObservable.svelte.js';
  import commonStyles from '../styles/common.module.css';

  const store = getProxyLogsStore();

  const refreshingGetter = mobxObservable(() => store.refreshing);
  let refreshing = $derived(refreshingGetter());

  function handleRefresh(): void {
    // Fire-and-forget: errors land in store.chatsError and render as banners.
    void flowResult(store.refresh()).catch(() => {});
  }
</script>

<button
  class="{commonStyles['btn']} {commonStyles['btn-primary']} {commonStyles['btn-sm']}"
  disabled={refreshing}
  onclick={handleRefresh}
  data-testid="refresh-button"
>
  {refreshing ? 'Refreshing…' : '↻ Refresh'}
</button>
