# Phase 5: Frontend - Project UI

## Goal
Display projects, allow attachment, show MCP status.

## Current State Analysis
- Frontend types in [`frontend/src/lib/types/index.ts`](frontend/src/lib/types/index.ts) use Zod schemas
- Chat stores in [`frontend/src/lib/chatStores.ts`](frontend/src/lib/chatStores.ts) use Svelte writable stores
- WebSocket handlers in [`frontend/src/lib/chatWs.ts`](frontend/src/lib/chatWs.ts) use `sendRequest()` pattern
- Components in [`frontend/src/components/`](frontend/src/components/) - Svelte components
- [`ChatView.svelte`](frontend/src/components/ChatView.svelte) - main chat UI
- [`ChatsTab.svelte`](frontend/src/components/ChatsTab.svelte) - sidebar with chat list

## Subtasks

### 5.1. Add project types to frontend
**File**: [`frontend/src/lib/types/index.ts`](frontend/src/lib/types/index.ts)

**New schemas**:
```typescript
export const ProjectSchema = z.object({
  name: z.string(),
  hasMcp: z.boolean(),
  hasSystemPrompt: z.boolean(),
});
export type Project = z.infer<typeof ProjectSchema>;

export const McpStatusSchema = z.object({
  projectName: z.string(),
  mcpName: z.string(),
  status: z.enum(['connecting', 'connected', 'failed']),
  error: z.string().optional(),
});
export type McpStatus = z.infer<typeof McpStatusSchema>;

export const ChatProjectSchema = z.object({
  name: z.string(),
  systemPromptAdded: z.boolean(),
});
export type ChatProject = z.infer<typeof ChatProjectSchema>;
```

**File**: [`frontend/src/lib/types/ws.ts`](frontend/src/lib/types/ws.ts)

**Add event schemas**:
```typescript
export const ProjectMcpStatusChangedSchema = z.object({
  projectName: z.string(),
  mcpName: z.string(),
  status: z.enum(['connecting', 'connected', 'failed']),
  error: z.string().optional(),
});

export const ProjectAttachedSchema = z.object({
  chatId: z.number(),
  projectName: z.string(),
});

export const ProjectDetachedSchema = z.object({
  chatId: z.number(),
  projectName: z.string(),
});
```

### 5.2. Add project stores
**File**: `frontend/src/lib/projectStores.ts` (new)

**Stores**:
```typescript
import { writable } from 'svelte/store';
import type { Project, McpStatus, ChatProject } from './types';

export const projects = writable<Project[]>([]);
export const mcpStatuses = writable<McpStatus[]>([]);
export const chatProjects = writable<ChatProject[]>([]);
```

**Functions**:
```typescript
export async function loadProjects(): Promise<void>
export async function loadMcpStatus(projectName: string): Promise<void>
export async function loadChatProjects(chatId: number): Promise<void>
export function handleMcpStatusEvent(event: any): void
export function handleProjectAttachedEvent(event: any): void
export function handleProjectDetachedEvent(event: any): void
```

### 5.3. Create ProjectsPanel component
**File**: `frontend/src/components/ProjectsPanel.svelte` (new)

**UI**:
- Show list of available projects
- Each project shows:
  - Name
  - Attach/detach button (context-aware based on current chat)
  - MCP status indicator (if attached)

**Integration**: Add to [`ChatsTab.svelte`](frontend/src/components/ChatsTab.svelte) sidebar or separate tab

**Logic**:
- Load projects on mount
- Check if project attached to current chat
- Show attach button if not attached, detach if attached
- Disable attach if any MCP server not connected

### 5.4. Create McpStatusDrawer component
**File**: `frontend/src/components/McpStatusDrawer.svelte` (new)

**UI**:
- Right-side drawer (toggleable)
- Show attached projects list
- For each project:
  - Project name
  - List of MCP servers with status dots (green/red/yellow)
  - Error message if failed

**Toggle button**: Add to header or chat view

**Logic**:
- Subscribe to `mcpStatuses` store
- Filter by attached projects
- Show real-time status updates

### 5.5. Add project attachment to ChatView
**File**: [`frontend/src/components/ChatView.svelte`](frontend/src/components/ChatView.svelte)

**Changes**:
- Show attached projects in header (badges or chips)
- Button to open project selector (ProjectsPanel)
- Disable send button if any MCP server not connected
- Show MCP status drawer toggle

**Logic**:
- Load chat projects when chat selected
- Update UI on project attach/detach events
- Check MCP status before enabling send

### 5.6. Add WebSocket handlers for projects
**File**: [`frontend/src/lib/chatWs.ts`](frontend/src/lib/chatWs.ts)

**New functions**:
```typescript
export async function attachProject(chatId: number, projectName: string): Promise<WsResponse>
export async function detachProject(chatId: number, projectName: string): Promise<WsResponse>
export async function loadChatProjects(chatId: number): Promise<WsResponse>
export async function loadProjects(): Promise<WsResponse>
export async function loadProjectMcpStatus(projectName: string): Promise<WsResponse>
```

**Event handling** in `handleChatEvent()`:
- `projectMcpStatusChanged` → update `mcpStatuses` store
- `projectAttached` → update `chatProjects` store
- `projectDetached` → update `chatProjects` store

## Deliverables
- [ ] Project types and schemas in [`frontend/src/lib/types/index.ts`](frontend/src/lib/types/index.ts)
- [ ] Project stores in `frontend/src/lib/projectStores.ts`
- [ ] ProjectsPanel component
- [ ] McpStatusDrawer component
- [ ] Project attachment UI in [`ChatView.svelte`](frontend/src/components/ChatView.svelte)
- [ ] WebSocket handlers for projects in [`frontend/src/lib/chatWs.ts`](frontend/src/lib/chatWs.ts)

## Dependencies
- Phase 1-3 (Backend) must be complete for API to work
- Can start UI development with mock data

## Risk Assessment
- **Low risk**: Follows existing frontend patterns
- **Unknown**: How to handle multiple projects with conflicting tools
- **Mitigation**: UI shows all attached projects, user responsible for conflicts
