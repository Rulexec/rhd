# Phase 6: Frontend - Role Selector UI

## Goal

Add role selector dropdown to the chat interface. This phase implements the frontend UI for viewing available roles, selecting a role, and displaying the current active role.

## Current State Analysis

### Chat Stores (`frontend/src/lib/chatStores.ts`)

```typescript
export const chats: Writable<Chat[]> = writable([]);
export const currentChatId: Writable<number | null> = writable(null);
export const messages: Writable<ChatMessage[]> = writable([]);
export const isStreaming: Writable<boolean> = writable(false);
export const availableModels: Writable<string[]> = writable([]);
export const selectedModel: Writable<string | null> = writable(null);
// ... other stores

export function resetAllStores(): void {
  // Resets all stores to initial state
}
```

### Chat WebSocket Functions (`frontend/src/lib/chatWs.ts`)

```typescript
export async function loadChats(): Promise<WsResponse> { ... }
export async function loadAvailableModels(): Promise<WsResponse> { ... }
export async function sendMessage(content: string, model: string): Promise<WsResponse> { ... }

export function handleChatEvent(event: string, data: unknown): void {
  switch (event) {
    case 'chatStreamChunk': { ... }
    case 'chatMessageAdded': { ... }
    // ... other events
  }
}
```

### Actions Layer (`frontend/src/lib/actions/`)

```typescript
// actions/types.ts
export type ChatAction =
  | { type: 'sendMessage'; payload: { content: string; model: string } }
  | { type: 'selectModel'; payload: { model: string } }
  // ... other actions

// actions/dispatcher.ts
export function dispatch(action: ChatAction): void { ... }

// actions/processors.ts
export async function processAction(action: ChatAction): Promise<void> {
  switch (action.type) {
    case 'sendMessage':
      await sendMessage(action.payload.content, action.payload.model);
      break;
    // ... other cases
  }
}
```

### Chat Components

- `ChatView.svelte`: Main chat area with header, message list, and input
- `MessageInput.svelte`: Textarea with send/abort buttons, model selector dropdown
- `MessageList.svelte`: Scrollable message list

## Implementation Plan

### 6.1 Add Role Types

**File: `frontend/src/lib/types/index.ts`**

Add role-related types:

```typescript
export interface RoleInfo {
  projectName: string;
  roleName: string;
  whenToUse: string;
}

export interface ActiveRole {
  projectName: string;
  roleName: string;
}
```

### 6.2 Add Role Stores

**File: `frontend/src/lib/chatStores.ts`**

Add stores for role state:

```typescript
import type { RoleInfo, ActiveRole } from './types/index';

export const availableRoles: Writable<RoleInfo[]> = writable([]);
export const activeRole: Writable<ActiveRole | null> = writable(null);

// Derived store for checking if roles are available
export const hasRoles: Readable<boolean> = derived(
  availableRoles,
  ($availableRoles) => $availableRoles.length > 0
);

// Update resetAllStores()
export function resetAllStores(): void {
  // ... existing resets ...
  availableRoles.set([]);
  activeRole.set(null);
}
```

### 6.3 Add WebSocket Functions for Roles

**File: `frontend/src/lib/chatWs.ts`**

Add functions to load and set roles:

```typescript
import { availableRoles, activeRole } from './chatStores';

export async function loadAvailableRoles(chatId: number): Promise<WsResponse> {
  const id = generateRequestId();
  const response = await sendRequest({ type: 'getAvailableRoles', id, chatId });
  if (response.success) {
    availableRoles.set(response.data.roles || []);
    if (response.data.activeRoleProject && response.data.activeRoleName) {
      activeRole.set({
        projectName: response.data.activeRoleProject,
        roleName: response.data.activeRoleName,
      });
    } else {
      activeRole.set(null);
    }
  }
  return response;
}

export async function setRole(
  chatId: number,
  projectName: string,
  roleName: string
): Promise<WsResponse> {
  const id = generateRequestId();
  const response = await sendRequest({
    type: 'setRole',
    id,
    chatId,
    projectName,
    roleName,
  });
  if (response.success) {
    activeRole.set({ projectName, roleName });
  }
  return response;
}

export async function clearActiveRole(chatId: number): Promise<WsResponse> {
  const id = generateRequestId();
  const response = await sendRequest({ type: 'clearActiveRole', id, chatId });
  if (response.success) {
    activeRole.set(null);
  }
  return response;
}
```

