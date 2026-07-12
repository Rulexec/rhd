# Roles Feature Implementation Plans

This directory contains detailed implementation plans for the Roles feature, broken down into 7 sequential phases.

## Overview

The Roles feature allows projects to define multiple behavioral roles that can be switched during a chat. Each role has its own system prompt and usage description, enabling dynamic AI behavior adaptation.

## Implementation Phases

### Phase 1: Data Model & Loading
**File**: [phase-1-data-model-loading.md](./phase-1-data-model-loading.md)

**Focus**: Define role data structures and implement file loading logic

**Key Tasks**:
- Define `Role` struct with name, system_prompt, and when_to_use fields
- Extend `Project` struct to include roles
- Implement `load_roles()` function to scan roles directory
- Add unit tests for role loading

**Status**: ✅ Completed

---

### Phase 2: Backend - Role Provider & Storage
**File**: [phase-2-role-provider-storage.md](./phase-2-role-provider-storage.md)

**Focus**: Extend ProjectProvider trait and add database storage for role tracking

**Key Tasks**:
- Add `get_project_roles()` and `get_role_system_prompt()` to ProjectProvider trait
- Implement role methods in ProjectManager
- Add database columns for tracking active role and injection state
- Add role tracking methods to ChatDb

**Status**: ✅ Completed

---

### Phase 3: Backend - Role Injection Logic & Conflict Detection
**File**: [phase-3-role-injection-conflict-detection.md](./phase-3-role-injection-conflict-detection.md)

**Focus**: Implement role system prompt injection and conflict detection

**Key Tasks**:
- Implement `inject_roles_prompt()` to inject roles list system prompt
- Implement `inject_role_system_prompt()` to inject individual role prompts
- Add `check_role_conflicts()` to detect role name conflicts
- Update `attach_project()` to check for conflicts
- Integrate role injection into message sending flow

**Status**: ✅ Completed

---

### Phase 4: Backend - rhd_set_role Tool
**File**: [phase-4-rhd-set-role-tool.md](./phase-4-rhd-set-role-tool.md)

**Focus**: Implement internal tool for AI to switch roles

**Key Tasks**:
- Define `rhd_set_role` tool schema
- Implement tool handler to switch active role
- Add tool to built-in tools collection
- Update tool execution logic to handle built-in tools
- Add unit tests for tool functionality

**Status**: ✅ Completed

---

### Phase 5: WebSocket Protocol
**File**: [phase-5-websocket-protocol.md](./phase-5-websocket-protocol.md)

**Focus**: Add WebSocket events and requests for role management

**Key Tasks**:
- Add `RoleInfo` DTO
- Add `SetRole`, `GetAvailableRoles`, `ClearActiveRole` request types
- Add `RoleChanged`, `RolesUpdated`, `ActiveRoleCleared` event types
- Implement WebSocket handlers for role requests
- Forward role events to WebSocket clients

**Status**: ✅ Completed

---

### Phase 6: Frontend - Role Selector UI
**File**: [phase-6-frontend-role-selector-ui.md](./phase-6-frontend-role-selector-ui.md)

**Focus**: Add role selector dropdown to chat interface

**Key Tasks**:
- Add role-related types (RoleInfo, ActiveRole)
- Add role stores (availableRoles, activeRole, hasRoles)
- Add WebSocket functions for role operations
- Handle role events in chatWs.ts
- Create RoleSelector component
- Integrate component into ChatView
- Add unit tests for stores and actions

**Status**: ✅ Completed

---

### Phase 7: Integration & Edge Cases
**File**: [phase-7-integration-edge-cases.md](./phase-7-integration-edge-cases.md)

**Focus**: Handle edge cases and ensure smooth integration

**Key Tasks**:
- Handle project attachment/detachment with roles
- Handle active role clearing when project is detached
- Handle role switching during streaming
- Handle role name conflicts
- Add integration tests
- Update state export/import for roles

**Status**: ✅ Completed

---

## Dependency Graph

```
Phase 1 (Data Model)
    ↓
Phase 2 (Provider & Storage)
    ↓
Phase 3 (Injection Logic)
    ↓
Phase 4 (rhd_set_role Tool)
    ↓
Phase 5 (WebSocket Protocol)
    ↓
Phase 6 (Frontend UI)
    ↓
Phase 7 (Integration & Edge Cases)
```

## Key Design Decisions

1. **Role names are globally unique across attached projects**: Role names must be unique across all attached projects in a chat. If two projects define roles with the same name, attachment is rejected.

2. **Role injection is tracked per chat**: Similar to system prompts, role injection is tracked to avoid duplicates.

3. **Role switching injects new prompt**: When role changes, a new system message is injected with the new role's system prompt.

4. **rhd_set_role is internal**: This tool is not available for scenarios, only for chat AI.

5. **No automatic role selection**: When roles become available, the user or AI must explicitly choose a role.

6. **Conflict detection before attachment**: Role name conflicts are checked before project attachment, not after.

## File Structure

```
plans/roles/
├── README.md                              # This file
├── phase-1-data-model-loading.md          # Data model and loading
├── phase-2-role-provider-storage.md       # Provider trait and DB storage
├── phase-3-role-injection-conflict-detection.md  # Injection and conflicts
├── phase-4-rhd-set-role-tool.md           # Internal tool implementation
├── phase-5-websocket-protocol.md          # WebSocket protocol
├── phase-6-frontend-role-selector-ui.md   # Frontend UI
└── phase-7-integration-edge-cases.md      # Edge cases and integration
```

## Related Documentation

- Main feature plan: [plans/milestones/roles-feature-plan.md](../milestones/roles-feature-plan.md)
- Architecture: [memory/architecture.md](../../memory/architecture.md)
- Chat feature: [memory/chat.md](../../memory/chat.md)
- Projects feature: [memory/features/projects.md](../../memory/features/projects.md)
- Frontend: [memory/frontend.md](../../memory/frontend.md)
- Protocols: [memory/protocols.md](../../memory/protocols.md)

## Implementation Notes

- All phases have been planned with comprehensive test coverage
- Each phase builds on the previous one, ensuring incremental development
- Edge cases are documented and addressed in Phase 7
- The implementation follows existing patterns in the codebase
- Backward compatibility is maintained through database migrations

## Future Enhancements (Out of Scope)

- Role templates/inheritance
- Role sharing between projects
- Role versioning
- Role analytics (which roles are used most)
- Validation of active roles after `rhd reload`
