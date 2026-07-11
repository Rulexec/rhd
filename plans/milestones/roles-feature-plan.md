# Roles Feature Plan

## Overview

This plan introduces a "roles" feature that allows projects to define multiple behavioral roles. Users can switch between roles during a chat, and the AI's behavior is defined by the active role's system prompt.

## Feature Requirements

### Role Definition
- Roles are defined in project directories under `roles/<roleName>/`
- Each role directory contains:
  - `systemPrompt.md` — the role's system prompt
  - `whenToUse.md` — description for when to use this role

### Role Discovery
- When a project is attached, scan for `roles/` subdirectory
- Load all role definitions from the project
- Aggregate roles from all attached projects

### Role Name Conflict Detection
- **Before** attaching a project, check for role name conflicts with already-attached projects
- If duplicate role names found across projects, reject attachment with error listing conflicting role names
- User must detach one of the conflicting projects or rename roles
- This mirrors the existing MCP ID conflict detection behavior

### Role Injection
- When roles are available, inject a system prompt listing all available roles with their `whenToUse.md` content
- If new project with roles is attached mid-conversation, inject updated roles list before next user message
- When role is selected/changed, inject the role's `systemPrompt.md`

### User Interface
- Role selector dropdown near message input (visible when roles are available)
- First role is pre-selected as current role
- Role selection triggers role system prompt injection

### Internal Tool
- `rhd_set_role` tool for AI to switch roles
- Not available for scenarios (internal use only)

---

## Implementation Phases

### Phase 1: Data Model & Loading

**Goal**: Extend project data model to include roles and load them from disk.

**Files to modify**:
- [`packages/rhd_api/src/project.rs`](packages/rhd_api/src/project.rs)
  - Add `Role` struct with `name`, `system_prompt`, `when_to_use` fields
  - Add `roles: Vec<Role>` field to `Project` struct
  - Update `ProjectInfo` to include `has_roles: bool`

- [`packages/rhd_app/src/project_loader.rs`](packages/rhd_app/src/project_loader.rs)
  - Add `load_roles()` function to scan `roles/` subdirectory
  - Load `systemPrompt.md` and `whenToUse.md` for each role
  - Integrate role loading into `load_project()`

**Tests**:
- Unit tests for role loading
- Test project with roles directory
- Test project without roles directory

---

### Phase 2: Backend - Role Provider & Storage

**Goal**: Extend `ProjectProvider` trait and add role storage to chat state.

**Files to modify**:
- [`packages/rhd_chat/src/lib.rs`](packages/rhd_chat/src/lib.rs)
  - Extend `ProjectProvider` trait with role-related methods:
    - `get_project_roles(&self, project_name: &str) -> Vec<Role>`
    - `get_role_system_prompt(&self, project_name: &str, role_name: &str) -> Option<String>`

- [`packages/rhd_app/src/project_manager.rs`](packages/rhd_app/src/project_manager.rs)
  - Implement new trait methods
  - Add role caching if needed

- [`packages/rhd_db/src/chat_db.rs`](packages/rhd_db/src/chat_db.rs)
  - Add table/column for tracking current active role per chat
  - Add table for tracking which roles have been injected per chat
  - Add methods: `set_active_role()`, `get_active_role()`, `mark_role_injected()`, `has_role_been_injected()`

**Tests**:
- Database migration tests
- Role tracking tests

---

### Phase 3: Backend - Role Injection Logic & Conflict Detection

**Goal**: Implement role system prompt injection and conflict detection.

**Files to modify**:
- [`packages/rhd_chat/src/projects.rs`](packages/rhd_chat/src/projects.rs)
  - Add `inject_roles_prompt()` function
  - Build roles list system prompt from all attached projects
  - Inject before first user message if roles available
  - Add `check_role_conflicts()` function to detect role name conflicts before attachment
  - Return error with list of conflicting role names if conflicts found

- [`packages/rhd_chat/src/stream.rs`](packages/rhd_chat/src/stream.rs)
  - Call `inject_roles_prompt()` in `send_message()` before user message
  - Handle role switching: inject new role's system prompt when role changes

- [`packages/rhd_chat/src/manager.rs`](packages/rhd_chat/src/manager.rs)
  - Add `set_active_role()` method
  - Add `get_available_roles()` method
  - Handle role change events
  - Update `attach_project()` to check for role conflicts

**System Prompt Format**:
```
Your behavior is defined by the current active role. Current active role is <currentRoleName>. You can switch your role by tool `rhd_set_role`.

These are the currently available roles:

# <roleNameA>

<whenToUse.md content>

# <roleNameB>

<whenToUse.md content>
```

**Role Switch Prompt**:
```
Your current role is now <currentRoleName>.

-----

<systemPrompt.md content>
```

