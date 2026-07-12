# Phase 7: Integration & Edge Cases

## Goal

Handle edge cases and ensure smooth integration of the roles feature across all components. This phase addresses scenarios that arise from dynamic project attachment/detachment, role lifecycle management, and ensures the feature works correctly in all situations.

## Current State Analysis

After Phases 1-6, the roles feature is fully implemented with:
- Role data model and loading from disk
- Role provider and storage in backend
- Role injection logic and conflict detection
- `rhd_set_role` tool for AI to switch roles
- WebSocket protocol for role management
- Frontend role selector UI

However, several edge cases need to be addressed to ensure robust behavior.

## Edge Cases to Handle

### 7.1 Project with Roles Attached After Chat Started

**Scenario**: User starts a chat, then attaches a project that has roles.

**Expected Behavior**:
1. `RolesUpdated` event is emitted with the new roles list
2. Frontend updates available roles
3. Roles list prompt is injected on next message (because `roles_list_injected` was reset)
4. No role is automatically selected (user or AI must choose)

**Implementation**:
- Already handled in Phase 3 (`attach_project()` resets `roles_list_injected` and emits `RolesUpdated`)
- Frontend already handles `rolesUpdated` event (Phase 6)

**Testing**:
```rust
#[tokio::test]
async fn test_attach_project_with_roles_after_chat_started() {
    let db = Arc::new(ChatDb::new("test_attach_after_start.db").unwrap());
    let chat_id = db.create_chat("Test").unwrap();
    
    // Send a message (no roles yet)
    // ... setup and send message ...
    
    // Attach project with roles
    let mut provider = MockProjectProvider { /* ... */ };
    provider.roles.insert("project-a".to_string(), vec![
        Role { name: "developer".to_string(), /* ... */ },
    ]);
    
    let (event_sender, mut event_rx) = broadcast::channel(100);
    attach_project(&db, &Arc::new(provider), chat_id, "project-a", event_sender.clone()).await.unwrap();
    
    // Verify RolesUpdated event was emitted
    let event = event_rx.recv().await.unwrap();
    assert!(matches!(event, ChatEvent::RolesUpdated { .. }));
    
    // Verify roles_list_injected was reset
    assert!(!db.has_roles_list_been_injected(chat_id).unwrap());
    
    cleanup("test_attach_after_start.db");
}
```

### 7.2 Role Removed When Project Detached

**Scenario**: User detaches a project that has roles.

**Expected Behavior**:
1. Roles from that project are no longer available
2. `RolesUpdated` event is emitted with updated roles list
3. If the detached project's role was active, it should be cleared
4. Roles list prompt will be re-injected on next message

**Implementation**:
- Already handled in Phase 5 (`detach_project()` emits `RolesUpdated`)
- Need to add logic to clear active role if it belongs to detached project

**File: `packages/rhd_chat/src/projects.rs`**

```rust
pub fn detach_project<P: ProjectProvider>(
    db: &Arc<ChatDb>,
    project_provider: &Arc<P>,
    chat_id: i64,
    project_name: &str,
    event_sender: broadcast::Sender<ChatEvent>,
) -> Result<(), ChatError> {
    // Check if detached project has roles
    let had_roles = !project_provider.get_project_roles(project_name).is_empty();
    
    // Check if active role belongs to this project
    let active_role = db.get_active_role(chat_id)?;
    let should_clear_active_role = active_role
        .as_ref()
        .map(|(proj, _)| proj == project_name)
        .unwrap_or(false);
    
    db.detach_project(chat_id, project_name)?;
    
    // Clear active role if it belonged to detached project
    if should_clear_active_role {
        db.clear_active_role(chat_id)?;
        let _ = event_sender.send(ChatEvent::ActiveRoleCleared { chat_id });
    }
    
    // Emit RolesUpdated if project had roles
    if had_roles {
        db.reset_roles_list_injected(chat_id)?;
        let _ = event_sender.send(ChatEvent::RolesUpdated { chat_id });
    }
    
    let _ = event_sender.send(ChatEvent::ProjectDetached {
        chat_id,
        project_name: project_name.to_string(),
    });
    
    Ok(())
}
```

**Testing**:
```rust
#[tokio::test]
async fn test_detach_project_clears_active_role() {
    let db = Arc::new(ChatDb::new("test_detach_clear_role.db").unwrap());
    let chat_id = db.create_chat("Test").unwrap();
    
    let mut provider = MockProjectProvider { /* ... */ };
    provider.roles.insert("project-a".to_string(), vec![
        Role { name: "developer".to_string(), /* ... */ },
    ]);
    provider.role_prompts.insert(
        ("project-a".to_string(), "developer".to_string()),
        "Dev prompt".to_string(),
    );
    
    db.attach_project(chat_id, "project-a").unwrap();
    db.set_active_role(chat_id, "project-a", "developer").unwrap();
    
    let (event_sender, mut event_rx) = broadcast::channel(100);
    detach_project(&db, &Arc::new(provider), chat_id, "project-a", event_sender.clone()).unwrap();
    
    // Verify active role was cleared
    assert!(db.get_active_role(chat_id).unwrap().is_none());
    
    // Verify ActiveRoleCleared event was emitted
    let event = event_rx.recv().await.unwrap();
    assert!(matches!(event, ChatEvent::ActiveRoleCleared { .. }));
    
    cleanup("test_detach_clear_role.db");
}
```

