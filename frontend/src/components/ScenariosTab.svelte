<script lang="ts">
  import ActiveScenario from './ActiveScenario.svelte';
  import FinishedScenario from './FinishedScenario.svelte';
  import { subscribe, getFinishedScenarios } from '../lib/ws';
  import { activeScenariosList, finishedScenarios, lastKnownId, wsConnected } from '../lib/stores';

  let loaded = $state(false);
  let dataLoaded = $state(false);

  $effect(() => {
    if ($wsConnected && !dataLoaded) {
      loadData();
    }
  });

  async function loadData() {
    try {
      await subscribe();

      const currentLastId = $lastKnownId;
      const storeIsEmpty = $finishedScenarios.length === 0;
      const fetchAll = currentLastId === 0 || storeIsEmpty;
      const response = await getFinishedScenarios(fetchAll ? undefined : currentLastId);

      if (response.success && Array.isArray(response.data)) {
        const newScenarios = response.data;
        if (fetchAll) {
          finishedScenarios.setAll(newScenarios);
        } else {
          for (const scenario of newScenarios.reverse()) {
            finishedScenarios.prepend(scenario);
          }
        }
        if (newScenarios.length > 0) {
          const maxId = Math.max(...newScenarios.map((s) => s.id));
          lastKnownId.set(maxId);
        }
      }
      loaded = true;
      dataLoaded = true;
    } catch (err) {
      console.error('Failed to load scenarios:', err);
      loaded = true;
    }
  }
</script>

<div class="scenarios-tab">
  {#if !$wsConnected}
    <div class="connection-error">
      Connecting to daemon...
    </div>
  {/if}

  {#if $activeScenariosList.length > 0}
    <section class="section">
      <h2 class="section-title">Active</h2>
      <div class="scenario-list">
        {#each $activeScenariosList as scenario (scenario.id)}
          <ActiveScenario {scenario} />
        {/each}
      </div>
    </section>
  {/if}

  <section class="section">
    <h2 class="section-title">Finished</h2>
    {#if loaded && $finishedScenarios.length === 0}
      <p class="empty-message">No finished scenarios</p>
    {:else}
      <div class="scenario-list">
        {#each $finishedScenarios as scenario, index (index)}
          <FinishedScenario {scenario} />
        {/each}
      </div>
    {/if}
  </section>
</div>

<style>
  .scenarios-tab {
    display: flex;
    flex-direction: column;
    gap: 24px;
  }

  .connection-error {
    padding: 12px 16px;
    background: var(--color-bg-warning);
    border-radius: 8px;
    font-size: 13px;
    color: var(--color-text-muted);
  }

  .section-title {
    font-size: 14px;
    font-weight: 600;
    color: var(--color-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.5px;
    margin-bottom: 12px;
  }

  .scenario-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .empty-message {
    color: var(--color-text-muted);
    font-size: 14px;
  }
</style>
