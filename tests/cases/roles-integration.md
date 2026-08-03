# Test Case: Roles Integration

## Description
Tests the integration of the roles feature across all components, including edge cases for project attachment/detachment, role switching, and state management.

## Preconditions
- WebSocket connected to daemon
- Test project with roles defined:
  - `roles/developer/systemPrompt.md`
  - `roles/developer/whenToUse.md`
  - `roles/reviewer/systemPrompt.md`
  - `roles/reviewer/whenToUse.md`

## Test Scenarios

### Scenario 1: Basic Role Selection
1. Create a chat
2. Attach project with roles
3. Verify `RolesUpdated` event is emitted
4. Verify role selector appears in UI
5. Select "developer" role via UI
6. Send a message
7. Verify role's system prompt is injected
8. Verify AI responds according to the role

### Scenario 2: Role Switching via Tool
1. Create a chat and attach project with multiple roles
2. Send a message asking AI to switch roles
3. Verify AI calls `rhd_set_role` tool
4. Verify `RoleChanged` event is emitted
5. Verify role changes and system prompt is injected
6. Verify AI responds according to new role

### Scenario 3: Project Detachment with Active Role
1. Attach a project with roles
2. Select a role (e.g., "developer")
3. Detach the project
4. Verify `ActiveRoleCleared` event is emitted
5. Verify active role is cleared in database
6. Verify role selector is hidden (if no other roles)
7. Verify `RolesUpdated` event is emitted

### Scenario 4: Role Name Conflict
1. Create two projects with overlapping role names (both have "developer")
2. Attach first project
3. Try to attach second project
4. Verify attachment is rejected with `RoleNameConflict` error
5. Verify error message lists conflicting role names
6. Verify second project is not attached

### Scenario 5: Project Attached After Chat Started
1. Create a chat (no projects attached)
2. Send a message (no roles available)
3. Attach project with roles
4. Verify `RolesUpdated` event is emitted
5. Verify role selector appears
6. Verify `roles_list_injected` flag is reset
7. Send another message
8. Verify roles list prompt is injected

### Scenario 6: No Roles Available
1. Create a chat
2. Attach project without roles
3. Verify no `RolesUpdated` event
4. Verify role selector is hidden
5. Verify `rhd_set_role` tool is not available
6. Verify roles list prompt is not injected

### Scenario 7: Multiple Projects with Roles
1. Attach first project with roles (developer, reviewer)
2. Attach second project with different roles (architect, tester)
3. Verify all roles are available
4. Verify role selector shows all 4 roles
5. Select a role from second project
6. Verify role switching works correctly

### Scenario 8: State Export/Import with Roles
1. Attach project with roles
2. Select a role
3. Export state
4. Verify exported state includes `availableRoles` and `activeRole`
5. Import state
6. Verify stores are restored correctly
7. Verify UI reflects restored state

## Expected Results
- All scenarios complete successfully
- Events are emitted correctly
- Database state is consistent
- UI reflects backend state
- Edge cases are handled gracefully

## Actions
- `attachProject` — attach project to chat
- `detachProject` — detach project from chat
- `setRole` — set active role
- `getAvailableRoles` — get list of available roles
- `clearActiveRole` — clear active role

## Covered By

### E2E Tests
- [`roles-integration.test.ts`](../../frontend/src/tests/e2e/roles-integration.test.ts) - `Scenario 1: Basic Role Selection` (steps 1-8)
- [`roles-integration.test.ts`](../../frontend/src/tests/e2e/roles-integration.test.ts) - `Scenario 2: Role Switching via Tool` (steps 1-6)
- [`roles-integration.test.ts`](../../frontend/src/tests/e2e/roles-integration.test.ts) - `Scenario 3: Project Detachment with Active Role` (steps 1-7)
- [`roles-integration.test.ts`](../../frontend/src/tests/e2e/roles-integration.test.ts) - `Scenario 4: Role Name Conflict` (steps 1-6)
- [`roles-integration.test.ts`](../../frontend/src/tests/e2e/roles-integration.test.ts) - `Scenario 5: Project Attached After Chat Started` (steps 1-8)
- [`roles-integration.test.ts`](../../frontend/src/tests/e2e/roles-integration.test.ts) - `Scenario 6: No Roles Available` (steps 1-6)
- [`roles-integration.test.ts`](../../frontend/src/tests/e2e/roles-integration.test.ts) - `Scenario 7: Multiple Projects with Roles` (steps 1-6)
- [`roles-integration.test.ts`](../../frontend/src/tests/e2e/roles-integration.test.ts) - `Scenario 8: State Export/Import with Roles` (steps 1-7)

### Coverage Notes
- All 8 scenarios are covered by E2E tests
- Full coverage: 100%
