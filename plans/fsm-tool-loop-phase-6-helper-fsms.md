# Phase 6: Helper FSMs Implementation

## Overview

This phase implements full `TodoListFsm` and `RolesFsm` to handle built-in tools, moving logic from the current implementation into these dedicated FSMs. These helper FSMs use the event interception mechanism to intercept `ToolCallRequested` events for built-in tools and handle them without propagating to the MCP client.

**Scope:**
- Implement `TodoListFsm` with complete state management
- Implement `RolesFsm` with role switching logic
- Implement listener methods for each FSM
- Integrate with `FsmToolLoop` via listener system
- Move built-in tool handling from async wrapper to helper FSMs
- Use event interception with `propagate` flag to prevent tool call propagation
- Update existing code to use new FSMs instead of direct handlers

**Out of Scope:**
- Plugin system implementation
- Changes to DB schema

## Files to Create

### 1. `packages/rhd_fsm/src/todo_list_fsm.rs` (NEW FILE)

**Purpose:** FSM for managing todo list state and handling `rhd_set_todo_list` tool calls.

**Complete Implementation:**

```rust
use std::sync::atomic::Ordering;
use std::sync::Arc;

use crate::tool_loop_fsm::{ToolLoopFsmEvent, ToolLoopListenerId};

/// Represents the current state of the todo list
#[derive(Debug, Clone, PartialEq)]
pub enum TodoListState {
    /// No todo list has been set yet
    Empty,
    /// Todo list has been set with items
    Active { items: Vec<TodoItem> },
}

/// Represents a single todo item
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TodoItem {
    pub id: String,
    pub content: String,
    pub completed: bool,
}

/// FSM for managing todo list state
pub struct TodoListFsm {
    state: TodoListState,
    listener_id: Option<ToolLoopListenerId>,
}

impl TodoListFsm {
    /// Create a new TodoListFsm in Empty state
    pub fn new() -> Self {
        Self {
            state: TodoListState::Empty,
            listener_id: None,
        }
    }

    /// Get current state
    pub fn state(&self) -> &TodoListState {
        &self.state
    }

    /// Check if todo list is empty
    pub fn is_empty(&self) -> bool {
        matches!(self.state, TodoListState::Empty)
    }

    /// Get todo items if active
    pub fn items(&self) -> Option<&[TodoItem]> {
        match &self.state {
            TodoListState::Active { items } => Some(items),
            TodoListState::Empty => None,
        }
    }

    /// Create a listener callback for this FSM
    /// 
    /// This callback intercepts `ToolCallRequested` events for `rhd_set_todo_list` tool
    /// and sets `propagate` to false to prevent the tool call from being executed by MCP.
    pub fn create_listener(&mut self) -> Arc<dyn Fn(ToolLoopFsmEvent) + Send + Sync> {
        let state = Arc::new(std::sync::Mutex::new(self.state.clone()));
        let state_clone = state.clone();

        Arc::new(move |event: ToolLoopFsmEvent| {
            if let ToolLoopFsmEvent::ToolCallRequested { tool_call, propagate } = event {
                if tool_call.name == "rhd_set_todo_list" {
                    // Parse todo list from tool call arguments
                    if let Ok(items) = parse_todo_list_from_arguments(&tool_call.arguments) {
                        // Update state
                        let mut state_guard = state_clone.lock().unwrap();
                        *state_guard = TodoListState::Active { items };
                        
                        // Prevent propagation to MCP
                        propagate.store(false, Ordering::SeqCst);
                    }
                }
            }
        })
    }

    /// Register this FSM as a listener on the tool loop FSM
    pub fn register(&mut self, tool_loop_fsm: &mut crate::ToolLoopFsm) -> ToolLoopListenerId {
        let listener = self.create_listener();
        let id = tool_loop_fsm.add_listener(listener);
        self.listener_id = Some(id);
        id
    }

    /// Unregister this FSM from the tool loop FSM
    pub fn unregister(&mut self, tool_loop_fsm: &mut crate::ToolLoopFsm) -> bool {
        if let Some(id) = self.listener_id {
            let removed = tool_loop_fsm.remove_listener(id);
            self.listener_id = None;
            removed
        } else {
            false
        }
    }

    /// Handle the todo list update directly
    /// 
    /// This method is called by the async wrapper when a `rhd_set_todo_list` tool call
    /// is intercepted. It parses the arguments and updates the state.
    pub fn handle_set_todo_list(&mut self, arguments: &str) -> Result<(), String> {
        let items = parse_todo_list_from_arguments(arguments)?;
        self.state = TodoListState::Active { items };
        Ok(())
    }

    /// Generate a todo list message for injection into the conversation
    pub fn generate_todo_list_message(&self, template_loader: &dyn TemplateLoader) -> Option<String> {
        match &self.state {
            TodoListState::Empty => {
                // Load empty todo list template
                template_loader.load_template("todo_list_empty")
            }
            TodoListState::Active { items } => {
                // Load todo list with items template
                let context = TodoListContext { items: items.clone() };
                template_loader.load_template_with_context("todo_list_with_items", &context)
            }
        }
    }
}

impl Default for TodoListFsm {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse todo list from tool call arguments
fn parse_todo_list_from_arguments(arguments: &str) -> Result<Vec<TodoItem>, String> {
    let json: serde_json::Value = serde_json::from_str(arguments)
        .map_err(|e| format!("Failed to parse arguments: {}", e))?;

    let items_array = json
        .get("items")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "Missing 'items' array".to_string())?;

    let mut items = Vec::new();
    for (index, item_value) in items_array.iter().enumerate() {
        let content = item_value
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("Item {} missing 'content'", index))?;

        let completed = item_value
            .get("completed")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        items.push(TodoItem {
            id: format!("todo_{}", index),
            content: content.to_string(),
            completed,
        });
    }

    Ok(items)
}

/// Context for todo list template rendering
#[derive(serde::Serialize)]
struct TodoListContext {
    items: Vec<TodoItem>,
}

/// Trait for template loading
pub trait TemplateLoader {
    fn load_template(&self, template_name: &str) -> Option<String>;
    fn load_template_with_context<T: serde::Serialize>(
        &self,
        template_name: &str,
        context: &T,
    ) -> Option<String>;
}
```

