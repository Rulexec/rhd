# Phase 6: Frontend - Todo List UI Component

## Overview

This phase creates the UI component for displaying the todo list near the role selector. The component shows a button with progress count and expands to show the full list on click.

## Files to Create

### 1. `frontend/src/lib/components/TodoListButton.svelte`

**Component structure:**

```svelte
<script lang="ts">
  import { todoList, hasTodoList, todoListStats } from '../chatStores';
  import type { TodoItem } from '../types/index';
  
  let isExpanded = false;
  
  $: items = $todoList;
  $: stats = $todoListStats;
  $: visible = $hasTodoList;
  
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

{#if visible}
  <div class="todo-list-button-container">
    <button 
      class="todo-list-button"
      on:click={toggleExpand}
      title="Toggle todo list"
    >
      <span class="todo-count">{stats.completed}/{stats.total}</span>
      <span class="todo-icon">📋</span>
    </button>
    
    {#if isExpanded}
      <div class="todo-list-dropdown">
        <div class="todo-list-header">
          <span class="todo-list-title">Task List</span>
          <span class="todo-list-stats">
            {stats.completed} completed, {stats.pending} pending
          </span>
        </div>
        
        <div class="todo-list-items">
          {#each items as item, index}
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
  
  /* Status-specific styles */
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
```

## Files to Modify

### 1. `frontend/src/components/ChatView.svelte` (or parent component containing RoleSelector)

**Import and add TodoListButton:**

```svelte
<script lang="ts">
  // ... existing imports ...
  import TodoListButton from '$lib/components/TodoListButton.svelte';
</script>

<!-- In the template, near RoleSelector -->
<div class="chat-header-controls">
  <RoleSelector />
  <TodoListButton />
</div>
```

**Alternative: Modify RoleSelector.svelte to include TodoListButton:**

```svelte
<script lang="ts">
  // ... existing imports ...
  import TodoListButton from './TodoListButton.svelte';
</script>

{#if $hasRoles && chatId}
  <div class="role-selector">
    <label for="role-select">Role:</label>
    <select
      id="role-select"
      value={selectedValue}
      on:change={handleRoleChange}
      disabled={$isStreaming}
    >
      <option value="">No role</option>
      {#each roles as role}
        <option value="{role.projectName}:{role.roleName}">
          {role.roleName} ({role.projectName})
        </option>
      {/each}
    </select>
  </div>
{/if}

<!-- Always show TodoListButton if there's a todo list -->
<TodoListButton />
```

## UI Behavior

### Button Display

- **Format**: Shows `completed/total` count (e.g., "3/5")
- **Icon**: Clipboard emoji (📋) next to count
- **Visibility**: Only shown when `hasTodoList` is true
- **Position**: Next to role selector in chat header

### Dropdown Behavior

- **Trigger**: Click on button toggles dropdown
- **Position**: Below button, right-aligned
- **Close**: Click outside or click button again
- **Scroll**: Max height with overflow scroll for long lists

### Item Display

- **Numbering**: Sequential numbers (1., 2., 3., ...)
- **Status icons**:
  - ✓ (green) - Completed
  - ◐ (blue) - In Progress
  - ○ (gray) - Pending
  - ✗ (red) - Discarded
- **Text styling**:
  - Completed: strikethrough, gray text
  - In Progress: bold text
  - Pending: normal text
  - Discarded: strikethrough, faded text

### Status Colors

