import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import { handleChatEvent } from './chatWs';
import { todoList, resetAllStores } from './chatStores';
import type { TodoItem } from './types/index';

describe('handleChatEvent todoListUpdated', () => {
  beforeEach(() => {
    resetAllStores();
  });

  it('updates todoList store', () => {
    const items: TodoItem[] = [
      { content: 'Task 1', status: 'completed' },
      { content: 'Task 2', status: 'in_progress' },
    ];
    
    handleChatEvent('todoListUpdated', {
      chatId: 42,
      items,
    });
    
    expect(get(todoList)).toEqual(items);
  });

  it('replaces existing todo list', () => {
    todoList.set([{ content: 'Old task', status: 'pending' }]);
    
    const newItems: TodoItem[] = [
      { content: 'New task', status: 'completed' },
    ];
    
    handleChatEvent('todoListUpdated', {
      chatId: 42,
      items: newItems,
    });
    
    expect(get(todoList)).toEqual(newItems);
  });

  it('handles empty items array', () => {
    todoList.set([{ content: 'Task', status: 'pending' }]);
    
    handleChatEvent('todoListUpdated', {
      chatId: 42,
      items: [],
    });
    
    expect(get(todoList)).toEqual([]);
  });
});
