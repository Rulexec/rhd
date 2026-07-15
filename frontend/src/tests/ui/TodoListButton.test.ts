import { describe, it, expect, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/svelte';
import TodoListButton from '../../lib/components/TodoListButton.svelte';
import { todoList } from '../../lib/chatStores';
import type { TodoItem } from '../../lib/types/index';

describe('TodoListButton', () => {
  beforeEach(() => {
    todoList.set([]);
  });

  it('does not render when todo list is empty', () => {
    render(TodoListButton);
    expect(screen.queryByText(/📋/)).toBeNull();
  });

  it('renders when todo list has items', () => {
    const items: TodoItem[] = [
      { content: 'Task 1', status: 'completed' },
      { content: 'Task 2', status: 'pending' },
    ];
    todoList.set(items);

    render(TodoListButton);
    expect(screen.getByText('1/2')).toBeTruthy();
  });

  it('shows correct count', () => {
    const items: TodoItem[] = [
      { content: 'Task 1', status: 'completed' },
      { content: 'Task 2', status: 'completed' },
      { content: 'Task 3', status: 'pending' },
    ];
    todoList.set(items);

    render(TodoListButton);
    expect(screen.getByText('2/3')).toBeTruthy();
  });

  it('toggles dropdown on click', async () => {
    const items: TodoItem[] = [
      { content: 'Task 1', status: 'completed' },
    ];
    todoList.set(items);

    render(TodoListButton);

    const button = screen.getByRole('button');
    expect(screen.queryByText('Task 1')).toBeNull();

    await fireEvent.click(button);
    expect(screen.getByText('Task 1')).toBeTruthy();

    await fireEvent.click(button);
    expect(screen.queryByText('Task 1')).toBeNull();
  });

  it('displays all todo items', async () => {
    const items: TodoItem[] = [
      { content: 'First task', status: 'completed' },
      { content: 'Second task', status: 'in_progress' },
      { content: 'Third task', status: 'pending' },
    ];
    todoList.set(items);

    render(TodoListButton);

    const button = screen.getByRole('button');
    await fireEvent.click(button);

    expect(screen.getByText('First task')).toBeTruthy();
    expect(screen.getByText('Second task')).toBeTruthy();
    expect(screen.getByText('Third task')).toBeTruthy();
  });

  it('shows status icons', async () => {
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

    expect(screen.getByText('✓')).toBeTruthy();
    expect(screen.getByText('◐')).toBeTruthy();
    expect(screen.getByText('○')).toBeTruthy();
    expect(screen.getByText('✗')).toBeTruthy();
  });
});
