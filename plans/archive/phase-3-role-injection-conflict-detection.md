# Phase 3: Backend - Role Injection Logic & Conflict Detection

## Goal

Implement role system prompt injection and conflict detection. This phase handles injecting the roles list prompt, detecting role name conflicts when attaching projects, and managing role switching.

## Current State Analysis

### Project Attachment Flow (`packages/rhd_chat/src/projects.rs`)

```rust
pub async fn attach_project<P: ProjectProvider>(
    db: &Arc<ChatDb>,
    project_provider: &Arc<P>,
    chat_id: i64,
    project_name: &str,
    event_sender: broadcast::Sender<ChatEvent>,
) -> Result<(), ChatError> {
    // 1. Validate chat exists
    // 2. Validate project exists
    // 3. Check MCP ID conflicts
    // 4. Spawn MCP clients
    // 5. Record attachment in DB
    // 6. Emit ProjectAttached event
}
```

### System Prompt Injection (`packages/rhd_chat/src/projects.rs`)

```rust
pub async fn inject_system_prompts<P: ProjectProvider>(
    db: &Arc<ChatDb>,
    project_provider: &Arc<P>,
    chat_id: i64,
    event_sender: &broadcast::Sender<ChatEvent>,
) -> Result<(), ChatError> {
    // For each attached project:
    // - Check MCP status
    // - If system_prompt_added is false, inject system prompt and mark as added
}
```

### Message Sending Flow (`packages/rhd_chat/src/stream.rs`)

```rust
pub async fn send_message<P: ProjectProvider>(
    manager: &ChatManager<P>,
    chat_id: i64,
    content: String,
    model: &str,
    models: &HashMap<String, ModelConfig>,
    event_sender: broadcast::Sender<ChatEvent>,
    reload_lock: &tokio::sync::RwLock<()>,
) -> Result<i64, ChatError> {
    // 1. Acquire reload lock
    // 2. Check pause state
    // 3. Inject system prompts
    // 4. Update active model
    // 5. Add user message
    // 6. Build message history
    // 7. Collect tools
    // 8. Stream response
}
```

## Implementation Plan

### 3.1 Add RoleNameConflict Error Variant

**File: `packages/rhd_chat/src/error.rs`**

Add new error variant for role name conflicts:

```rust
#[derive(Debug, Error)]
pub enum ChatError {
    // ... existing variants ...
    
    #[error("role name conflict: {0}")]
    RoleNameConflict(String),
}
```

### 3.2 Implement Role Conflict Detection

**File: `packages/rhd_chat/src/projects.rs`**

Add function to check for role name conflicts:

```rust
pub fn check_role_conflicts<P: ProjectProvider>(
    db: &Arc<ChatDb>,
    project_provider: &Arc<P>,
    chat_id: i64,
    project_name: &str,
) -> Result<(), ChatError> {
    let attached_projects = db.get_chat_projects(chat_id)?;
    
    // Collect all role names from already-attached projects
    let mut existing_role_names = std::collections::HashSet::new();
    for (attached_name, _) in &attached_projects {
        for role in project_provider.get_project_roles(attached_name) {
            existing_role_names.insert(role.name);
        }
    }
    
    // Check for conflicts with new project's roles
    let mut conflicting_names = Vec::new();
    for role in project_provider.get_project_roles(project_name) {
        if existing_role_names.contains(&role.name) {
            conflicting_names.push(role.name);
        }
    }
    
    if !conflicting_names.is_empty() {
        return Err(ChatError::RoleNameConflict(format!(
            "Role names already in use: {}",
            conflicting_names.join(", ")
        )));
    }
    
    Ok(())
}
```

### 3.3 Update attach_project to Check Role Conflicts

**File: `packages/rhd_chat/src/projects.rs`**

Modify `attach_project()` to check for role conflicts and reset roles list injection flag:

```rust
pub async fn attach_project<P: ProjectProvider>(
    db: &Arc<ChatDb>,
    project_provider: &Arc<P>,
    chat_id: i64,
    project_name: &str,
    event_sender: broadcast::Sender<ChatEvent>,
) -> Result<(), ChatError> {
    if db.get_chat(chat_id)?.is_none() {
        return Err(ChatError::ChatNotFound);
    }

    if project_provider.get_project_system_prompt(project_name).is_none()
        && project_provider.get_project_mcp_refs(project_name).is_empty()
        && project_provider.get_project_roles(project_name).is_empty()
    {
        return Err(ChatError::ProjectNotFound(project_name.to_string()));
    }

    // Check MCP ID conflicts (existing code)
    let attached_projects = db.get_chat_projects(chat_id)?;
    let mut existing_mcp_ids = HashSet::new();
    for (attached_name, _) in &attached_projects {
        for mcp_ref in project_provider.get_project_mcp_refs(attached_name) {
            existing_mcp_ids.insert(mcp_ref.effective_id().to_string());
        }
    }

    let mut conflicting_ids = Vec::new();
    for mcp_ref in project_provider.get_project_mcp_refs(project_name) {
        let eid = mcp_ref.effective_id();
        if existing_mcp_ids.contains(eid) {
            conflicting_ids.push(eid.to_string());
        }
    }

    if !conflicting_ids.is_empty() {
        return Err(ChatError::McpIdConflict(format!(
            "MCP ids already in use: {}",
            conflicting_ids.join(", ")
        )));
    }

    // NEW: Check role name conflicts
    check_role_conflicts(db, project_provider, chat_id, project_name)?;

    project_provider
        .spawn_project_mcp(project_name)
        .await
        .map_err(ChatError::McpNotConnected)?;

    db.attach_project(chat_id, project_name)?;

    // NEW: If project has roles, reset roles list injection flag
    let roles = project_provider.get_project_roles(project_name);
    if !roles.is_empty() {
        db.reset_roles_list_injected(chat_id)?;
    }

    let _ = event_sender.send(ChatEvent::ProjectAttached {
        chat_id,
        project_name: project_name.to_string(),
    });

    Ok(())
}
```

### 3.4 Implement Roles List Injection

**File: `packages/rhd_chat/src/projects.rs`**

Add function to inject the roles list system prompt:

```rust
pub async fn inject_roles_prompt<P: ProjectProvider>(
    db: &Arc<ChatDb>,
    project_provider: &Arc<P>,
    chat_id: i64,
    event_sender: &broadcast::Sender<ChatEvent>,
) -> Result<(), ChatError> {
    // Check if roles list has already been injected
    if db.has_roles_list_been_injected(chat_id)? {
        return Ok(());
    }

    // Collect all roles from attached projects
    let attached_projects = db.get_chat_projects(chat_id)?;
    let mut all_roles: Vec<(String, Role)> = Vec::new(); // (project_name, role)
    
    for (project_name, _) in &attached_projects {
        let roles = project_provider.get_project_roles(project_name);
        for role in roles {
            all_roles.push((project_name.clone(), role));
        }
    }

    // If no roles available, nothing to inject
    if all_roles.is_empty() {
        return Ok(());
    }

    // Get current active role
    let active_role = db.get_active_role(chat_id)?;
    let current_role_name = active_role
        .as_ref()
        .map(|(_, name)| name.as_str())
        .unwrap_or("none");

    // Build roles list prompt
    let mut prompt = String::new();
    prompt.push_str(&format!(
        "Your behavior is defined by the current active role. Current active role is {}. You can switch your role by tool `rhd_set_role`.\n\n",
        current_role_name
    ));
    prompt.push_str("These are the currently available roles:\n");

    for (project_name, role) in &all_roles {
        prompt.push_str(&format!("\n# {} (from project: {})\n\n", role.name, project_name));
        prompt.push_str(&role.when_to_use);
        prompt.push('\n');
    }

    // Inject as system message
    let system_message_id = db.add_message(chat_id, "system", &prompt, None, None)?;
    let system_message = rhd_db::Message {
        id: system_message_id,
        chat_id,
        role: "system".to_string(),
        content: prompt,
        created_at: chrono::Utc::now().to_rfc3339(),
        model: None,
        thinking_content: None,
    };
    let _ = event_sender.send(ChatEvent::MessageAdded {
        chat_id,
        message: system_message,
    });

    // Mark as injected
    db.mark_roles_list_injected(chat_id)?;

    Ok(())
}
```