### 7.3 Current Role's Project Detached

**Scenario**: The active role belongs to a project that is detached.

**Expected Behavior**:
- Same as 7.2: active role is cleared, `ActiveRoleCleared` event emitted

**Implementation**: Already handled in 7.2

### 7.4 No Roles Available

**Scenario**: No attached projects have roles, or all projects with roles are detached.

**Expected Behavior**:
1. `availableRoles` store is empty
2. `hasRoles` derived store is false
3. Role selector UI is hidden
4. `rhd_set_role` tool is not available (not added to tool list)
5. Roles list prompt is not injected

**Implementation**:
- Frontend: `RoleSelector` component only renders when `hasRoles` is true (Phase 6)
- Backend: `collect_builtin_tools()` only adds `rhd_set_role` when roles exist (Phase 4)
- Backend: `inject_roles_prompt()` returns early if no roles (Phase 3)

**Testing**:
```rust
#[tokio::test]
async fn test_no_roles_available() {
    let db = Arc::new(ChatDb::new("test_no_roles.db").unwrap());
    let chat_id = db.create_chat("Test").unwrap();
    
    let provider = MockProjectProvider {
        roles: HashMap::new(),
        role_prompts: HashMap::new(),
    };
    
    // Verify no built-in tools
    let tools = collect_builtin_tools(&db, &Arc::new(provider), chat_id);
    assert!(tools.is_empty());
    
    // Verify roles list not injected
    let (event_sender, _) = broadcast::channel(100);
    inject_roles_prompt(&db, &Arc::new(provider), chat_id, &event_sender).await.unwrap();
    let messages = db.get_messages(chat_id).unwrap();
    assert!(messages.is_empty()); // No system message injected
    
    cleanup("test_no_roles.db");
}
```

### 7.5 Role Switching During Streaming

**Scenario**: User tries to change role while AI is streaming a response.

**Expected Behavior**:
- Frontend: Role selector is disabled during streaming (`isStreaming` is true)
- Backend: If `rhd_set_role` tool is called during streaming, it should work (tool calls happen during streaming)

