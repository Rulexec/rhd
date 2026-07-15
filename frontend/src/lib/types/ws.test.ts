import { describe, it, expect } from 'vitest';
import { TodoListUpdatedEventSchema, TodoItemSchema } from './ws';

describe('TodoItemSchema', () => {
  it('validates correct todo item', () => {
    const item = {
      content: 'Task 1',
      status: 'completed',
    };
    
    const result = TodoItemSchema.safeParse(item);
    expect(result.success).toBe(true);
  });

  it('rejects invalid status', () => {
    const item = {
      content: 'Task',
      status: 'invalid_status',
    };
    
    const result = TodoItemSchema.safeParse(item);
    expect(result.success).toBe(false);
  });

  it('validates all status values', () => {
    const statuses = ['pending', 'in_progress', 'completed', 'discarded'] as const;
    
    for (const status of statuses) {
      const item = { content: 'Task', status };
      const result = TodoItemSchema.safeParse(item);
      expect(result.success).toBe(true);
    }
  });
});

describe('TodoListUpdatedEventSchema', () => {
  it('validates correct event', () => {
    const event = {
      type: 'event',
      event: 'todoListUpdated',
      data: {
        chatId: 42,
        items: [
          { content: 'Task 1', status: 'completed' },
          { content: 'Task 2', status: 'in_progress' },
        ],
      },
    };
    
    const result = TodoListUpdatedEventSchema.safeParse(event);
    expect(result.success).toBe(true);
  });

  it('rejects invalid status', () => {
    const event = {
      type: 'event',
      event: 'todoListUpdated',
      data: {
        chatId: 42,
        items: [
          { content: 'Task', status: 'invalid_status' },
        ],
      },
    };
    
    const result = TodoListUpdatedEventSchema.safeParse(event);
    expect(result.success).toBe(false);
  });

  it('validates all status values', () => {
    const statuses = ['pending', 'in_progress', 'completed', 'discarded'] as const;
    
    for (const status of statuses) {
      const event = {
        type: 'event',
        event: 'todoListUpdated',
        data: {
          chatId: 1,
          items: [{ content: 'Task', status }],
        },
      };
      
      const result = TodoListUpdatedEventSchema.safeParse(event);
      expect(result.success).toBe(true);
    }
  });

  it('validates empty items array', () => {
    const event = {
      type: 'event',
      event: 'todoListUpdated',
      data: {
        chatId: 1,
        items: [],
      },
    };
    
    const result = TodoListUpdatedEventSchema.safeParse(event);
    expect(result.success).toBe(true);
  });

  it('rejects missing chatId', () => {
    const event = {
      type: 'event',
      event: 'todoListUpdated',
      data: {
        items: [],
      },
    };
    
    const result = TodoListUpdatedEventSchema.safeParse(event);
    expect(result.success).toBe(false);
  });

  it('rejects wrong event type', () => {
    const event = {
      type: 'event',
      event: 'wrongEvent',
      data: {
        chatId: 1,
        items: [],
      },
    };
    
    const result = TodoListUpdatedEventSchema.safeParse(event);
    expect(result.success).toBe(false);
  });
});