### 3.5 Implement Role Switch Prompt Injection

**File: `packages/rhd_chat/src/projects.rs`**

Add function to inject role's system prompt when role changes:

```rust
pub fn inject_role_system_prompt<P: ProjectProvider>(
    db: &Arc<ChatDb>,
    project_provider: &Arc<P>,
    chat_id: i64,
    project_name: &str,
    role_name: &str,
    event_sender: &broadcast::Sender<ChatEvent>,
) -> Result<(), ChatError> {
    let system_prompt = project_provider
        .get_role_system_prompt(project_name, role_name)
        .ok_or_else(|| {
            ChatError::RoleNotFound(format!("{}:{}", project_name, role_name))
        })?;

    let mut prompt = String::new();
    prompt.push_str(&format!("Your current role is now {}.\n\n", role_name));
    prompt.push_str("-----\n\n");
    prompt.push_str(&system_prompt);

    let system_message_id = db.add_message(chat_id, "system", &prompt, None, None)?;
    let system_message = rhd_db::Message {
        id: system_message_id,
        chat_id,
        role: "system".to_string(),
        content: prompt,
        created_at: chrono::Utc::now().to_rfc3339(),
        model: None,
        thinking_content: None,
    };
    let _ = event_sender.send(ChatEvent::MessageAdded {
        chat_id,
        message: system_message,
    });

    Ok(())
}
```

### 3.6 Add RoleNotFound Error Variant

**File: `packages/rhd_chat/src/error.rs`**

```rust
#[derive(Debug, Error)]
pub enum ChatError {
    // ... existing variants ...
    
    #[error("role not found: {0}")]
    RoleNotFound(String),
}
```

### 3.7 Integrate Role Injection into send_message

**File: `packages/rhd_chat/src/stream.rs`**

Update `send_message()` to call `inject_roles_prompt()`:

```rust
pub async fn send_message<P: ProjectProvider>(
    manager: &ChatManager<P>,
    chat_id: i64,
    content: String,
    model: &str,
    models: &HashMap<String, ModelConfig>,
    event_sender: broadcast::Sender<ChatEvent>,
    reload_lock: &tokio::sync::RwLock<()>,
) -> Result<i64, ChatError> {
    let _reload_guard = reload_lock.read().await;
    let chat_info = manager.db().get_chat(chat_id)?.ok_or(ChatError::ChatNotFound)?;
    let chat_title = chat_info.title.clone();

    let pause_notify = manager.get_paused_notify(chat_id).await;

    if let Some(notify) = pause_notify {
        // ... existing pause handling ...
    }

    let model_config = models
        .get(model)
        .ok_or_else(|| ChatError::ModelNotFound(model.to_string()))?;

    // Inject system prompts (existing)
    projects::inject_system_prompts(
        manager.db(),
        manager.project_provider(),
        chat_id,
        &event_sender,
    )
    .await?;

    // NEW: Inject roles list prompt
    projects::inject_roles_prompt(
        manager.db(),
        manager.project_provider(),
        chat_id,
        &event_sender,
    )
    .await?;

    manager.db().update_chat_active_model(chat_id, model)?;

    // ... rest of existing code ...
}
```

Similarly update `edit_and_resend()`:

