# Phase 5: Frontend - Todo List Stores & WebSocket Handling

## Overview

This phase adds frontend stores and WebSocket handling for the todo list feature. The stores manage the todo list state, and the WebSocket handler updates the stores when the backend emits `todoListUpdated` events.

## Files to Modify

### 1. `frontend/src/lib/types/ws.ts`

**Add TodoListUpdatedEventSchema:**

```typescript
export const TodoItemSchema = z.object({
  content: z.string(),
  status: z.enum(['pending', 'in_progress', 'completed', 'discarded']),
});

export const TodoListUpdatedEventSchema = z.object({
  type: z.literal('event'),
  event: z.literal('todoListUpdated'),
  data: z.object({
    chatId: z.number(),
    items: z.array(TodoItemSchema),
  }),
});
```

**Add to WsEventSchema union:**

```typescript
export const WsEventSchema = z.discriminatedUnion('event', [
  // ... existing events ...
  TodoListUpdatedEventSchema,
]);
```

### 2. `frontend/src/lib/types/index.ts`

**Add TodoItem type:**

```typescript
export interface TodoItem {
  content: string;
  status: 'pending' | 'in_progress' | 'completed' | 'discarded';
}
```

### 3. `frontend/src/lib/chatStores.ts`

**Add todo list stores:**

```typescript
import type { TodoItem } from './types/index';

// Add after existing stores
export const todoList: Writable<TodoItem[]> = writable([]);

// Derived store: has todo list
export const hasTodoList: Readable<boolean> = derived(
  todoList,
  ($todoList) => $todoList.length > 0
);

// Derived store: todo list statistics
export const todoListStats: Readable<{
  total: number;
  completed: number;
  pending: number;
  inProgress: number;
  discarded: number;
}> = derived(todoList, ($todoList) => {
  const total = $todoList.length;
  const completed = $todoList.filter(item => item.status === 'completed').length;
  const pending = $todoList.filter(item => item.status === 'pending').length;
  const inProgress = $todoList.filter(item => item.status === 'in_progress').length;
  const discarded = $todoList.filter(item => item.status === 'discarded').length;
  
  return { total, completed, pending, inProgress, discarded };
});
```

**Update resetAllStores():**

```typescript
export function resetAllStores(): void {
  // ... existing resets ...
  todoList.set([]);
}
```

### 4. `frontend/src/lib/chatWs.ts`

**Import todoList store:**

```typescript
import {
  // ... existing imports ...
  todoList,
} from './chatStores';
```

**Add event handler in handleChatEvent():**

```typescript
export function handleChatEvent(event: string, data: unknown): void {
  switch (event) {
    // ... existing cases ...
    
    case 'todoListUpdated': {
      const { items } = data as {
        chatId: number;
        items: import('./types/index').TodoItem[];
      };
      todoList.set(items);
      break;
    }
  }
}
```

**Update selectChat() to clear todo list:**

```typescript
export async function selectChat(chatId: number): Promise<WsResponse> {
  // ... existing code ...
  
  if (response.success) {
    // ... existing store resets ...
    todoList.set([]);  // Add this line
    
    // ... rest of the code ...
  }
  return response;
}
```

**Update deleteAllChats() to clear todo list:**

```typescript
export async function deleteAllChats(): Promise<WsResponse> {
  // ... existing code ...
  
  if (response.success) {
    // ... existing store resets ...
    todoList.set([]);  // Add this line
    
    // ... rest of the code ...
  }
  return response;
}
```

### 5. `frontend/src/lib/actions/types.ts`

**Add todoListUpdated action type:**

```typescript
export type ChatAction =
  // ... existing action types ...
  | { type: 'todoListUpdated'; payload: { chatId: number; items: import('../types/index').TodoItem[] } };
```

### 6. `frontend/src/lib/actions/processors.ts`

**Add processor for todoListUpdated:**

```typescript
export async function processAction(action: ChatAction): Promise<void> {
  switch (action.type) {
    // ... existing cases ...
    
    case 'todoListUpdated':
      handleChatEvent('todoListUpdated', action.payload);
      break;
  }
}
```

## Store Behavior

### todoList Store