**Implementation**:
- Frontend: `RoleSelector` component has `disabled={$isStreaming}` (Phase 6)
- Backend: `rhd_set_role` tool works during streaming (it's a tool call, which happens during the tool loop)

**Testing**:
```typescript
// Frontend test
it('disables role selector during streaming', () => {
  isStreaming.set(true);
  const select = screen.getByLabelText('Role:');
  expect(select).toBeDisabled();
  
  isStreaming.set(false);
  expect(select).not.toBeDisabled();
});
```

### 7.6 Multiple Projects with Overlapping Role Names

**Scenario**: Two projects are attached, both have a role named "developer".

**Expected Behavior**:
- Attachment of second project is rejected with `RoleNameConflict` error
- User must detach one project or rename roles

**Implementation**: Already handled in Phase 3 (`check_role_conflicts()`)

**Testing**:
```rust
#[tokio::test]
async fn test_role_name_conflict_prevents_attachment() {
    let db = Arc::new(ChatDb::new("test_role_conflict.db").unwrap());
    let chat_id = db.create_chat("Test").unwrap();
    
    let mut provider = MockProjectProvider { /* ... */ };
    provider.roles.insert("project-a".to_string(), vec![
        Role { name: "developer".to_string(), /* ... */ },
    ]);
    
    db.attach_project(chat_id, "project-a").unwrap();
    
    provider.roles.insert("project-b".to_string(), vec![
        Role { name: "developer".to_string(), /* ... */ }, // Conflict!
    ]);
    
    let (event_sender, _) = broadcast::channel(100);
    let result = attach_project(&db, &Arc::new(provider), chat_id, "project-b", event_sender).await;
    
    assert!(matches!(result, Err(ChatError::RoleNameConflict(_))));
    
    // Verify project-b was not attached
    let projects = db.get_chat_projects(chat_id).unwrap();
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].0, "project-a");
    
    cleanup("test_role_conflict.db");
}
```

### 7.7 Role Injection After Edit and Resend

**Scenario**: User edits a message and resends. Roles should be re-injected if needed.

**Expected Behavior**:
- `inject_roles_prompt()` is called in `edit_and_resend()` (already done in Phase 3)
- Roles list is injected if not already injected

**Implementation**: Already handled in Phase 3

### 7.8 Chat Deletion with Active Role

**Scenario**: User deletes a chat that has an active role.

**Expected Behavior**:
- Chat is deleted (cascade deletes messages, project attachments)
- No special cleanup needed for role state (it's stored on the chat record)

**Implementation**: Already handled by database cascade delete

### 7.9 Reload Command with Roles

**Scenario**: User runs `rhd reload` while chats have active roles.

**Expected Behavior**:
- Projects are reloaded from disk
- Roles are reloaded
- Active roles remain valid if the role still exists
- If a role no longer exists after reload, it should be cleared

**Implementation**:
- Need to add logic to `handle_reload()` to validate active roles after reload
- This is complex and may be deferred to a future enhancement

**Note**: This is a complex edge case that may require significant work. For the initial implementation, we can document this as a known limitation and address it in a future phase.

### 7.10 Frontend State Export/Import with Roles

**Scenario**: User exports state, then imports it.

**Expected Behavior**:
- `availableRoles` and `activeRole` stores are exported
- On import, stores are restored
- UI reflects the restored state

**Implementation**:
- Update `stateExport.ts` to include role stores
- Update `stateImport.ts` to restore role stores

**File: `frontend/src/lib/stateExport.ts`**

```typescript
export function exportState(): any {
  return {
    // ... existing exports ...
    availableRoles: get(availableRoles),
    activeRole: get(activeRole),
  };
}
```

**File: `frontend/src/lib/stateImport.ts`**

```typescript
export function importState(state: any): void {
  // ... existing imports ...
  if (state.availableRoles) {
    availableRoles.set(state.availableRoles);
  }
  if (state.activeRole) {
    activeRole.set(state.activeRole);
  }
}
```

## Integration Testing

### 7.11 End-to-End Test Scenarios

Create comprehensive E2E tests that cover the full user journey:

**Test 1: Basic Role Selection**
1. Create a project with roles
2. Start a chat
3. Attach the project
4. Verify role selector appears
5. Select a role
6. Send a message
7. Verify role's system prompt is injected
8. Verify AI responds according to the role

**Test 2: Role Switching via Tool**
1. Create a project with multiple roles
2. Start a chat and attach the project
3. Send a message asking AI to switch roles
4. Verify AI calls `rhd_set_role` tool
5. Verify role changes and system prompt is injected
6. Verify AI responds according to new role

**Test 3: Project Detachment**
1. Attach a project with roles
2. Select a role
3. Detach the project
4. Verify role selector is hidden (if no other roles)
5. Verify active role is cleared

**Test 4: Role Name Conflict**
1. Create two projects with overlapping role names
2. Attach first project
3. Try to attach second project
4. Verify attachment is rejected with conflict error

### 7.12 Backend Integration Tests

**File: `tests/cases/roles-integration.md`** (or similar)

```markdown
# Roles Integration Test

## Setup
- Create test project with roles:
  - roles/developer/systemPrompt.md
  - roles/developer/whenToUse.md
  - roles/reviewer/systemPrompt.md
  - roles/reviewer/whenToUse.md

## Test Steps
1. Start daemon
2. Create chat
3. Attach project
4. Verify roles are available via getAvailableRoles
5. Set role to "developer"
6. Send message
7. Verify system prompt is injected
8. Switch role to "reviewer" via rhd_set_role tool
9. Verify role change event is emitted
10. Detach project
11. Verify active role is cleared
```

## Files to Modify

1. **`packages/rhd_chat/src/projects.rs`**
   - Update `detach_project()` to clear active role if it belongs to detached project
   - Add integration tests

2. **`frontend/src/lib/stateExport.ts`**
   - Add role stores to export

3. **`frontend/src/lib/stateImport.ts`**
   - Add role stores to import

4. **`tests/cases/roles-integration.md`** (NEW)
   - Add E2E test scenarios

## Dependencies

- All previous phases (1-6) must be complete

## Success Criteria

1. ✅ Project with roles attached after chat started works correctly
2. ✅ Role removed when project detached
3. ✅ Active role cleared when its project is detached
4. ✅ No roles available hides selector and disables tool
5. ✅ Role selector disabled during streaming
6. ✅ Role name conflicts prevent attachment
7. ✅ Role injection works after edit and resend
8. ✅ Chat deletion cleans up role state
9. ✅ Frontend state export/import includes role state
10. ✅ All edge case tests pass
11. ✅ Integration tests pass

## Design Decisions

1. **Active role cleared on project detach**: When a project is detached, if its role was active, we clear the active role. This prevents stale state.

2. **No automatic role selection**: When roles become available, we don't automatically select the first role. The user or AI must explicitly choose a role. This gives more control.

3. **Role selector disabled during streaming**: Prevents confusion and potential race conditions.

4. **State export/import includes roles**: Ensures debugging and testing workflows work correctly.

## Known Limitations

1. **Reload command**: Active roles are not validated after `rhd reload`. If a role no longer exists after reload, it may cause issues. This can be addressed in a future enhancement.

2. **Role inheritance/templates**: Not supported in this implementation. Roles are independent and don't inherit from each other.

3. **Role sharing between projects**: Not supported. Each project defines its own roles.

## Next Steps

After this phase:
- The roles feature is complete and ready for use
- Future enhancements can include:
  - Role templates/inheritance
  - Role sharing between projects
  - Role versioning
  - Role analytics
  - Validation of active roles after reload