### 6.4 Handle Role Events

**File: `frontend/src/lib/chatWs.ts`**

Update `handleChatEvent()` to handle role events:

```typescript
export function handleChatEvent(event: string, data: unknown): void {
  switch (event) {
    // ... existing cases ...
    
    case 'roleChanged': {
      const { projectName, roleName } = data as {
        chatId: number;
        projectName: string;
        roleName: string;
      };
      activeRole.set({ projectName, roleName });
      break;
    }
    
    case 'rolesUpdated': {
      const { roles, activeRoleProject, activeRoleName } = data as {
        chatId: number;
        roles: RoleInfo[];
        activeRoleProject?: string;
        activeRoleName?: string;
      };
      availableRoles.set(roles);
      if (activeRoleProject && activeRoleName) {
        activeRole.set({
          projectName: activeRoleProject,
          roleName: activeRoleName,
        });
      } else {
        activeRole.set(null);
      }
      break;
    }
    
    case 'activeRoleCleared': {
      activeRole.set(null);
      break;
    }
  }
}
```

### 6.5 Add Role Actions

**File: `frontend/src/lib/actions/types.ts`**

Add role-related action types:

```typescript
export type ChatAction =
  // ... existing actions ...
  | { type: 'loadAvailableRoles'; payload: { chatId: number } }
  | { type: 'setRole'; payload: { chatId: number; projectName: string; roleName: string } }
  | { type: 'clearActiveRole'; payload: { chatId: number } }
  | { type: 'roleChanged'; payload: { chatId: number; projectName: string; roleName: string } }
  | { type: 'rolesUpdated'; payload: { chatId: number; roles: RoleInfo[]; activeRoleProject?: string; activeRoleName?: string } }
  | { type: 'activeRoleCleared'; payload: { chatId: number } };
```

**File: `frontend/src/lib/actions/processors.ts`**

Add action processors:

```typescript
import { loadAvailableRoles, setRole, clearActiveRole } from '../chatWs';
import { availableRoles, activeRole } from '../chatStores';

export async function processAction(action: ChatAction): Promise<void> {
  switch (action.type) {
    // ... existing cases ...
    
    case 'loadAvailableRoles':
      await loadAvailableRoles(action.payload.chatId);
      break;
    
    case 'setRole':
      await setRole(
        action.payload.chatId,
        action.payload.projectName,
        action.payload.roleName
      );
      break;
    
    case 'clearActiveRole':
      await clearActiveRole(action.payload.chatId);
      break;
    
    case 'roleChanged':
      activeRole.set({
        projectName: action.payload.projectName,
        roleName: action.payload.roleName,
      });
      break;
    
    case 'rolesUpdated':
      availableRoles.set(action.payload.roles);
      if (action.payload.activeRoleProject && action.payload.activeRoleName) {
        activeRole.set({
          projectName: action.payload.activeRoleProject,
          roleName: action.payload.activeRoleName,
        });
      } else {
        activeRole.set(null);
      }
      break;
    
    case 'activeRoleCleared':
      activeRole.set(null);
      break;
  }
}
```

### 6.6 Load Roles on Chat Selection

**File: `frontend/src/lib/chatWs.ts`**

Update `selectChat()` to load available roles:

```typescript
export async function selectChat(chatId: number): Promise<WsResponse> {
  const id = generateRequestId();
  const response = await sendRequest({ type: 'getChat', id, chatId });
  if (response.success) {
    currentChatId.set(chatId);
    messages.set(response.data.messages || []);
    streamingContent.set('');
    streamingThinkingContent.set('');
    isStreaming.set(false);
    streamError.set(null);
    streamingMessageId.set(null);
    selectedModel.set(response.data.chat.activeModel || null);
    chatProjects.set([]);
    mcpStatuses.set([]);
    availableRoles.set([]);
    activeRole.set(null);
    loadChatProjects(chatId);
    loadAvailableRoles(chatId); // NEW: Load roles
  }
  return response;
}
```

### 6.7 Create RoleSelector Component

**File: `frontend/src/lib/components/RoleSelector.svelte`**

Create the role selector dropdown component:

