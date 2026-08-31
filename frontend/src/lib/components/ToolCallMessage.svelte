<script lang="ts">
  import type { ToolCall, StreamToolCallDelta } from '../api/schemas.js';

  interface Props {
    toolCall: ToolCall | StreamToolCallDelta;
    result?: string | null;
  }

  let { toolCall, result = null }: Props = $props();

  let expanded: boolean = $state(false);

  // Extract MCP ID from namespaced format (e.g., "fs1/read_file")
  let mcpId: string | null = $derived.by(() => {
    const name = 'function' in toolCall ? toolCall.function.name : toolCall.name;
    const slashIndex = name.indexOf('/');
    if (slashIndex > 0) {
      return name.substring(0, slashIndex);
    }
    return null;
  });

  let displayName: string = $derived.by(() => {
    const name = 'function' in toolCall ? toolCall.function.name : toolCall.name;
    const slashIndex = name.indexOf('/');
    if (slashIndex > 0) {
      return name.substring(slashIndex + 1);
    }
    return name;
  });

  let argumentsContent: string = $derived.by(() => {
    return 'function' in toolCall ? toolCall.function.arguments : toolCall.arguments;
  });

  let formattedArguments: string = $derived.by(() => {
    try {
      const parsed = JSON.parse(argumentsContent);
      return JSON.stringify(parsed, null, 2);
    } catch {
      return argumentsContent;
    }
  });

  let formattedResult: string = $derived.by(() => {
    if (!result) return '';
    try {
      const parsed = JSON.parse(result);
      return JSON.stringify(parsed, null, 2);
    } catch {
      return result;
    }
  });

  function toggle(): void {
    expanded = !expanded;
  }
</script>

<div class="tool-call" class:expanded>
  <button class="tool-call-header" onclick={toggle}>
    <span class="tool-call-icon">{expanded ? '▼' : '▶'}</span>
    {#if mcpId}
      <span class="mcp-id">{mcpId}</span>
      <span class="separator">/</span>
    {/if}
    <span class="tool-name">{displayName}</span>
    {#if result !== null}
      <span class="result-indicator" title="Has result">✓</span>
    {/if}
  </button>

  {#if expanded}
    <div class="tool-call-body">
      <div class="section">
        <div class="section-label">Arguments</div>
        <pre class="section-content arguments">{formattedArguments}</pre>
      </div>

      {#if result !== null}
        <div class="section">
          <div class="section-label">Result</div>
          <pre class="section-content result">{formattedResult}</pre>
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .tool-call {
    margin-top: var(--spacing-sm);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    overflow: hidden;
  }

  .tool-call-header {
    display: flex;
    align-items: center;
    gap: var(--spacing-xs);
    width: 100%;
    padding: var(--spacing-sm) var(--spacing-md);
    border: none;
    background: var(--color-bg-tertiary);
    color: var(--color-text);
    font-size: var(--font-size-sm);
    cursor: pointer;
    text-align: left;
  }

  .tool-call-header:hover {
    background: var(--color-bg-secondary);
  }

  .tool-call-icon {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
    width: 12px;
  }

  .mcp-id {
    color: var(--color-text-muted);
    font-size: var(--font-size-xs);
  }

  .separator {
    color: var(--color-text-muted);
  }

  .tool-name {
    font-weight: 600;
    color: var(--color-success);
  }

  .result-indicator {
    margin-left: auto;
    color: var(--color-success);
    font-size: var(--font-size-xs);
  }

  .tool-call-body {
    padding: var(--spacing-sm) var(--spacing-md);
    border-top: 1px solid var(--color-border);
  }

  .section {
    margin-bottom: var(--spacing-sm);
  }

  .section:last-child {
    margin-bottom: 0;
  }

  .section-label {
    font-size: var(--font-size-xs);
    font-weight: 600;
    color: var(--color-text-secondary);
    text-transform: uppercase;
    margin-bottom: var(--spacing-xs);
  }

  .section-content {
    margin: 0;
    padding: var(--spacing-sm);
    background: var(--color-bg-secondary);
    border-radius: var(--radius-sm);
    font-size: var(--font-size-xs);
    overflow-x: auto;
    white-space: pre-wrap;
    word-wrap: break-word;
    max-height: 300px;
    overflow-y: auto;
  }

  .section-content.arguments {
    border-left: 3px solid var(--color-primary);
  }

  .section-content.result {
    border-left: 3px solid var(--color-success);
  }
</style>