### 2. `packages/rhd_fsm/src/roles_fsm.rs` (NEW FILE)

**Purpose:** FSM for managing role state and handling `rhd_set_role` tool calls.

**Complete Implementation:**

```rust
use std::sync::atomic::Ordering;
use std::sync::Arc;

use crate::tool_loop_fsm::{ToolLoopFsmEvent, ToolLoopListenerId};

/// Represents the current role state
#[derive(Debug, Clone, PartialEq)]
pub enum RoleState {
    /// No role is active
    None,
    /// A role is active
    Active { role_name: String },
}

/// FSM for managing role state
pub struct RolesFsm {
    state: RoleState,
    listener_id: Option<ToolLoopListenerId>,
}

impl RolesFsm {
    /// Create a new RolesFsm with no active role
    pub fn new() -> Self {
        Self {
            state: RoleState::None,
            listener_id: None,
        }
    }

    /// Get current state
    pub fn state(&self) -> &RoleState {
        &self.state
    }

    /// Check if a role is active
    pub fn has_active_role(&self) -> bool {
        matches!(self.state, RoleState::Active { .. })
    }

    /// Get active role name
    pub fn active_role(&self) -> Option<&str> {
        match &self.state {
            RoleState::Active { role_name } => Some(role_name),
            RoleState::None => None,
        }
    }

    /// Create a listener callback for this FSM
    /// 
    /// This callback intercepts `ToolCallRequested` events for `rhd_set_role` tool
    /// and sets `propagate` to false to prevent the tool call from being executed by MCP.
    pub fn create_listener(&mut self) -> Arc<dyn Fn(ToolLoopFsmEvent) + Send + Sync> {
        let state = Arc::new(std::sync::Mutex::new(self.state.clone()));
        let state_clone = state.clone();

        Arc::new(move |event: ToolLoopFsmEvent| {
            if let ToolLoopFsmEvent::ToolCallRequested { tool_call, propagate } = event {
                if tool_call.name == "rhd_set_role" {
                    // Parse role name from tool call arguments
                    if let Ok(role_name) = parse_role_from_arguments(&tool_call.arguments) {
                        // Update state
                        let mut state_guard = state_clone.lock().unwrap();
                        *state_guard = RoleState::Active { role_name };
                        
                        // Prevent propagation to MCP
                        propagate.store(false, Ordering::SeqCst);
                    }
                }
            }
        })
    }

    /// Register this FSM as a listener on the tool loop FSM
    pub fn register(&mut self, tool_loop_fsm: &mut crate::ToolLoopFsm) -> ToolLoopListenerId {
        let listener = self.create_listener();
        let id = tool_loop_fsm.add_listener(listener);
        self.listener_id = Some(id);
        id
    }

    /// Unregister this FSM from the tool loop FSM
    pub fn unregister(&mut self, tool_loop_fsm: &mut crate::ToolLoopFsm) -> bool {
        if let Some(id) = self.listener_id {
            let removed = tool_loop_fsm.remove_listener(id);
            self.listener_id = None;
            removed
        } else {
            false
        }
    }

    /// Handle the role switch directly
    /// 
    /// This method is called by the async wrapper when a `rhd_set_role` tool call
    /// is intercepted. It parses the arguments and updates the state.
    pub fn handle_set_role(&mut self, arguments: &str) -> Result<(), String> {
        let role_name = parse_role_from_arguments(arguments)?;
        self.state = RoleState::Active { role_name };
        Ok(())
    }

    /// Generate a role switch prompt for injection into the conversation
    pub fn generate_role_switch_prompt(&self, template_loader: &dyn RoleTemplateLoader) -> Option<String> {
        match &self.state {
            RoleState::None => None,
            RoleState::Active { role_name } => {
                let context = RoleContext {
                    role_name: role_name.clone(),
                };
                template_loader.load_template_with_context("role_switch_prompt", &context)
            }
        }
    }
}

impl Default for RolesFsm {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse role name from tool call arguments
fn parse_role_from_arguments(arguments: &str) -> Result<String, String> {
    let json: serde_json::Value = serde_json::from_str(arguments)
        .map_err(|e| format!("Failed to parse arguments: {}", e))?;

    let role_name = json
        .get("role")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing 'role' field".to_string())?;

    Ok(role_name.to_string())
}

/// Context for role template rendering
#[derive(serde::Serialize)]
struct RoleContext {
    role_name: String,
}

/// Trait for role template loading
pub trait RoleTemplateLoader {
    fn load_template_with_context<T: serde::Serialize>(
        &self,
        template_name: &str,
        context: &T,
    ) -> Option<String>;
}
```