```rust
pub async fn edit_and_resend<P: ProjectProvider>(
    manager: &ChatManager<P>,
    message_id: i64,
    new_content: String,
    model: &str,
    models: &HashMap<String, ModelConfig>,
    event_sender: broadcast::Sender<ChatEvent>,
    reload_lock: &tokio::sync::RwLock<()>,
) -> Result<i64, ChatError> {
    // ... existing code ...

    projects::inject_system_prompts(
        manager.db(),
        manager.project_provider(),
        chat_id,
        &event_sender,
    )
    .await?;

    // NEW: Inject roles list prompt
    projects::inject_roles_prompt(
        manager.db(),
        manager.project_provider(),
        chat_id,
        &event_sender,
    )
    .await?;

    // ... rest of existing code ...
}
```

### 3.8 Add Role Management Methods to ChatManager

**File: `packages/rhd_chat/src/manager.rs`**

Add methods for role management:

```rust
impl<P: ProjectProvider> ChatManager<P> {
    // ... existing methods ...

    /// Sets the active role for a chat and injects the role's system prompt
    pub async fn set_active_role(
        &self,
        chat_id: i64,
        project_name: &str,
        role_name: &str,
        event_sender: broadcast::Sender<ChatEvent>,
    ) -> Result<(), ChatError> {
        // Validate chat exists
        if self.db.get_chat(chat_id)?.is_none() {
            return Err(ChatError::ChatNotFound);
        }

        // Validate role exists
        let role = self
            .project_provider
            .get_role_system_prompt(project_name, role_name)
            .ok_or_else(|| ChatError::RoleNotFound(format!("{}:{}", project_name, role_name)))?;

        // Set active role in DB
        self.db.set_active_role(chat_id, project_name, role_name)?;

        // Reset roles list injection flag so it will be re-injected with updated current role
        self.db.reset_roles_list_injected(chat_id)?;

        // Inject role's system prompt
        projects::inject_role_system_prompt(
            &self.db,
            &self.project_provider,
            chat_id,
            project_name,
            role_name,
            &event_sender,
        )?;

        Ok(())
    }

    /// Clears the active role for a chat
    pub fn clear_active_role(&self, chat_id: i64) -> Result<(), ChatError> {
        if self.db.get_chat(chat_id)?.is_none() {
            return Err(ChatError::ChatNotFound);
        }
        self.db.clear_active_role(chat_id)?;
        self.db.reset_roles_list_injected(chat_id)?;
        Ok(())
    }

    /// Gets the active role for a chat
    pub fn get_active_role(&self, chat_id: i64) -> Result<Option<(String, String)>, ChatError> {
        Ok(self.db.get_active_role(chat_id)?)
    }

    /// Gets all available roles from attached projects
    pub fn get_available_roles(&self, chat_id: i64) -> Result<Vec<(String, String, String)>, ChatError> {
        // Returns Vec<(project_name, role_name, when_to_use)>
        if self.db.get_chat(chat_id)?.is_none() {
            return Err(ChatError::ChatNotFound);
        }

        let attached_projects = self.db.get_chat_projects(chat_id)?;
        let mut roles = Vec::new();

        for (project_name, _) in attached_projects {
            for role in self.project_provider.get_project_roles(&project_name) {
                roles.push((project_name.clone(), role.name, role.when_to_use));
            }
        }

        Ok(roles)
    }
}
```

### 3.9 Add Unit Tests

