<script>
  import { formatDateTime, formatDuration, formatCost } from '../lib/utils.js';

  let { scenario } = $props();

  let totalPromptTokens = $derived(
    scenario.tokens?.promptTokens ?? 0
  );
  let totalCompletionTokens = $derived(
    scenario.tokens?.completionTokens ?? 0
  );
</script>

<div class="finished-scenario">
  <div class="scenario-header">
    <span class="scenario-name">{scenario.scenario}</span>
    <span class="timestamp">{formatDateTime(scenario.finished)}</span>
  </div>
  <div class="scenario-details">
    <span class="detail">Duration: <strong>{formatDuration(scenario.durationMs)}</strong></span>
    {#if scenario.tokens}
      <span class="detail">Input: <strong>{totalPromptTokens.toLocaleString()}</strong></span>
      <span class="detail">Output: <strong>{totalCompletionTokens.toLocaleString()}</strong></span>
    {/if}
    {#if scenario.cost !== null && scenario.cost !== undefined}
      <span class="detail">Cost: <strong>{formatCost(scenario.cost)}</strong></span>
    {/if}
  </div>
</div>

<style>
  .finished-scenario {
    padding: 12px 16px;
    border: 1px solid var(--color-border);
    border-radius: 8px;
    background: var(--color-bg);
  }

  .scenario-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 8px;
  }

  .scenario-name {
    font-weight: 600;
    font-size: 15px;
  }

  .timestamp {
    font-size: 12px;
    color: var(--color-text-muted);
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