**Tests**:
- Role injection tests
- Role switching tests
- Multiple projects with roles tests
- Role name conflict detection tests

---

### Phase 4: Backend - rhd_set_role Tool

**Goal**: Implement internal tool for AI to switch roles.

**Files to modify**:
- [`packages/rhd_chat/src/tools.rs`](packages/rhd_chat/src/tools.rs)
  - Add `rhd_set_role` tool definition
  - Add tool handler that switches active role
  - Inject new role's system prompt after tool call

- [`packages/rhd_api/src/lib.rs`](packages/rhd_api/src/lib.rs)
  - Add tool-related types if needed

**Tool Definition**:
```json
{
  "name": "rhd_set_role",
  "description": "Switch the current active role",
  "parameters": {
    "type": "object",
    "properties": {
      "role_name": {
        "type": "string",
        "description": "The name of the role to switch to"
      }
    },
    "required": ["role_name"]
  }
}
```

**Tests**:
- Tool call tests
- Role switching via tool tests

---

### Phase 5: WebSocket Protocol

**Goal**: Add WebSocket events and requests for role management.

**Files to modify**:
- [`packages/rhd_api/src/lib.rs`](packages/rhd_api/src/lib.rs)
  - Add `RoleInfo` DTO
  - Add `WsRequest::SetRole` variant
  - Add `WsRequest::GetAvailableRoles` variant
  - Add `WsEvent::RoleChanged` variant
  - Add `WsEvent::RolesUpdated` variant

- [`packages/rhd_app/src/ws.rs`](packages/rhd_app/src/ws.rs)
  - Handle `SetRole` request
  - Handle `GetAvailableRoles` request
  - Send `RoleChanged` event when role changes
  - Send `RolesUpdated` event when new roles become available

**Tests**:
- WebSocket protocol tests
- Event handling tests

---

### Phase 6: Frontend - Role Selector UI

**Goal**: Add role selector dropdown to chat interface.

**Files to modify**:
- [`frontend/src/lib/chatStores.ts`](frontend/src/lib/chatStores.ts)
  - Add store for available roles
  - Add store for current active role
  - Add actions for role selection

- [`frontend/src/lib/chatWs.ts`](frontend/src/lib/chatWs.ts)
  - Handle `RoleChanged` event
  - Handle `RolesUpdated` event
  - Send `SetRole` request

- [`frontend/src/routes/Chat.svelte`](frontend/src/routes/Chat.svelte) (or similar)
  - Add role selector dropdown component
  - Show dropdown when roles are available
  - Pre-select first role
  - Display current role in chat header

**UI Behavior**:
- Dropdown visible only when roles are available
- First role pre-selected
- Role change triggers backend update
- Visual indicator of current role

**Tests**:
- Component tests
- Store tests
- E2E tests for role selection

---

### Phase 7: Integration & Edge Cases

**Goal**: Handle edge cases and ensure smooth integration.

**Scenarios to handle**:
1. Project with roles attached after chat started
2. Role removed when project detached
3. Current role's project detached (reset to first available role or no role)
4. No roles available (hide selector)
5. Role switching during streaming

**Files to modify**:
- [`packages/rhd_chat/src/projects.rs`](packages/rhd_chat/src/projects.rs)
  - Handle role cleanup on project detach
  - Reset active role if current role's project is detached

- [`frontend/src/lib/chatStores.ts`](frontend/src/lib/chatStores.ts)
  - Handle role availability changes
  - Update UI when roles change

**Tests**:
- Edge case tests
- Integration tests

---

## Dependency Graph

```
Phase 1 (Data Model)
    ↓
Phase 2 (Provider & Storage)
    ↓
Phase 3 (Injection Logic & Conflict Detection)
    ↓
Phase 4 (rhd_set_role Tool)
    ↓
Phase 5 (WebSocket Protocol)
    ↓
Phase 6 (Frontend UI)
    ↓
Phase 7 (Integration & Edge Cases)
```

---

## Key Design Decisions

1. **Role names are globally unique across attached projects**: Role names must be unique across all attached projects in a chat. If two projects define roles with the same name, attachment is rejected (similar to MCP ID conflicts).

2. **Role injection is tracked per chat**: Similar to system prompts, role injection is tracked to avoid duplicates.

3. **Role switching injects new prompt**: When role changes, a new system message is injected with the new role's system prompt.

4. **rhd_set_role is internal**: This tool is not available for scenarios, only for chat AI.

5. **First role is default**: When roles become available, the first role (alphabetically or by discovery order) is pre-selected.

6. **Conflict detection before attachment**: Role name conflicts are checked before project attachment, not after. This prevents inconsistent state.

---

## Future Enhancements (Out of Scope)

- Role templates/inheritance
- Role sharing between projects
- Role versioning
- Role analytics (which roles are used most)
