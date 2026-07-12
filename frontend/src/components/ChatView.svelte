<script lang="ts">
  import { currentChat } from '../lib/chatStores';
  import { chatProjects, mcpStatuses } from '../lib/projectStores';
  import MessageList from './MessageList.svelte';
  import MessageInput from './MessageInput.svelte';
  import McpStatusDrawer from './McpStatusDrawer.svelte';
  import ProjectsPanel from './ProjectsPanel.svelte';

  let showMcpDrawer = $state(false);
  let showProjectsPanel = $state(false);

  function getProjectMcpStatus(projectName: string): string {
    const statuses = $mcpStatuses.filter((s) => s.projectName === projectName);
    if (statuses.length === 0) return 'unknown';
    if (statuses.some((s) => s.status === 'failed')) return 'failed';
    if (statuses.some((s) => s.status === 'connecting')) return 'connecting';
    return 'connected';
  }

  function getStatusColor(status: string): string {
    switch (status) {
      case 'connected': return '#22c55e';
      case 'connecting': return '#eab308';
      case 'failed': return '#ef4444';
      default: return '#9ca3af';
    }
  }
</script>

<div class="chat-view">
  <div class="header">
    <div class="header-row">
      <h2>{$currentChat?.title || 'Chat'}</h2>
      <div class="header-actions">
        <button
          class="header-btn"
          class:active={showProjectsPanel}
          onclick={() => showProjectsPanel = !showProjectsPanel}
        >
          Projects
        </button>
        {#if $chatProjects.length > 0}
          <button
            class="header-btn"
            class:active={showMcpDrawer}
            onclick={() => showMcpDrawer = !showMcpDrawer}
          >
            MCP Status
          </button>
        {/if}
      </div>
    </div>
    {#if $chatProjects.length > 0}
      <div class="project-chips">
        {#each $chatProjects as project (project.name)}
          {@const status = getProjectMcpStatus(project.name)}
          <span class="chip">
            <span class="chip-dot" style="background: {getStatusColor(status)}"></span>
            {project.name}
          </span>
        {/each}
      </div>
    {/if}
  </div>
  {#if showProjectsPanel}
    <div class="projects-panel-container">
      <ProjectsPanel />
    </div>
  {/if}
  <MessageList />
  <MessageInput />
</div>

<McpStatusDrawer bind:visible={showMcpDrawer} />

<style>
  .chat-view {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .header {
    padding: var(--spacing-m) var(--spacing-l);
    border-bottom: 1px solid var(--color-border);
    background: var(--color-bg);
  }

  .header-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .header h2 {
    font-size: 18px;
    font-weight: 600;
    margin: 0;
  }

  .header-actions {
    display: flex;
    gap: var(--spacing-s);
  }

  .header-btn {
    padding: var(--spacing-xs) var(--spacing-s);
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-bg);
    cursor: pointer;
    font-size: 13px;
  }

  .header-btn.active {
    background: var(--color-primary);
    color: white;
    border-color: var(--color-primary);
  }

  .project-chips {
    display: flex;
    gap: var(--spacing-s);
    margin-top: var(--spacing-s);
    flex-wrap: wrap;
  }

  .chip {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 2px 8px;
    border-radius: 12px;
    background: var(--color-bg-active);
    font-size: 12px;
    border: 1px solid var(--color-border);
  }

  .chip-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
  }

  .projects-panel-container {
    border-bottom: 1px solid var(--color-border);
  }
</style>
