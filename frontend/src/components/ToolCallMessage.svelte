<script lang="ts">
  import type { ToolCall } from '../lib/types/index';

  export let toolCall: ToolCall;

  let expanded = false;
  let showArguments = false;
  let showResult = false;

  $: statusIcon = getStatusIcon(toolCall.status);
  $: statusClass = getStatusClass(toolCall.status);

  function getStatusIcon(status?: string): string {
    switch (status) {
      case 'running':
        return '⏳';
      case 'completed':
        return '✓';
      case 'failed':
        return '✗';
      case 'pending':
        return '○';
      default:
        return '○';
    }
  }

  function getStatusClass(status?: string): string {
    switch (status) {
      case 'running':
        return 'status-running';
      case 'completed':
        return 'status-completed';
      case 'failed':
        return 'status-failed';
      case 'pending':
        return 'status-pending';
      default:
        return 'status-pending';
    }
  }

  function formatJson(str: string): string {
    try {
      return JSON.stringify(JSON.parse(str), null, 2);
    } catch {
      return str;
    }
  }
</script>

<div class="tool-call">
  <button class="tool-header" on:click={() => (expanded = !expanded)}>
    <span class="toggle-icon">{expanded ? '▼' : '▶'}</span>
    <span class="tool-status {statusClass}">{statusIcon}</span>
    {#if toolCall.mcpId}
      <span class="mcp-name">MCP: {toolCall.mcpId}</span>
      <span class="separator">-</span>
    {/if}
    <span class="tool-name">{toolCall.name}</span>
  </button>

  {#if expanded}
    {#if toolCall.arguments}
      <div class="tool-section">
        <button
          class="section-toggle"
          on:click={() => (showArguments = !showArguments)}
        >
          <span class="toggle-icon">{showArguments ? '▼' : '▶'}</span>
          Arguments
        </button>
        {#if showArguments}
          <pre class="section-content">{formatJson(toolCall.arguments)}</pre>
        {/if}
      </div>
    {/if}

    {#if toolCall.result !== undefined}
      <div class="tool-section">
        <button
          class="section-toggle"
          on:click={() => (showResult = !showResult)}
        >
          <span class="toggle-icon">{showResult ? '▼' : '▶'}</span>
          Result
        </button>
        {#if showResult}
          <pre class="section-content">{formatJson(toolCall.result)}</pre>
        {/if}
      </div>
    {/if}
  {/if}
</div>

<style>
  .tool-call {
    background: var(--color-bg-secondary, #f8f9fa);
    border: 1px solid var(--color-border, #e0e0e0);
    border-radius: 6px;
    padding: var(--spacing-s, 8px);
    margin: var(--spacing-s, 8px) 0;
    font-size: 13px;
  }

  .tool-header {
    display: flex;
    align-items: center;
    gap: var(--spacing-xs, 4px);
    font-weight: 500;
    background: none;
    border: none;
    padding: 0;
    cursor: pointer;
    width: 100%;
    text-align: left;
  }

  .tool-status {
    font-size: 14px;
  }

  .mcp-name {
    color: var(--color-text-secondary, #666);
    font-size: 12px;
  }

  .separator {
    color: var(--color-text-secondary, #999);
  }

  .status-running {
    color: #0066cc;
  }

  .status-completed {
    color: #28a745;
  }

  .status-failed {
    color: #dc3545;
  }

  .status-pending {
    color: #6c757d;
  }

  .tool-name {
    font-family: monospace;
    color: var(--color-text, #212529);
  }

  .tool-section {
    margin-top: var(--spacing-xs, 4px);
  }

  .section-toggle {
    background: none;
    border: none;
    padding: 2px 4px;
    cursor: pointer;
    font-size: 12px;
    color: var(--color-text-secondary, #666);
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .section-toggle:hover {
    color: var(--color-text, #212529);
  }

  .toggle-icon {
    font-size: 10px;
  }

  .section-content {
    background: var(--color-bg, #fff);
    border: 1px solid var(--color-border, #e0e0e0);
    border-radius: 4px;
    padding: var(--spacing-xs, 8px);
    margin-top: var(--spacing-xs, 4px);
    font-family: monospace;
    font-size: 12px;
    overflow-x: auto;
    white-space: pre-wrap;
    word-wrap: break-word;
    max-height: 300px;
    overflow-y: auto;
  }
</style>
