<script lang="ts">
  import { formatDuration, elapsedSeconds } from '@/lib/utils';
  import { abortScenario } from '@/lib/ws';
  import type { ActiveScenario } from '@/lib/types/index';

  let { scenario }: { scenario: ActiveScenario } = $props();

  let elapsed = $state(0);
  let intervalId: ReturnType<typeof setInterval>;

  $effect(() => {
    const startTime = scenario.currentStepStartedAt || scenario.startedAt;
    elapsed = elapsedSeconds(startTime);
    intervalId = setInterval(() => {
      elapsed = elapsedSeconds(startTime);
    }, 1000);

    return () => clearInterval(intervalId);
  });

  function handleAbort() {
    abortScenario(scenario.id);
  }
</script>

<div class="active-scenario">
  <div class="scenario-header">
    <span class="scenario-name">{scenario.scenarioName}</span>
    <span class="badge">running</span>
    <button class="abort-button" onclick={handleAbort}>Abort</button>
  </div>
  <div class="scenario-details">
    {#if scenario.currentStep}
      <span class="detail">Step: <strong>{scenario.currentStep}</strong></span>
    {/if}
    <span class="detail">Elapsed: <strong>{formatDuration(elapsed * 1000)}</strong></span>
  </div>
</div>

<style>
  .active-scenario {
    padding: 12px 16px;
    border: 1px solid var(--color-border);
    border-radius: 8px;
    background: var(--color-bg-active);
  }

  .scenario-header {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 8px;
  }

  .scenario-name {
    font-weight: 600;
    font-size: 15px;
  }

  .badge {
    font-size: 11px;
    padding: 2px 8px;
    border-radius: 12px;
    background: var(--color-primary);
    color: white;
    font-weight: 500;
  }

  .abort-button {
    margin-left: auto;
    padding: 4px 12px;
    font-size: 12px;
    font-weight: 500;
    color: white;
    background: var(--color-danger, #dc2626);
    border: none;
    border-radius: 6px;
    cursor: pointer;
    transition: background 0.2s;
  }

  .abort-button:hover {
    background: var(--color-danger-hover, #b91c1c);
  }

  .scenario-details {
    display: flex;
    gap: 16px;
    font-size: 13px;
    color: var(--color-text-muted);
  }

  .detail strong {
    color: var(--color-text);
  }
</style>