**File: `packages/rhd_chat/src/projects.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use rhd_api::project::Role;
    use std::sync::Arc;
    use tokio::sync::broadcast;

    // Mock ProjectProvider for testing
    struct MockProjectProvider {
        roles: HashMap<String, Vec<Role>>,
        role_prompts: HashMap<(String, String), String>,
    }

    #[async_trait]
    impl ProjectProvider for MockProjectProvider {
        async fn get_mcp_status(&self, _project_name: &str) -> Vec<(String, McpStatus)> {
            vec![]
        }

        fn get_project_system_prompt(&self, _project_name: &str) -> Option<String> {
            None
        }

        fn get_project_mcp_refs(&self, _project_name: &str) -> Vec<McpRef> {
            vec![]
        }

        async fn get_mcp_clients(&self, _project_name: &str) -> Vec<(String, Arc<McpClient>)> {
            vec![]
        }

        async fn spawn_project_mcp(&self, _project_name: &str) -> Result<(), String> {
            Ok(())
        }

        fn get_project_roles(&self, project_name: &str) -> Vec<Role> {
            self.roles.get(project_name).cloned().unwrap_or_default()
        }

        fn get_role_system_prompt(&self, project_name: &str, role_name: &str) -> Option<String> {
            self.role_prompts
                .get(&(project_name.to_string(), role_name.to_string()))
                .cloned()
        }
    }

    #[tokio::test]
    async fn test_check_role_conflicts_no_conflict() {
        let db = Arc::new(ChatDb::new("test_no_conflict.db").unwrap());
        let chat_id = db.create_chat("Test").unwrap();

        let mut provider = MockProjectProvider {
            roles: HashMap::new(),
            role_prompts: HashMap::new(),
        };
        provider.roles.insert("project-a".to_string(), vec![
            Role {
                name: "developer".to_string(),
                system_prompt: "Dev".to_string(),
                when_to_use: "Coding".to_string(),
            },
        ]);

        db.attach_project(chat_id, "project-a").unwrap();

        // Attach project-b with different role names
        provider.roles.insert("project-b".to_string(), vec![
            Role {
                name: "reviewer".to_string(),
                system_prompt: "Review".to_string(),
                when_to_use: "Reviewing".to_string(),
            },
        ]);

        let result = check_role_conflicts(&db, &Arc::new(provider), chat_id, "project-b");
        assert!(result.is_ok());

        cleanup("test_no_conflict.db");
    }

    #[tokio::test]
    async fn test_check_role_conflicts_with_conflict() {
        let db = Arc::new(ChatDb::new("test_conflict.db").unwrap());
        let chat_id = db.create_chat("Test").unwrap();

        let mut provider = MockProjectProvider {
            roles: HashMap::new(),
            role_prompts: HashMap::new(),
        };
        provider.roles.insert("project-a".to_string(), vec![
            Role {
                name: "developer".to_string(),
                system_prompt: "Dev".to_string(),
                when_to_use: "Coding".to_string(),
            },
        ]);

        db.attach_project(chat_id, "project-a").unwrap();

        // Try to attach project-b with same role name
        provider.roles.insert("project-b".to_string(), vec![
            Role {
                name: "developer".to_string(), // Conflict!
                system_prompt: "Dev 2".to_string(),
                when_to_use: "Coding 2".to_string(),
            },
        ]);

        let result = check_role_conflicts(&db, &Arc::new(provider), chat_id, "project-b");
        assert!(matches!(result, Err(ChatError::RoleNameConflict(_))));

        cleanup("test_conflict.db");
    }

    #[tokio::test]
    async fn test_inject_roles_prompt() {
        let db = Arc::new(ChatDb::new("test_inject_roles.db").unwrap());
        let chat_id = db.create_chat("Test").unwrap();

        let mut provider = MockProjectProvider {
            roles: HashMap::new(),
            role_prompts: HashMap::new(),
        };
        provider.roles.insert("project-a".to_string(), vec![
            Role {
                name: "developer".to_string(),
                system_prompt: "You are a developer.".to_string(),
                when_to_use: "Use for coding tasks.".to_string(),
            },
            Role {
                name: "reviewer".to_string(),
                system_prompt: "You are a reviewer.".to_string(),
                when_to_use: "Use for code review.".to_string(),
            },
        ]);

        db.attach_project(chat_id, "project-a").unwrap();

        let (event_sender, _) = broadcast::channel(100);
        let provider = Arc::new(provider);

        // First injection
        inject_roles_prompt(&db, &provider, chat_id, &event_sender).await.unwrap();

        // Verify system message was added
        let messages = db.get_messages(chat_id).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "system");
        assert!(messages[0].content.contains("developer"));
        assert!(messages[0].content.contains("reviewer"));

        // Second injection should be no-op
        inject_roles_prompt(&db, &provider, chat_id, &event_sender).await.unwrap();
        let messages = db.get_messages(chat_id).unwrap();
        assert_eq!(messages.len(), 1); // Still only one system message

        cleanup("test_inject_roles.db");
    }

    #[tokio::test]
    async fn test_inject_role_system_prompt() {
        let db = Arc::new(ChatDb::new("test_inject_role_prompt.db").unwrap());
        let chat_id = db.create_chat("Test").unwrap();

        let mut provider = MockProjectProvider {
            roles: HashMap::new(),
            role_prompts: HashMap::new(),
        };
        provider.role_prompts.insert(
            ("project-a".to_string(), "developer".to_string()),
            "You are a developer.".to_string(),
        );

        let (event_sender, _) = broadcast::channel(100);
        let provider = Arc::new(provider);

        inject_role_system_prompt(
            &db,
            &provider,
            chat_id,
            "project-a",
            "developer",
            &event_sender,
        )
        .unwrap();

        let messages = db.get_messages(chat_id).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "system");
        assert!(messages[0].content.contains("Your current role is now developer"));
        assert!(messages[0].content.contains("You are a developer."));

        cleanup("test_inject_role_prompt.db");
    }
}
```

