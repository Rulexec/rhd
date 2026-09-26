<script lang="ts">
  import commonStyles from '../styles/common.module.css';

  interface Props {
    body: string | null;
    /** Optional heading shown above the body (e.g. "Response Body"). */
    label?: string;
  }

  let { body, label = '' }: Props = $props();

  /** Pretty-print JSON bodies; leave everything else (SSE text) verbatim. */
  function displayText(value: string): string {
    try {
      return JSON.stringify(JSON.parse(value), null, 2);
    } catch {
      return value;
    }
  }
</script>

<div class="raw-body">
  {#if label}
    <div class="raw-body-label {commonStyles['text-muted']}">{label}</div>
  {/if}
  {#if body === null}
    <div class="raw-body-empty {commonStyles['text-muted']}">— nothing recorded —</div>
  {:else}
    <pre class="raw-body-pre">{displayText(body)}</pre>
  {/if}
</div>

<style>
  .raw-body {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-xs);
    min-width: 0;
  }

  .raw-body-label {
    font-size: var(--font-size-xs);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .raw-body-empty {
    padding: var(--spacing-sm);
    font-style: italic;
  }

  .raw-body-pre {
    margin: 0;
    padding: var(--spacing-md);
    background: var(--color-bg-tertiary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: var(--font-size-xs);
    line-height: 1.45;
    white-space: pre-wrap;
    word-break: break-word;
    max-height: 480px;
    overflow: auto;
  }
</style>
