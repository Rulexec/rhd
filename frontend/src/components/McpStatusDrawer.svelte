<script lang="ts">
  import { chatProjects, mcpStatuses } from '../lib/projectStores';

  export let visible: boolean = false;

  function getStatusColor(status: string): string {
    switch (status) {
      case 'connected': return '#22c55e';
      case 'connecting': return '#eab308';
      case 'failed': return '#ef4444';
      default: return '#9ca3af';
    }
  }

  function getProjectStatuses(projectName: string) {
    return $mcpStatuses.filter((s) => s.projectName === projectName);
  }
</script>

{#if visible}
  <div class="drawer-overlay" role="button" tabindex="0" on:click on:keydown={(e) => e.key === 'Escape'}>
    <div class="drawer" role="dialog" tabindex="-1" on:click|stopPropagation on:keydown|stopPropagation>
      <div class="drawer-header">
        <h3>MCP Status</h3>
        <button class="close-btn" on:click>×</button>
      </div>
      <div class="drawer-content">
        {#if $chatProjects.length === 0}
          <p class="empty-text">No projects attached</p>
        {:else}
          {#each $chatProjects as chatProject (chatProject.name)}
            {@const statuses = getProjectStatuses(chatProject.name)}
            <div class="project-section">
              <h4 class="project-name">{chatProject.name}</h4>
              {#if statuses.length === 0}
                <p class="no-mcp">No MCP servers</p>
              {:else}
                <div class="mcp-list">
                  {#each statuses as status (status.mcpId)}
                    <div class="mcp-item">
                      <span class="status-dot" style="background: {getStatusColor(status.status)}"></span>
                      <span class="mcp-name">{status.mcpId}</span>
                      <span class="status-label">{status.status}</span>
                    </div>
                    {#if status.error}
                      <p class="error-text">{status.error}</p>
                    {/if}
                  {/each}
                </div>
              {/if}
            </div>
          {/each}
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .drawer-overlay {
    position: fixed;
    top: 0;
    right: 0;
    bottom: 0;
    left: 0;
    background: rgba(0, 0, 0, 0.3);
    z-index: 100;
    display: flex;
    justify-content: flex-end;
  }

  .drawer {
    width: 320px;
    background: var(--color-bg);
    height: 100%;
    box-shadow: -2px 0 8px rgba(0, 0, 0, 0.15);
    display: flex;
    flex-direction: column;
  }

  .drawer-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: var(--spacing-m) var(--spacing-l);
    border-bottom: 1px solid var(--color-border);
  }

  .drawer-header h3 {
    font-size: 16px;
    font-weight: 600;
    margin: 0;
  }

  .close-btn {
    background: none;
    border: none;
    font-size: 24px;
    cursor: pointer;
    color: var(--color-text-muted);
    padding: 0;
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 4px;
  }

  .close-btn:hover {
    background: rgba(0, 0, 0, 0.1);
    color: var(--color-text);
  }

  .drawer-content {
    flex: 1;
    overflow-y: auto;
    padding: var(--spacing-m) var(--spacing-l);
  }

  .empty-text {
    color: var(--color-text-muted);
    font-size: 14px;
  }

  .project-section {
    margin-bottom: var(--spacing-l);
  }

  .project-name {
    font-size: 14px;
    font-weight: 600;
    margin-bottom: var(--spacing-s);
  }

  .no-mcp {
    font-size: 13px;
    color: var(--color-text-muted);
  }

  .mcp-list {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-xs);
  }

  .mcp-item {
    display: flex;
    align-items: center;
    gap: var(--spacing-s);
    font-size: 13px;
  }

  .status-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .mcp-name {
    flex: 1;
  }

  .status-label {
    color: var(--color-text-muted);
    font-size: 12px;
  }

  .error-text {
    font-size: 12px;
    color: #ef4444;
    margin-left: 16px;
    margin-top: 2px;
  }
</style>