## Files to Modify

1. **`packages/rhd_chat/src/error.rs`**
   - Add `RoleNameConflict(String)` variant
   - Add `RoleNotFound(String)` variant

2. **`packages/rhd_chat/src/projects.rs`**
   - Add `check_role_conflicts()` function
   - Update `attach_project()` to check role conflicts and reset injection flag
   - Add `inject_roles_prompt()` function
   - Add `inject_role_system_prompt()` function
   - Add comprehensive unit tests

3. **`packages/rhd_chat/src/stream.rs`**
   - Update `send_message()` to call `inject_roles_prompt()`
   - Update `edit_and_resend()` to call `inject_roles_prompt()`

4. **`packages/rhd_chat/src/manager.rs`**
   - Add `set_active_role()` method
   - Add `clear_active_role()` method
   - Add `get_active_role()` method
   - Add `get_available_roles()` method

## Dependencies

- Phase 1 must be complete (Role struct, role loading)
- Phase 2 must be complete (ProjectProvider trait extensions, DB methods)

## Success Criteria

1. ✅ Role name conflicts detected before project attachment
2. ✅ Attachment rejected with list of conflicting role names
3. ✅ Roles list prompt injected on first message when roles available
4. ✅ Roles list prompt includes all roles from all attached projects
5. ✅ Roles list prompt includes current active role indication
6. ✅ Role system prompt injected when role is selected
7. ✅ Roles list re-injected when role changes (with updated current role)
8. ✅ `ChatManager` provides methods for role management
9. ✅ All unit tests pass
10. ✅ Existing tests still pass

## Design Decisions

1. **Roles list prompt includes current active role**: This means the prompt needs to be re-injected when the role changes. We handle this by resetting the `roles_list_injected` flag when the role changes.

2. **Role switch prompt is separate**: When a role is selected, we inject a separate system message with the role's system prompt. This keeps the concerns separate.

3. **Conflict detection before attachment**: Similar to MCP ID conflicts, we check for role name conflicts before actually attaching the project. This prevents inconsistent state.

4. **Roles list includes project name**: Since roles are namespaced by project, the roles list prompt includes the project name to avoid ambiguity.

## Next Steps

After this phase:
- Phase 4 will implement the `rhd_set_role` tool for AI to switch roles
- Phase 5 will add WebSocket protocol for role management
- Phase 6 will implement the frontend role selector UI