```svelte
<script lang="ts">
  import { availableRoles, activeRole, hasRoles } from '../chatStores';
  import { dispatch } from '../actions';
  import { currentChatId } from '../chatStores';

  $: roles = $availableRoles;
  $: currentRole = $activeRole;
  $: chatId = $currentChatId;

  function handleRoleChange(event: Event) {
    const select = event.target as HTMLSelectElement;
    const value = select.value;
    
    if (value === '') {
      if (chatId) {
        dispatch({ type: 'clearActiveRole', payload: { chatId } });
      }
    } else {
      const [projectName, roleName] = value.split(':');
      if (chatId) {
        dispatch({
          type: 'setRole',
          payload: { chatId, projectName, roleName },
        });
      }
    }
  }

  $: selectedValue = currentRole
    ? `${currentRole.projectName}:${currentRole.roleName}`
    : '';
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

<style>
  .role-selector {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.5rem;
    border-bottom: 1px solid var(--border-color, #e0e0e0);
  }

  .role-selector label {
    font-size: 0.875rem;
    font-weight: 500;
    color: var(--text-secondary, #666);
  }

  .role-selector select {
    padding: 0.25rem 0.5rem;
    border: 1px solid var(--border-color, #e0e0e0);
    border-radius: 4px;
    background: var(--bg-primary, #fff);
    color: var(--text-primary, #333);
    font-size: 0.875rem;
    cursor: pointer;
  }

  .role-selector select:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
```

### 6.8 Integrate RoleSelector into ChatView

**File: `frontend/src/routes/ChatView.svelte`** (or similar)

Add the RoleSelector component to the chat view:

```svelte
<script lang="ts">
  import RoleSelector from '../lib/components/RoleSelector.svelte';
  // ... other imports
</script>

<div class="chat-view">
  <div class="chat-header">
    <h2>{currentChat?.title || 'Chat'}</h2>
  </div>
  
  <RoleSelector />
  
  <MessageList />
  
  <MessageInput />
</div>
```

Alternatively, integrate into `MessageInput.svelte` if the role selector should be near the message input:

```svelte
<script lang="ts">
  import RoleSelector from './RoleSelector.svelte';
  // ... other imports
</script>

<div class="message-input-container">
  <RoleSelector />
  
  <div class="input-area">
    <textarea bind:value={content} on:keydown={handleKeyDown} />
    <button on:click={handleSend} disabled={!content.trim() || $isStreaming}>
      Send
    </button>
  </div>
</div>
```

### 6.9 Add Unit Tests

**File: `frontend/src/tests/roleStores.test.ts`**

```typescript
import { describe, it, expect, beforeEach } from 'vitest';
import {
  availableRoles,
  activeRole,
  hasRoles,
  resetAllStores,
} from '../lib/chatStores';
import { get } from 'svelte/store';

describe('Role Stores', () => {
  beforeEach(() => {
    resetAllStores();
  });

  it('initializes with empty roles', () => {
    expect(get(availableRoles)).toEqual([]);
    expect(get(activeRole)).toBeNull();
    expect(get(hasRoles)).toBe(false);
  });

  it('updates available roles', () => {
    const roles = [
      {
        projectName: 'project-a',
        roleName: 'developer',
        whenToUse: 'Use for coding',
      },
    ];
    availableRoles.set(roles);
    expect(get(availableRoles)).toEqual(roles);
    expect(get(hasRoles)).toBe(true);
  });

  it('updates active role', () => {
    const role = { projectName: 'project-a', roleName: 'developer' };
    activeRole.set(role);
    expect(get(activeRole)).toEqual(role);
  });

  it('resetAllStores clears role state', () => {
    availableRoles.set([
      {
        projectName: 'project-a',
        roleName: 'developer',
        whenToUse: 'Use for coding',
      },
    ]);
    activeRole.set({ projectName: 'project-a', roleName: 'developer' });

    resetAllStores();

    expect(get(availableRoles)).toEqual([]);
    expect(get(activeRole)).toBeNull();
    expect(get(hasRoles)).toBe(false);
  });
});
```

**File: `frontend/src/tests/roleActions.test.ts`**

