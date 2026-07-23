<script lang="ts">
  import { onMount } from 'svelte';
  import { projects, chatProjects, mcpStatuses } from '@/lib/projectStores';
  import { currentChatId } from '@/lib/chatStores';
  import { loadProjects, attachProject, detachProject, loadMcpStatus } from '@/lib/projectStores';

  let attachError = '';
  let errorTimeout: ReturnType<typeof setTimeout> | null = null;

  function isAttached(projectName: string): boolean {
    return $chatProjects.some((p) => p.name === projectName);
  }

  function hasMcpErrors(projectName: string): boolean {
    return $mcpStatuses
      .filter((s) => s.projectName === projectName)
      .some((s) => s.status === 'failed');
  }

  function hasConnectingMcp(projectName: string): boolean {
    return $mcpStatuses
      .filter((s) => s.projectName === projectName)
      .some((s) => s.status === 'connecting');
  }

  async function toggleProject(projectName: string) {
    const chatId = $currentChatId;
    if (!chatId) return;

    if (isAttached(projectName)) {
      await detachProject(chatId, projectName);
    } else {
      const result = await attachProject(chatId, projectName);
      if (!result.success) {
        attachError = result.error || 'Failed to attach project';
        if (errorTimeout) clearTimeout(errorTimeout);
        errorTimeout = setTimeout(() => {
          attachError = '';
        }, 5000);
      } else {
        await loadMcpStatus(projectName);
      }
    }
  }

  onMount(() => {
    loadProjects();
  });
</script>

<div class="projects-panel">
  <h3 class="panel-title">Projects</h3>
  {#if attachError}
    <div class="error-toast">{attachError}</div>
  {/if}
  {#if $projects.length === 0}
    <p class="empty-text">No projects available</p>
  {:else}
    <div class="project-list">
      {#each $projects as project (project.name)}
        {@const attached = isAttached(project.name)}
        <div class="project-item" class:attached>
          <div class="project-info">
            <span class="project-name">{project.name}</span>
            <div class="project-badges">
              {#if project.hasMcp}
                <span class="badge mcp">MCP</span>
              {/if}
              {#if project.hasSystemPrompt}
                <span class="badge prompt">Prompt</span>
              {/if}
            </div>
          </div>
          <button
            class="toggle-btn"
            class:attached
            on:click={() => toggleProject(project.name)}
            disabled={attached && hasConnectingMcp(project.name)}
          >
            {attached ? 'Detach' : 'Attach'}
          </button>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .projects-panel {
    border-top: 1px solid var(--color-border);
    padding: var(--spacing-m);
  }

  .panel-title {
    font-size: 13px;
    font-weight: 600;
    text-transform: uppercase;
    color: var(--color-text-muted);
    margin-bottom: var(--spacing-s);
  }

  .empty-text {
    font-size: 13px;
    color: var(--color-text-muted);
  }

  .project-list {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-xs);
  }

  .project-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--spacing-s);
    border-radius: 4px;
    border: 1px solid var(--color-border);
  }

  .project-item.attached {
    background: var(--color-bg-active);
    border-color: var(--color-primary);
  }

  .project-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .project-name {
    font-size: 14px;
    font-weight: 500;
  }

  .project-badges {
    display: flex;
    gap: 4px;
  }

  .badge {
    font-size: 10px;
    padding: 1px 4px;
    border-radius: 3px;
    font-weight: 500;
  }

  .badge.mcp {
    background: #e0f2fe;
    color: #0369a1;
  }

  .badge.prompt {
    background: #fef3c7;
    color: #92400e;
  }

  .toggle-btn {
    padding: var(--spacing-xs) var(--spacing-s);
    border: 1px solid var(--color-border);
    border-radius: 4px;
    background: var(--color-bg);
    cursor: pointer;
    font-size: 12px;
  }

  .toggle-btn.attached {
    background: var(--color-primary);
    color: white;
    border-color: var(--color-primary);
  }

  .toggle-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .error-toast {
    background: #fee2e2;
    color: #991b1b;
    padding: var(--spacing-s);
    border-radius: 4px;
    margin-bottom: var(--spacing-s);
    font-size: 13px;
    border: 1px solid #fecaca;
  }
</style>
