<script lang="ts">
  import { pausedScenarios } from '../lib/stores';
  import { retryScenario, abortScenarioWithError } from '../lib/ws';
  import type { PausedScenario } from '../lib/types/index';

  let { scenario }: { scenario: PausedScenario } = $props();

  let selectedModel = $state<string | null>(null);
  let retrying = $state(false);

  $effect(() => {
    if (scenario.availableModels.length > 0 && selectedModel === null) {
      selectedModel = scenario.availableModels[0];
    }
  });

  async function handleRetry() {
    retrying = true;
    try {
      await retryScenario(scenario.executionId, selectedModel || undefined);
    } finally {
      retrying = false;
    }
  }

  function handleAbort() {
    abortScenarioWithError(scenario.executionId);
  }

  function handleModelChange(event: Event) {
    const target = event.target as HTMLSelectElement;
    selectedModel = target.value || null;
    pausedScenarios.updateSelectedModel(scenario.executionId, selectedModel);
  }
</script>

<div class="paused-scenario">
  <div class="scenario-header">
    <span class="scenario-name">{scenario.scenarioName}</span>
    <span class="badge paused">paused</span>
  </div>
  <div class="scenario-details">
    <div class="detail">
      <span class="label">Step:</span>
      <strong>{scenario.stepName}</strong>
    </div>
    <div class="detail error">
      <span class="label">Error:</span>
      <span class="error-message">{scenario.error}</span>
    </div>
  </div>
  <div class="scenario-actions">
    <div class="model-selector">
      <label for="model-select">Retry with model:</label>
      <select id="model-select" value={selectedModel || ''} onchange={handleModelChange}>
        {#each scenario.availableModels as model}
          <option value={model}>{model}</option>
        {/each}
      </select>
    </div>
    <div class="action-buttons">
      <button class="retry-button" onclick={handleRetry} disabled={retrying}>
        {retrying ? 'Retrying...' : 'Retry'}
      </button>
      <button class="abort-button" onclick={handleAbort}>Abort</button>
    </div>
  </div>
</div>

<style>
  .paused-scenario {
    padding: 12px 16px;
    border: 1px solid var(--color-border);
    border-radius: 8px;
    background: var(--color-bg-paused, #fff3cd);
    margin-bottom: 8px;
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
    font-weight: 500;
  }

  .badge.paused {
    background: var(--color-warning, #ffc107);
    color: #000;
  }

  .scenario-details {
    display: flex;
    flex-direction: column;
    gap: 8px;
    font-size: 13px;
    color: var(--color-text-muted);
    margin-bottom: 12px;
  }

  .detail {
    display: flex;
    gap: 8px;
  }

  .detail .label {
    color: var(--color-text-muted);
  }

  .detail strong {
    color: var(--color-text);
  }

  .detail.error {
    color: var(--color-danger, #dc2626);
  }

  .error-message {
    word-break: break-word;
  }

  .scenario-actions {
    display: flex;
    align-items: center;
    gap: 16px;
    flex-wrap: wrap;
  }

  .model-selector {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
  }

  .model-selector label {
    color: var(--color-text-muted);
  }

  .model-selector select {
    padding: 4px 8px;
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: white;
    font-size: 13px;
  }

  .action-buttons {
    display: flex;
    gap: 8px;
    margin-left: auto;
  }

  .retry-button,
  .abort-button {
    padding: 6px 12px;
    font-size: 13px;
    font-weight: 500;
    border: none;
    border-radius: 6px;
    cursor: pointer;
    transition: background 0.2s;
  }

  .retry-button {
    color: white;
    background: var(--color-primary, #007bff);
  }

  .retry-button:hover {
    background: var(--color-primary-hover, #0056b3);
  }

  .abort-button {
    color: white;
    background: var(--color-danger, #dc2626);
  }

  .abort-button:hover {
    background: var(--color-danger-hover, #b91c1c);
  }
</style>
