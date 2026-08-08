# Phase 4: Replace tool_loop

## Overview

This phase replaces the existing monolithic `tool_loop()` function with the FSM-based implementation. The new implementation uses `FsmToolLoop` wrapper with DB sync listener.

**Scope:**
- Replace `tool_loop()` body with call to `FsmToolLoop`
- Ensure all existing behavior is preserved:
  - Streaming events
  - Logging
  - Error handling
  - Pause/resume
  - Abort
- Remove old implementation code
- Keep function signature compatible

**Out of Scope:**
- Helper FSMs for built-in tools (Phase 6)
- Changes to function signature (keep compatible)

## Files to Modify

### 1. `packages/rhd_chat/src/tools/tool_loop.rs`

**Modifications:**

Replace the entire `tool_loop()` function body with a call to `FsmToolLoop`:

```rust
use std::sync::Arc;

use rhd_ai::client::{OpenAiClient, ToolCall};
use rhd_ai::ToolDefinition;
use rhd_db::ChatDb;
use rhd_mcp_client::client::McpClient;
use rhd_mcp_client::{McpClientTrait, ToolResult};
use tokio::sync::{broadcast, Notify};
use tokio_util::sync::CancellationToken;

use crate::chat_log::ChatLoggers;
use crate::error::ChatError;
use crate::event::ChatEvent;
use crate::manager::ChatManager;
use crate::stream::TemplateLoaderRef;
use crate::ProjectProvider;

use super::builtin::{
    RHD_SET_ROLE_TOOL_NAME, RHD_SET_TODO_LIST_TOOL_NAME, handle_rhd_set_role,
    handle_rhd_set_todo_list, inject_todo_list_message,
};
use super::db_sync_listener::create_db_sync_listener;
use super::fsm_wrapper::FsmToolLoop;
use super::messages::build_chat_messages_for_tools;
use super::utils::{extract_mcp_id_from_tool_name, split_tool_name};

#[derive(Debug)]
pub enum ToolLoopResult {
    Completed { message_id: i64 },
    Paused,
}

pub async fn collect_tools_from_projects<P: ProjectProvider>(
    db: &Arc<ChatDb>,
    project_provider: &Arc<P>,
    chat_id: i64,
    template_loader: &TemplateLoaderRef,
) -> (Vec<ToolDefinition>, Vec<(String, String, Arc<McpClient>)>) {
    let mut tools = Vec::new();
    let mut mcp_clients = Vec::new();

    tools.extend(super::builtin::collect_builtin_tools(db, project_provider, chat_id, template_loader));

    let attached_projects = match db.get_chat_projects(chat_id) {
        Ok(projects) => projects,
        Err(_) => return (tools, mcp_clients),
    };

    for (project_name, _) in attached_projects {
        let clients = project_provider.get_mcp_clients(&project_name).await;
        for (mcp_id, client) in clients {
            if let Ok(client_tools) = client.list_tools().await {
                for tool in client_tools {
                    let prefixed_name = format!("{}/{}", mcp_id, tool.name);
                    tools.push(ToolDefinition {
                        tool_type: "function".to_string(),
                        function: rhd_ai::FunctionDefinition {
                            name: prefixed_name,
                            description: tool.description,
                            parameters: tool.input_schema,
                        },
                    });
                }
                mcp_clients.push((project_name.clone(), mcp_id, client));
            }
        }
    }

    (tools, mcp_clients)
}

/// Main tool loop function - now implemented using FSM
pub async fn tool_loop<P: ProjectProvider>(
    manager: &ChatManager<P>,
    chat_id: i64,
    model: &str,
    api_model: &str,
    client: &OpenAiClient,
    tools: &[ToolDefinition],
    mcp_clients: &[(String, String, Arc<McpClient>)],
    cancel_token: &CancellationToken,
    event_sender: &broadcast::Sender<ChatEvent>,
    iterations: &mut u32,
    current_content: &mut String,
    max_iterations: u32,
    loggers: Option<ChatLoggers>,
    template_loader: &crate::stream::TemplateLoaderRef,
) -> Result<ToolLoopResult, ChatError> {
    // Create pause notification channel
    let pause_notify = Arc::new(Notify::new());
    
    // Create DB sync listener
    let db_sync_listener = create_db_sync_listener(
        manager.db().clone(),
        chat_id,
        event_sender.clone(),
    );

    // Create FSM wrapper
    let mut fsm_loop = FsmToolLoop::new(
        manager,
        chat_id,
        model.to_string(),
        api_model.to_string(),
        client,
        mcp_clients.to_vec(),
        cancel_token.clone(),
        event_sender.clone(),
        pause_notify.clone(),
        max_iterations,
        loggers,
        template_loader,
    );

    // Register DB sync listener with FSM
    fsm_loop.add_listener(db_sync_listener);

    // Run the FSM loop
    let result = fsm_loop.run().await;

    // Update iterations counter from FSM state
    *iterations = fsm_loop.iterations();
    
    // Update current_content if needed
    if let Ok(ToolLoopResult::Completed { .. }) = &result {
        // Content is already handled by the wrapper
    }

    result
}

/// Execute a tool call - kept for backward compatibility and used by FSM wrapper
pub async fn execute_tool_call<P: ProjectProvider>(
    manager: &ChatManager<P>,
    chat_id: i64,
    tool_call: &ToolCall,
    mcp_clients: &[(String, String, Arc<McpClient>)],
    event_sender: &broadcast::Sender<ChatEvent>,
) -> (ToolResult, String) {
    let tool_name = &tool_call.function.name;

    if tool_name == RHD_SET_TODO_LIST_TOOL_NAME {
        let result = handle_rhd_set_todo_list(manager, chat_id, &tool_call.function.arguments, event_sender).await;
        return (result, String::new());
    }

    if tool_name == RHD_SET_ROLE_TOOL_NAME {
        let result = handle_rhd_set_role(manager, chat_id, &tool_call.function.arguments, event_sender).await;
        return (result, String::new());
    }

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

### 2. `packages/rhd_chat/src/tools/fsm_wrapper.rs`

**Modifications:**

Add methods to expose FSM state and allow listener registration:

```rust
impl<'a, P: ProjectProvider> FsmToolLoop<'a, P> {
    /// Register a listener for FSM events
    pub fn add_listener(&mut self, callback: rhd_fsm::ToolLoopListenerCallback) {
        self.fsm.add_listener(callback);
    }