```typescript
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { _testOverrideAction, _testClearOverrides } from '../lib/actions';
import { availableRoles, activeRole, resetAllStores } from '../lib/chatStores';
import { get } from 'svelte/store';

describe('Role Actions', () => {
  beforeEach(() => {
    _testClearOverrides();
    resetAllStores();
  });

  it('handles roleChanged action', async () => {
    _testOverrideAction('roleChanged', async (action) => {
      activeRole.set({
        projectName: action.payload.projectName,
        roleName: action.payload.roleName,
      });
    });

    const { dispatch } = await import('../lib/actions');
    await dispatch({
      type: 'roleChanged',
      payload: {
        chatId: 1,
        projectName: 'project-a',
        roleName: 'developer',
      },
    });

    expect(get(activeRole)).toEqual({
      projectName: 'project-a',
      roleName: 'developer',
    });
  });

  it('handles rolesUpdated action', async () => {
    _testOverrideAction('rolesUpdated', async (action) => {
      availableRoles.set(action.payload.roles);
      if (action.payload.activeRoleProject && action.payload.activeRoleName) {
        activeRole.set({
          projectName: action.payload.activeRoleProject,
          roleName: action.payload.activeRoleName,
        });
      }
    });

    const { dispatch } = await import('../lib/actions');
    await dispatch({
      type: 'rolesUpdated',
      payload: {
        chatId: 1,
        roles: [
          {
            projectName: 'project-a',
            roleName: 'developer',
            whenToUse: 'Use for coding',
          },
        ],
        activeRoleProject: 'project-a',
        activeRoleName: 'developer',
      },
    });

    expect(get(availableRoles)).toHaveLength(1);
    expect(get(activeRole)).toEqual({
      projectName: 'project-a',
      roleName: 'developer',
    });
  });
});
```

## Files to Modify

1. **`frontend/src/lib/types/index.ts`**
   - Add `RoleInfo` interface
   - Add `ActiveRole` interface

2. **`frontend/src/lib/chatStores.ts`**
   - Add `availableRoles` store
   - Add `activeRole` store
   - Add `hasRoles` derived store
   - Update `resetAllStores()` to clear role state

3. **`frontend/src/lib/chatWs.ts`**
   - Add `loadAvailableRoles()` function
   - Add `setRole()` function
   - Add `clearActiveRole()` function
   - Update `handleChatEvent()` to handle role events
   - Update `selectChat()` to load roles

4. **`frontend/src/lib/actions/types.ts`**
   - Add role-related action types

5. **`frontend/src/lib/actions/processors.ts`**
   - Add processors for role actions

6. **`frontend/src/lib/components/RoleSelector.svelte`** (NEW)
   - Create role selector dropdown component

7. **`frontend/src/routes/ChatView.svelte`** (or `MessageInput.svelte`)
   - Integrate `RoleSelector` component

8. **`frontend/src/tests/roleStores.test.ts`** (NEW)
   - Add unit tests for role stores

9. **`frontend/src/tests/roleActions.test.ts`** (NEW)
   - Add unit tests for role actions

## Dependencies

- Phase 5: WebSocket protocol for role management

## Success Criteria

1. ✅ `RoleInfo` and `ActiveRole` types defined
2. ✅ `availableRoles` and `activeRole` stores added
3. ✅ `hasRoles` derived store works correctly
4. ✅ `loadAvailableRoles()` fetches roles from backend
5. ✅ `setRole()` sends request and updates store
6. ✅ `clearActiveRole()` sends request and clears store
7. ✅ `roleChanged` event updates active role
8. ✅ `rolesUpdated` event updates available roles and active role
9. ✅ `activeRoleCleared` event clears active role
10. ✅ Roles loaded when chat is selected
11. ✅ `RoleSelector` component displays when roles available
12. ✅ Role selector allows changing active role
13. ✅ Role selector disabled during streaming
14. ✅ All unit tests pass
15. ✅ Existing tests still pass

## Design Decisions

1. **Role selector as separate component**: Keeps the UI modular and reusable. Can be placed in different locations (header, input area) as needed.

2. **Value format `projectName:roleName`**: Uses a composite key to uniquely identify roles across projects. This is parsed in the change handler.

3. **"No role" option**: Allows users to clear the active role if desired. This sends a `clearActiveRole` request.

4. **Disabled during streaming**: Prevents role changes while the AI is responding, which could cause confusion.

5. **Roles loaded on chat selection**: Ensures the role state is synchronized when switching between chats.

## UI Behavior

- **Visibility**: Role selector is only visible when `hasRoles` is true (at least one role available)
- **Default selection**: First role is NOT pre-selected automatically. User must explicitly select a role or the AI can use `rhd_set_role` tool.
- **Role change**: When user selects a role, `setRole` request is sent, backend injects role's system prompt, and `roleChanged` event is received
- **Streaming**: Role selector is disabled while `isStreaming` is true
- **Project attach/detach**: When roles change, `rolesUpdated` event updates the available roles list

## Next Steps

After this phase:
- Phase 7 will handle edge cases and integration testing
