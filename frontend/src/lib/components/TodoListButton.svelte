<script lang="ts">
  import { todoList, hasTodoList, todoListStats } from '../chatStores';
  import type { TodoItem } from '../types/index';

  let isExpanded = $state(false);

  function toggleExpand() {
    isExpanded = !isExpanded;
  }

  function getStatusIcon(status: TodoItem['status']): string {
    switch (status) {
      case 'completed':
        return '✓';
      case 'in_progress':
        return '◐';
      case 'pending':
        return '○';
      case 'discarded':
        return '✗';
      default:
        return '○';
    }
  }

  function getStatusClass(status: TodoItem['status']): string {
    switch (status) {
      case 'completed':
        return 'status-completed';
      case 'in_progress':
        return 'status-in-progress';
      case 'pending':
        return 'status-pending';
      case 'discarded':
        return 'status-discarded';
      default:
        return 'status-pending';
    }
  }
</script>

{#if $hasTodoList}
  <div class="todo-list-button-container">
    <button
      class="todo-list-button"
      onclick={toggleExpand}
      title="Toggle todo list"
    >
      <span class="todo-count">{$todoListStats.completed}/{$todoListStats.total}</span>
      <span class="todo-icon">📋</span>
    </button>

    {#if isExpanded}
      <div class="todo-list-dropdown">
        <div class="todo-list-header">
          <span class="todo-list-title">Task List</span>
          <span class="todo-list-stats">
            {$todoListStats.completed} completed, {$todoListStats.pending} pending
          </span>
        </div>

        <div class="todo-list-items">
          {#each $todoList as item, index}
            <div class="todo-item {getStatusClass(item.status)}">
              <span class="todo-item-number">{index + 1}.</span>
              <span class="todo-item-icon">{getStatusIcon(item.status)}</span>
              <span class="todo-item-content">{item.content}</span>
            </div>
          {/each}
        </div>
      </div>
    {/if}
  </div>
{/if}

<style>
  .todo-list-button-container {
    position: relative;
    display: inline-block;
  }

  .todo-list-button {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    padding: 0.25rem 0.5rem;
    border: 1px solid var(--border-color, #e0e0e0);
    border-radius: 4px;
    background: var(--bg-primary, #fff);
    color: var(--text-primary, #333);
    font-size: 0.875rem;
    cursor: pointer;
    transition: background-color 0.2s;
  }

  .todo-list-button:hover {
    background: var(--bg-hover, #f5f5f5);
  }

  .todo-count {
    font-weight: 500;
  }

  .todo-icon {
    font-size: 1rem;
  }

  .todo-list-dropdown {
    position: absolute;
    top: 100%;
    right: 0;
    margin-top: 0.25rem;
    min-width: 300px;
    max-width: 500px;
    max-height: 400px;
    overflow-y: auto;
    background: var(--bg-primary, #fff);
    border: 1px solid var(--border-color, #e0e0e0);
    border-radius: 4px;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
    z-index: 1000;
  }

  .todo-list-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.5rem 0.75rem;
    border-bottom: 1px solid var(--border-color, #e0e0e0);
    background: var(--bg-secondary, #f9f9f9);
  }

  .todo-list-title {
    font-weight: 600;
    font-size: 0.875rem;
  }

  .todo-list-stats {
    font-size: 0.75rem;
    color: var(--text-secondary, #666);
  }

  .todo-list-items {
    padding: 0.5rem 0;
  }

  .todo-item {
    display: flex;
    align-items: flex-start;
    gap: 0.5rem;
    padding: 0.375rem 0.75rem;
    font-size: 0.875rem;
    line-height: 1.4;
  }

  .todo-item-number {
    color: var(--text-secondary, #666);
    font-weight: 500;
    min-width: 1.5rem;
  }

  .todo-item-icon {
    font-size: 0.875rem;
    min-width: 1rem;
    text-align: center;
  }

  .todo-item-content {
    flex: 1;
    word-break: break-word;
  }

  .status-completed .todo-item-content {
    color: var(--text-secondary, #666);
    text-decoration: line-through;
  }

  .status-completed .todo-item-icon {
    color: #22c55e;
  }

  .status-in-progress .todo-item-icon {
    color: #3b82f6;
  }

  .status-in-progress .todo-item-content {
    font-weight: 500;
  }

  .status-pending .todo-item-icon {
    color: var(--text-secondary, #999);
  }

  .status-discarded .todo-item-content {
    color: var(--text-secondary, #999);
    text-decoration: line-through;
    opacity: 0.6;
  }

  .status-discarded .todo-item-icon {
    color: #ef4444;
  }
</style>