    /// Get the current iteration count
    pub fn iterations(&self) -> u32 {
        self.iterations
    }
}
```

### 3. `packages/rhd_chat/src/stream/send/tools.rs`

**Modifications:**

Update caller if needed to handle the new `tool_loop()` signature. The signature should remain compatible, so minimal changes are expected.

Check if `pause_notify` needs to be passed from the caller. If so, update the function signature:

```rust
pub async fn send_message_with_tools<P: ProjectProvider>(
    manager: &ChatManager<P>,
    chat_id: i64,
    model: &str,
    api_model: &str,
    client: &OpenAiClient,
    tools: &[ToolDefinition],
    mcp_clients: &[(String, String, Arc<McpClient>)],
    cancel_token: &CancellationToken,
    event_sender: &broadcast::Sender<ChatEvent>,
    pause_notify: Arc<Notify>,  // NEW: Pass pause notification channel
    max_iterations: u32,
    loggers: Option<ChatLoggers>,
    template_loader: &crate::stream::TemplateLoaderRef,
) -> Result<ToolLoopResult, ChatError> {
    let mut iterations = 0;
    let mut current_content = String::new();
    
    tool_loop(
        manager,
        chat_id,
        model,
        api_model,
        client,
        tools,
        mcp_clients,
        cancel_token,
        event_sender,
        &mut iterations,
        &mut current_content,
        max_iterations,
        loggers,
        template_loader,
    )
    .await
}
```

**Note:** The actual changes to `tools.rs` depend on how pause/resume is currently implemented. If `pause_notify` is already available in the caller, it should be passed to `tool_loop()`.

## Tests

### Integration Tests

The existing tests in `packages/rhd_chat/src/tools/tests/` should continue to pass. Verify:

1. `message_tests.rs` - Message handling tests
2. `role_tests.rs` - Role switching tests
3. `todo_list.rs` - Todo list tests
4. `tool_parsing.rs` - Tool parsing tests

Run all tests:

```bash
cd packages/rhd_chat
cargo test tools::tests
```

## Implementation Notes

1. **Backward Compatibility**: The `tool_loop()` function signature remains the same. The `iterations` and `current_content` parameters are still mutable references, but they are now updated by the FSM wrapper.

2. **Pause/Resume Integration**: The `pause_notify` channel is created in `tool_loop()` and passed to the FSM wrapper. The caller can trigger resume by calling `pause_notify.notify_one()`.

3. **DB Sync**: The DB sync listener is automatically registered with the FSM. All message mutations are synchronized to the database.

4. **Built-in Tools**: For now, built-in tools (`rhd_set_todo_list`, `rhd_set_role`) are still handled by the existing `execute_tool_call()` function. Phase 6 will move this to helper FSMs.

5. **Error Handling**: Errors from the FSM wrapper are propagated up as `ChatError`. The error types remain the same.

6. **Logging**: Logging is handled by the FSM wrapper based on FSM actions. The loggers are passed to the wrapper during initialization.

7. **Streaming Events**: Streaming events (`StreamChunk`, `ThinkingChunk`, etc.) are emitted by the FSM wrapper during AI calls.

## Dependencies

- This phase depends on Phase 2 (Async Wrapper) and Phase 3 (DB Sync Listener)
- This phase must be completed before Phase 5 (Integration Tests)

## Success Criteria

- [ ] All existing tests pass
- [ ] Behavior identical to old implementation
- [ ] Code is simpler and more maintainable
- [ ] No regressions in pause/resume/abort
- [ ] Streaming events work correctly
- [ ] Logging works correctly
- [ ] DB stays in sync
- [ ] Code compiles without warnings