### 3. `packages/rhd_chat/src/tools/builtin_fsms.rs` (NEW FILE)

**Purpose:** Integration of helper FSMs with the tool loop.

**Complete Implementation:**

```rust
use std::sync::Arc;

use rhd_fsm::{RolesFsm, TodoListFsm, ToolLoopFsm};

use crate::error::ChatError;
use crate::event::ChatEvent;
use crate::manager::ChatManager;
use crate::stream::TemplateLoaderRef;
use crate::ProjectProvider;

use super::builtin::{handle_rhd_set_role, handle_rhd_set_todo_list, inject_todo_list_message};

/// Manages helper FSMs for built-in tools
pub struct BuiltinFsmManager {
    pub todo_list_fsm: TodoListFsm,
    pub roles_fsm: RolesFsm,
}

impl BuiltinFsmManager {
    /// Create a new BuiltinFsmManager
    pub fn new() -> Self {
        Self {
            todo_list_fsm: TodoListFsm::new(),
            roles_fsm: RolesFsm::new(),
        }
    }

    /// Register helper FSMs with the tool loop FSM
    pub fn register(&mut self, tool_loop_fsm: &mut ToolLoopFsm) {
        self.todo_list_fsm.register(tool_loop_fsm);
        self.roles_fsm.register(tool_loop_fsm);
    }

    /// Unregister helper FSMs from the tool loop FSM
    pub fn unregister(&mut self, tool_loop_fsm: &mut ToolLoopFsm) {
        self.todo_list_fsm.unregister(tool_loop_fsm);
        self.roles_fsm.unregister(tool_loop_fsm);
    }

    /// Handle a built-in tool call
    /// 
    /// This method is called by the async wrapper when a built-in tool call
    /// is intercepted. It delegates to the appropriate helper FSM.
    pub async fn handle_builtin_tool<P: ProjectProvider>(
        &mut self,
        manager: &ChatManager<P>,
        chat_id: i64,
        tool_name: &str,
        arguments: &str,
        event_sender: &tokio::sync::broadcast::Sender<ChatEvent>,
    ) -> Result<rhd_mcp_client::ToolResult, ChatError> {
        match tool_name {
            "rhd_set_todo_list" => {
                self.todo_list_fsm.handle_set_todo_list(arguments)
                    .map_err(|e| ChatError::Internal(e))?;
                
                let result = handle_rhd_set_todo_list(manager, chat_id, arguments, event_sender).await;
                Ok(result)
            }
            "rhd_set_role" => {
                self.roles_fsm.handle_set_role(arguments)
                    .map_err(|e| ChatError::Internal(e))?;
                
                let result = handle_rhd_set_role(manager, chat_id, arguments, event_sender).await;
                Ok(result)
            }
            _ => Err(ChatError::Internal(format!("Unknown built-in tool: {}", tool_name))),
        }
    }

    /// Inject todo list message if needed
    pub fn inject_todo_list_message<P: ProjectProvider>(
        &self,
        manager: &ChatManager<P>,
        chat_id: i64,
        template_loader: &TemplateLoaderRef,
        event_sender: &tokio::sync::broadcast::Sender<ChatEvent>,
    ) -> Result<(), ChatError> {
        if !self.todo_list_fsm.is_empty() {
            inject_todo_list_message(manager, chat_id, template_loader, event_sender)?;
        }
        Ok(())
    }
}

impl Default for BuiltinFsmManager {
    fn default() -> Self {
        Self::new()
    }
}
```

