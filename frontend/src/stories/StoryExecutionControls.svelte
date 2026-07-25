<script lang="ts">
  import { stepButtonTestId } from '@/stories/testIds';

  export interface StepDefinition<T> {
    name: string;
    execute: (options: { state: T }) => Promise<{ state: T }>;
  }
  
  export interface StoryControlDefinition<T> {
    getInitialState: () => T;
    steps: StepDefinition<T>[];
  }
  
  let { story }: { story: StoryControlDefinition<any> } = $props();
  
  // svelte-ignore state_referenced_locally
  let currentState = $state(story.getInitialState());
  let currentStepIndex = $state(0);
  let isExecuting = $state(false);
  let error = $state<string | null>(null);
  
  let currentStep = $derived(story.steps[currentStepIndex]);
  let isComplete = $derived(currentStepIndex >= story.steps.length);
  
  async function executeStep() {
    if (!currentStep || isExecuting || isComplete) return;
    
    isExecuting = true;
    error = null;
    
    try {
      const result = await currentStep.execute({ state: currentState });
      currentState = result.state;
      currentStepIndex += 1;
    } catch (err) {
      error = err instanceof Error ? err.message : 'Unknown error';
      console.error('Step execution failed:', err);
    } finally {
      isExecuting = false;
    }
  }
</script>

<div class="story-controls">
  <div class="controls-header">
    <h3>Story Execution Controls</h3>
  </div>
  
  {#if error}
    <div class="error-message">{error}</div>
  {/if}
  
  <div class="step-info">
    <span>Step {currentStepIndex + 1} of {story.steps.length}</span>
    {#if isComplete}
      <span class="complete">✓ All steps completed</span>
    {/if}
  </div>
  
  <button
    onclick={executeStep}
    disabled={isExecuting || isComplete}
    class="execute-btn"
    data-testid={stepButtonTestId(currentStepIndex)}
  >
    {#if isExecuting}
      Executing...
    {:else if isComplete}
      All steps completed
    {:else}
      {currentStep.name}
    {/if}
  </button>
</div>

<style>
  .story-controls {
    padding: 16px;
    border: 1px solid #ddd;
    border-radius: 8px;
    background: #f9f9f9;
    margin-bottom: 16px;
  }
  
  .controls-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 12px;
  }
  
  .controls-header h3 {
    margin: 0;
    font-size: 16px;
  }
  
  .step-info {
    display: flex;
    justify-content: space-between;
    margin-bottom: 12px;
    font-size: 14px;
    color: #666;
  }
  
  .complete {
    color: #28a745;
    font-weight: bold;
  }
  
  .execute-btn {
    width: 100%;
    padding: 12px;
    font-size: 14px;
    font-weight: 500;
    background: #007bff;
    color: white;
    border: none;
    border-radius: 4px;
    cursor: pointer;
    transition: background 0.2s;
  }
  
  .execute-btn:hover:not(:disabled) {
    background: #0056b3;
  }
  
  .execute-btn:disabled {
    background: #ccc;
    cursor: not-allowed;
  }
  
  .error-message {
    background: #fee;
    border: 1px solid #fcc;
    color: #c33;
    padding: 8px;
    border-radius: 4px;
    margin-bottom: 12px;
    font-size: 13px;
  }
</style>
