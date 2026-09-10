<script lang="ts">
  import type { ChoiceToolArgs } from '../api/schemas.js';

  interface Props {
    args: ChoiceToolArgs;
    /** True once a tool-role message with this call's id exists. */
    resolved: boolean;
    /** The answer text (from toolResults) when resolved. */
    answer?: string | null;
    /** Disable interaction (e.g. no responder wired). */
    disabled?: boolean;
    /** Send the answer: exact option text or the user's typed message. */
    onRespond: (content: string) => void;
  }

  let { args, resolved, answer = null, disabled = false, onRespond }: Props = $props();

  let manualText: string = $state('');
  // Optimistic guard: disable controls immediately after the first submit;
  // the `resolved` prop (driven by the messageAdded round-trip) is the final truth.
  let sentLocally: boolean = $state(false);

  let controlsDisabled: boolean = $derived(resolved || sentLocally || disabled);

  function chooseOption(option: string): void {
    if (controlsDisabled) return;
    sentLocally = true;
    onRespond(option);
  }

  function submitManual(): void {
    const text = manualText.trim();
    if (!text || controlsDisabled) return;
    sentLocally = true;
    onRespond(text);
    manualText = '';
  }
</script>

<div class="choice-prompt" class:resolved>
  <div class="choice-question">{args.question}</div>

  {#if resolved}
    <div class="choice-answer">
      <span class="choice-answer-label">Answered:</span>
      <span class="choice-answer-text">{answer ?? ''}</span>
    </div>
  {:else}
    <div class="choice-options">
      {#each args.options as option}
        <button class="choice-option" disabled={controlsDisabled} onclick={() => chooseOption(option)}>
          {option}
        </button>
      {/each}
    </div>

    <form class="choice-manual" onsubmit={(e) => { e.preventDefault(); submitManual(); }}>
      <input
        type="text"
        class="choice-manual-input"
        placeholder="Or type your own answer..."
        bind:value={manualText}
        disabled={controlsDisabled}
        aria-label="Manual answer"
      />
      <button
        type="submit"
        class="choice-manual-submit"
        disabled={controlsDisabled || manualText.trim().length === 0}
      >
        Send
      </button>
    </form>
  {/if}
</div>

<style>
  .choice-prompt {
    margin-top: var(--spacing-sm);
    padding: var(--spacing-md);
    background: var(--color-bg-tertiary);
    border: 1px solid var(--color-border);
    border-left: 3px solid var(--color-primary);
    border-radius: var(--radius-sm);
  }

  .choice-question {
    font-size: var(--font-size-sm);
    font-weight: 600;
    color: var(--color-text);
    margin-bottom: var(--spacing-sm);
  }

  .choice-options {
    display: flex;
    flex-wrap: wrap;
    gap: var(--spacing-xs);
    margin-bottom: var(--spacing-sm);
  }

  .choice-option {
    padding: var(--spacing-xs) var(--spacing-md);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-full);
    background: transparent;
    color: var(--color-text);
    font-size: var(--font-size-sm);
    cursor: pointer;
    transition: background var(--transition-fast);
  }

  .choice-option:hover:not(:disabled) {
    background: var(--color-bg-secondary);
  }

  .choice-option:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .choice-manual {
    display: flex;
    gap: var(--spacing-xs);
  }

  .choice-manual-input {
    flex: 1;
    min-width: 0;
    padding: var(--spacing-xs) var(--spacing-sm);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    background: var(--color-bg-secondary);
    color: var(--color-text);
    font-size: var(--font-size-sm);
  }

  .choice-manual-input:disabled {
    opacity: 0.5;
  }

  .choice-manual-submit {
    padding: var(--spacing-xs) var(--spacing-md);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-full);
    background: var(--color-bg-secondary);
    color: var(--color-text);
    font-size: var(--font-size-sm);
    cursor: pointer;
    transition: background var(--transition-fast);
  }

  .choice-manual-submit:hover:not(:disabled) {
    background: var(--color-bg-tertiary);
  }

  .choice-manual-submit:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .choice-answer {
    font-size: var(--font-size-sm);
  }

  .choice-answer-label {
    font-weight: 600;
    color: var(--color-success);
  }

  .choice-answer-text {
    color: var(--color-text-muted);
  }

  .choice-prompt.resolved {
    border-left-color: var(--color-success);
  }
</style>