## Files to Modify

### 4. `packages/rhd_fsm/src/lib.rs`

**Modifications:**

Add exports for new FSMs:

```rust
pub mod todo_list_fsm;
pub mod roles_fsm;

pub use todo_list_fsm::{TodoListFsm, TodoListState, TodoItem};
pub use roles_fsm::{RolesFsm, RoleState};
```

### 5. `packages/rhd_chat/src/tools/mod.rs`

**Modifications:**

Add export for builtin FSMs:

```rust
pub mod builtin_fsms;
```

### 6. `packages/rhd_chat/src/tools/builtin.rs`

**Modifications:**

Refactor to use FSMs instead of direct handlers. The existing functions (`handle_rhd_set_todo_list`, `handle_rhd_set_role`, `inject_todo_list_message`) remain for backward compatibility, but the logic is delegated to the helper FSMs.

### 7. `packages/rhd_chat/src/tools/tool_loop.rs`

**Modifications:**

Remove built-in tool handling from wrapper. The `execute_tool_call` function should check if the tool is a built-in tool and delegate to `BuiltinFsmManager` instead of handling it directly.

```rust
pub async fn execute_tool_call<P: ProjectProvider>(
    manager: &ChatManager<P>,
    chat_id: i64,
    tool_call: &ToolCall,
    mcp_clients: &[(String, String, Arc<McpClient>)],
    event_sender: &broadcast::Sender<ChatEvent>,
    builtin_fsm_manager: &mut BuiltinFsmManager,  // NEW: Pass FSM manager
) -> (ToolResult, String) {
    let tool_name = &tool_call.function.name;

    // Check if it's a built-in tool
    if tool_name == RHD_SET_TODO_LIST_TOOL_NAME || tool_name == RHD_SET_ROLE_TOOL_NAME {
        let result = builtin_fsm_manager
            .handle_builtin_tool(manager, chat_id, tool_name, &tool_call.function.arguments, event_sender)
            .await;
        
        match result {
            Ok(tool_result) => return (tool_result, String::new()),
            Err(e) => return (ToolResult {
                content: format!("Error: {}", e),
                is_error: Some(true),
                raw_response: None,
            }, String::new()),
        }
    }

    // Handle MCP tools
    let (mcp_id, bare_tool_name) = split_tool_name(tool_name);
    for (_project_name, client_mcp_id, client) in mcp_clients {
        if *client_mcp_id == mcp_id {
            match client.call_tool(&bare_tool_name, &tool_call.function.arguments).await {
                Ok(result) => return (result, client_mcp_id.clone()),
                Err(e) => return (ToolResult {
                    content: format!("Error: {}", e),
                    is_error: Some(true),
                    raw_response: None,
                }, client_mcp_id.clone()),
            }
        }
    }
    (ToolResult {
        content: format!("Error: unknown tool '{}'", tool_call.function.name),
        is_error: Some(true),
        raw_response: None,
    }, String::new())
}
```

### 8. `packages/rhd_chat/src/tools/fsm_wrapper.rs`

**Modifications:**

Integrate `BuiltinFsmManager` into the wrapper:

```rust
pub struct FsmToolLoop<'a, P: ProjectProvider> {
    fsm: ToolLoopFsm,
    builtin_fsm_manager: BuiltinFsmManager,  // NEW: Add FSM manager
    // ... other fields ...
}

impl<'a, P: ProjectProvider> FsmToolLoop<'a, P> {
    pub fn new(/* ... */) -> Self {
        let mut fsm = ToolLoopFsm::with_message_id_counter(initial_message_id);
        let mut builtin_fsm_manager = BuiltinFsmManager::new();
        
        // Register helper FSMs
        builtin_fsm_manager.register(&mut fsm);
        
        Self {
            fsm,
            builtin_fsm_manager,
            // ... other fields ...
        }
    }
}
```

