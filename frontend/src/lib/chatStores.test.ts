import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import { todoList, hasTodoList, todoListStats, resetAllStores } from './chatStores';
import type { TodoItem } from './types/index';

describe('todoList stores', () => {
  beforeEach(() => {
    resetAllStores();
  });

  it('todoList initial value is empty array', () => {
    expect(get(todoList)).toEqual([]);
  });

  it('todoList can be updated', () => {
    const items: TodoItem[] = [
      { content: 'Task 1', status: 'completed' },
      { content: 'Task 2', status: 'pending' },
    ];
    todoList.set(items);
    expect(get(todoList)).toEqual(items);
  });

  it('hasTodoList is false when empty', () => {
    expect(get(hasTodoList)).toBe(false);
  });

  it('hasTodoList is true when items exist', () => {
    todoList.set([{ content: 'Task', status: 'pending' }]);
    expect(get(hasTodoList)).toBe(true);
  });

  it('todoListStats calculates correct counts', () => {
    const items: TodoItem[] = [
      { content: 'Task 1', status: 'completed' },
      { content: 'Task 2', status: 'completed' },
      { content: 'Task 3', status: 'in_progress' },
      { content: 'Task 4', status: 'pending' },
      { content: 'Task 5', status: 'pending' },
      { content: 'Task 6', status: 'pending' },
      { content: 'Task 7', status: 'discarded' },
    ];
    todoList.set(items);
    
    const stats = get(todoListStats);
    expect(stats.total).toBe(7);
    expect(stats.completed).toBe(2);
    expect(stats.inProgress).toBe(1);
    expect(stats.pending).toBe(3);
    expect(stats.discarded).toBe(1);
  });

  it('todoListStats handles empty list', () => {
    const stats = get(todoListStats);
    expect(stats.total).toBe(0);
    expect(stats.completed).toBe(0);
    expect(stats.pending).toBe(0);
    expect(stats.inProgress).toBe(0);
    expect(stats.discarded).toBe(0);
  });

  it('resetAllStores clears todoList', () => {
    todoList.set([{ content: 'Task', status: 'pending' }]);
    expect(get(todoList)).toHaveLength(1);
    
    resetAllStores();
    
    expect(get(todoList)).toEqual([]);
  });
});
