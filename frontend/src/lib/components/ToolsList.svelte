<script lang="ts">
  import type { ToolInfo } from '../api/schemas.js';

  interface Props {
    tools: ToolInfo[];
  }

  let { tools }: Props = $props();
</script>

<div class="tools-list">
  {#if tools.length === 0}
    <div class="tools-empty">
      <span class="text-muted">No tools registered for this chat</span>
    </div>
  {:else}
    <div class="tools-items">
      {#each tools as tool (tool.tool.function.name)}
        <div class="tool-item">
          <div class="tool-header">
            <h3 class="tool-name">{tool.tool.function.name}</h3>
            <span class="tool-plugin">by {tool.pluginId}</span>
          </div>
          <p class="tool-description">{tool.tool.function.description}</p>
          <details class="tool-parameters">
            <summary>Parameters</summary>
            <pre><code>{JSON.stringify(tool.tool.function.parameters, null, 2)}</code></pre>
          </details>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .tools-list {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow-y: auto;
    padding: var(--spacing-md);
  }

  .tools-empty {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: var(--color-text-muted);
  }

  .tools-items {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-md);
  }

  .tool-item {
    padding: var(--spacing-md);
    background: var(--color-bg-secondary);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
  }

  .tool-header {
    display: flex;
    align-items: baseline;
    gap: var(--spacing-sm);
    margin-bottom: var(--spacing-xs);
  }

  .tool-name {
    margin: 0;
    font-size: var(--font-size-base);
    font-weight: 600;
    color: var(--color-text);
  }

  .tool-plugin {
    font-size: var(--font-size-xs);
    color: var(--color-text-muted);
  }

  .tool-description {
    margin: 0;
    font-size: var(--font-size-sm);
    color: var(--color-text-secondary);
    line-height: 1.5;
  }

  .tool-parameters {
    margin-top: var(--spacing-sm);
  }

  .tool-parameters summary {
    cursor: pointer;
    font-size: var(--font-size-sm);
    font-weight: 500;
    color: var(--color-text-secondary);
    user-select: none;
  }

  .tool-parameters summary:hover {
    color: var(--color-text);
  }

  .tool-parameters pre {
    margin: var(--spacing-xs) 0 0 0;
    padding: var(--spacing-sm);
    background: var(--color-bg-tertiary);
    border-radius: var(--radius-sm);
    white-space: pre-wrap;
    word-wrap: break-word;
    overflow-wrap: break-word;
  }

  .tool-parameters code {
    font-family: var(--font-mono);
    font-size: var(--font-size-xs);
    color: var(--color-text);
    word-wrap: break-word;
    overflow-wrap: break-word;
  }

  .text-muted {
    color: var(--color-text-muted);
  }
</style>