## Tests

### Unit Tests for TodoListFsm

Create `packages/rhd_fsm/src/todo_list_fsm/tests.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state() {
        let fsm = TodoListFsm::new();
        assert!(fsm.is_empty());
        assert_eq!(fsm.state(), &TodoListState::Empty);
    }

    #[test]
    fn test_handle_set_todo_list() {
        let mut fsm = TodoListFsm::new();
        let arguments = r#"{"items": [{"content": "Task 1", "completed": false}]}"#;
        
        fsm.handle_set_todo_list(arguments).unwrap();
        
        assert!(!fsm.is_empty());
        assert_eq!(fsm.items().unwrap().len(), 1);
        assert_eq!(fsm.items().unwrap()[0].content, "Task 1");
    }

    #[test]
    fn test_parse_todo_list_from_arguments() {
        let arguments = r#"{"items": [{"content": "Task 1", "completed": false}, {"content": "Task 2", "completed": true}]}"#;
        
        let items = parse_todo_list_from_arguments(arguments).unwrap();
        
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].content, "Task 1");
        assert!(!items[0].completed);
        assert_eq!(items[1].content, "Task 2");
        assert!(items[1].completed);
    }
}
```

### Unit Tests for RolesFsm

Create `packages/rhd_fsm/src/roles_fsm/tests.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state() {
        let fsm = RolesFsm::new();
        assert!(!fsm.has_active_role());
        assert_eq!(fsm.state(), &RoleState::None);
    }

    #[test]
    fn test_handle_set_role() {
        let mut fsm = RolesFsm::new();
        let arguments = r#"{"role": "developer"}"#;
        
        fsm.handle_set_role(arguments).unwrap();
        
        assert!(fsm.has_active_role());
        assert_eq!(fsm.active_role(), Some("developer"));
    }

    #[test]
    fn test_parse_role_from_arguments() {
        let arguments = r#"{"role": "reviewer"}"#;
        
        let role_name = parse_role_from_arguments(arguments).unwrap();
        
        assert_eq!(role_name, "reviewer");
    }
}
```

### Integration Tests

Add integration tests in `packages/rhd_chat/src/tools/tests/fsm_integration_tests.rs`:

```rust
/// Test scenario: Built-in tool handling via helper FSMs
#[tokio::test]
async fn test_builtin_tool_handling() {
    // Setup: Create FSM with helper FSMs registered
    // Execute: AI returns rhd_set_todo_list tool call
    // Expected: Helper FSM intercepts, sets propagate to false
    // Verify: Tool call is not sent to MCP
    // Verify: Todo list state is updated
    // Verify: Todo list message is injected
}

/// Test scenario: Role switching via helper FSM
#[tokio::test]
async fn test_role_switching() {
    // Setup: Create FSM with helper FSMs registered
    // Execute: AI returns rhd_set_role tool call
    // Expected: Helper FSM intercepts, sets propagate to false
    // Verify: Tool call is not sent to MCP
    // Verify: Role state is updated
    // Verify: Role prompt is injected
}
```

## Implementation Notes

1. **Event Interception**: Helper FSMs use the `propagate` flag in `ToolCallRequested` events to prevent tool calls from being executed by MCP clients.

2. **State Management**: Each helper FSM maintains its own state independently. The state is updated when the corresponding tool call is intercepted.

3. **Template Integration**: Helper FSMs can generate messages for injection into the conversation using templates.

4. **Backward Compatibility**: The existing `handle_rhd_set_todo_list` and `handle_rhd_set_role` functions remain for backward compatibility. The helper FSMs delegate to these functions for the actual DB operations.

5. **Listener Lifecycle**: Helper FSMs register themselves as listeners on the tool loop FSM. They can be unregistered when no longer needed.

6. **Thread Safety**: Helper FSMs use `Arc<Mutex<>>` for state management to ensure thread safety when used as listeners.

## Dependencies

- This phase depends on Phase 5 (Integration Tests)
- This phase is the final phase in the grand plan

## Success Criteria

- [ ] `TodoListFsm` fully manages todo list state and operations
- [ ] `RolesFsm` fully manages role switching and injection
- [ ] Built-in tools handled entirely by helper FSMs
- [ ] No built-in tool logic remains in async wrapper
- [ ] Unit tests for both helper FSMs pass
- [ ] Integration tests verify end-to-end functionality
- [ ] Code compiles without warnings
- [ ] Event interception works correctly
- [ ] State is properly synchronized