| Status | Icon Color | Text Style |
|--------|-----------|------------|
| Completed | Green (#22c55e) | Strikethrough, gray |
| In Progress | Blue (#3b82f6) | Bold |
| Pending | Gray (#999) | Normal |
| Discarded | Red (#ef4444) | Strikethrough, faded |

## Styling Guidelines

### Match Existing Design

- Use CSS variables from `global.css`:
  - `--border-color`
  - `--bg-primary`
  - `--bg-secondary`
  - `--bg-hover`
  - `--text-primary`
  - `--text-secondary`

- Match RoleSelector styling:
  - Same padding (0.25rem 0.5rem)
  - Same border radius (4px)
  - Same font size (0.875rem)
  - Same border style

### Responsive Design

- Dropdown min-width: 300px
- Dropdown max-width: 500px
- Dropdown max-height: 400px with scroll
- Button adapts to content width

### Accessibility

- Button has `title` attribute for tooltip
- Keyboard navigation support (Tab to button, Enter to toggle)
- Focus styles for keyboard users
- ARIA labels for screen readers (can be added later)

## Tests

### Component Tests

```typescript
import { render, screen, fireEvent } from '@testing-library/svelte';
import { get } from 'svelte/store';
import TodoListButton from './TodoListButton.svelte';
import { todoList } from '../chatStores';
import type { TodoItem } from '../types/index';

describe('TodoListButton', () => {
  beforeEach(() => {
    todoList.set([]);
  });

  test('does not render when todo list is empty', () => {
    render(TodoListButton);
    expect(screen.queryByText(/📋/)).toBeNull();
  });

  test('renders when todo list has items', () => {
    const items: TodoItem[] = [
      { content: 'Task 1', status: 'completed' },
      { content: 'Task 2', status: 'pending' },
    ];
    todoList.set(items);
    
    render(TodoListButton);
    expect(screen.getByText('1/2')).toBeInTheDocument();
  });

  test('shows correct count', () => {
    const items: TodoItem[] = [
      { content: 'Task 1', status: 'completed' },
      { content: 'Task 2', status: 'completed' },
      { content: 'Task 3', status: 'pending' },
    ];
    todoList.set(items);
    
    render(TodoListButton);
    expect(screen.getByText('2/3')).toBeInTheDocument();
  });

  test('toggles dropdown on click', async () => {
    const items: TodoItem[] = [
      { content: 'Task 1', status: 'completed' },
    ];
    todoList.set(items);
    
    render(TodoListButton);
    
    const button = screen.getByRole('button');
    expect(screen.queryByText('Task 1')).toBeNull();
    
    await fireEvent.click(button);
    expect(screen.getByText('Task 1')).toBeInTheDocument();
    
    await fireEvent.click(button);
    expect(screen.queryByText('Task 1')).toBeNull();
  });

  test('displays all todo items', async () => {
    const items: TodoItem[] = [
      { content: 'First task', status: 'completed' },
      { content: 'Second task', status: 'in_progress' },
      { content: 'Third task', status: 'pending' },
    ];
    todoList.set(items);
    
    render(TodoListButton);
    
    const button = screen.getByRole('button');
    await fireEvent.click(button);
    
    expect(screen.getByText('First task')).toBeInTheDocument();
    expect(screen.getByText('Second task')).toBeInTheDocument();
    expect(screen.getByText('Third task')).toBeInTheDocument();
  });

  test('shows status icons', async () => {
    const items: TodoItem[] = [
      { content: 'Completed', status: 'completed' },
      { content: 'In Progress', status: 'in_progress' },
      { content: 'Pending', status: 'pending' },
      { content: 'Discarded', status: 'discarded' },
    ];
    todoList.set(items);
    
    render(TodoListButton);
    
    const button = screen.getByRole('button');
    await fireEvent.click(button);
    
    expect(screen.getByText('✓')).toBeInTheDocument();
    expect(screen.getByText('◐')).toBeInTheDocument();
    expect(screen.getByText('○')).toBeInTheDocument();
    expect(screen.getByText('✗')).toBeInTheDocument();
  });
});
```

### Integration Tests

```typescript
import { render, screen } from '@testing-library/svelte';
import { get } from 'svelte/store';
import ChatView from './ChatView.svelte';
import { todoList } from '../lib/chatStores';
import type { TodoItem } from '../lib/types/index';

describe('ChatView with TodoListButton', () => {
  test('shows todo button when todo list exists', () => {
    const items: TodoItem[] = [
      { content: 'Task', status: 'pending' },
    ];
    todoList.set(items);
    
    render(ChatView);
    expect(screen.getByText(/📋/)).toBeInTheDocument();
  });

  test('hides todo button when todo list is empty', () => {
    todoList.set([]);
    
    render(ChatView);
    expect(screen.queryByText(/📋/)).toBeNull();
  });
});
```

## Implementation Notes

1. **Component placement**: The TodoListButton should be placed near the RoleSelector in the chat header area. If RoleSelector is not visible (no roles), the button should still appear if there's a todo list.

2. **Dropdown positioning**: Uses absolute positioning relative to the button container. Right-aligned to prevent overflow on small screens.

3. **State management**: Uses Svelte stores for reactivity. The component automatically updates when the todo list changes.

4. **Performance**: The dropdown is only rendered when expanded, reducing DOM overhead when not in use.

5. **Accessibility**: Basic keyboard support is included. ARIA labels can be added for better screen reader support.

6. **Styling consistency**: Uses the same CSS variables and styling patterns as the RoleSelector component for visual consistency.

## Dependencies

- This phase depends on Phase 5 (Frontend Stores) for the todo list stores
- This phase is the final frontend phase
- No dependencies on Phase 7 (Contract Injection)