- **Type**: `Writable<TodoItem[]>`
- **Initial value**: `[]` (empty array)
- **Updated by**: `todoListUpdated` WebSocket event
- **Cleared by**: `selectChat()`, `deleteAllChats()`, `resetAllStores()`

### hasTodoList Store

- **Type**: `Readable<boolean>`
- **Derived from**: `todoList`
- **Value**: `true` if `todoList.length > 0`, `false` otherwise
- **Used by**: UI components to conditionally show todo list button

### todoListStats Store

- **Type**: `Readable<{ total, completed, pending, inProgress, discarded }>`
- **Derived from**: `todoList`
- **Used by**: UI components to display progress indicators

## Event Flow

```
1. Backend emits ChatEvent::TodoListUpdated
2. ws.rs converts to WsEvent with "todoListUpdated" name
3. WebSocket sends event to frontend
4. Frontend ws.ts receives and validates event with Zod schema
5. handleChatEvent() is called with event name and data
6. todoList store is updated with new items
7. Derived stores (hasTodoList, todoListStats) automatically update
8. UI components re-render with new data
```

## Tests

### Store Tests

```typescript
import { get } from 'svelte/store';
import { todoList, hasTodoList, todoListStats } from './chatStores';
import type { TodoItem } from './types/index';

describe('todoList stores', () => {
  beforeEach(() => {
    todoList.set([]);
  });

  test('todoList initial value is empty array', () => {
    expect(get(todoList)).toEqual([]);
  });

  test('todoList can be updated', () => {
    const items: TodoItem[] = [
      { content: 'Task 1', status: 'completed' },
      { content: 'Task 2', status: 'pending' },
    ];
    todoList.set(items);
    expect(get(todoList)).toEqual(items);
  });

  test('hasTodoList is false when empty', () => {
    expect(get(hasTodoList)).toBe(false);
  });

  test('hasTodoList is true when items exist', () => {
    todoList.set([{ content: 'Task', status: 'pending' }]);
    expect(get(hasTodoList)).toBe(true);
  });

  test('todoListStats calculates correct counts', () => {
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

  test('todoListStats handles empty list', () => {
    const stats = get(todoListStats);
    expect(stats.total).toBe(0);
    expect(stats.completed).toBe(0);
    expect(stats.pending).toBe(0);
    expect(stats.inProgress).toBe(0);
    expect(stats.discarded).toBe(0);
  });
});
```

### WebSocket Handler Tests

```typescript
import { get } from 'svelte/store';
import { handleChatEvent } from './chatWs';
import { todoList } from './chatStores';
import type { TodoItem } from './types/index';

describe('handleChatEvent todoListUpdated', () => {
  beforeEach(() => {
    todoList.set([]);
  });

  test('updates todoList store', () => {
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

  test('replaces existing todo list', () => {
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

  test('handles empty items array', () => {
    todoList.set([{ content: 'Task', status: 'pending' }]);
    
    handleChatEvent('todoListUpdated', {
      chatId: 42,
      items: [],
    });
    
    expect(get(todoList)).toEqual([]);
  });
});
```

### Zod Schema Tests

```typescript
import { TodoListUpdatedEventSchema, TodoItemSchema } from './types/ws';

describe('TodoListUpdatedEventSchema', () => {
  test('validates correct event', () => {
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

  test('rejects invalid status', () => {
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

  test('validates all status values', () => {
    const statuses = ['pending', 'in_progress', 'completed', 'discarded'];
    
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
});
```

## Implementation Notes

1. **Store naming**: Uses `todoList` (camelCase) to match frontend conventions.

2. **Type safety**: The `TodoItem` type is defined in `types/index.ts` and imported where needed.

3. **Derived stores**: `hasTodoList` and `todoListStats` automatically update when `todoList` changes, no manual intervention needed.

4. **Event handling**: The `todoListUpdated` event is handled in the same `handleChatEvent()` function as other chat events for consistency.

5. **Store clearing**: The todo list is cleared when switching chats or deleting all chats to prevent stale data.

6. **Zod validation**: The event schema validates the structure and status values before updating the store.

## Dependencies

- This phase depends on Phase 4 (WebSocket Protocol) for the event format
- This phase must be completed before Phase 6 (Frontend UI)
- Phase 6 uses the stores and derived values defined here
