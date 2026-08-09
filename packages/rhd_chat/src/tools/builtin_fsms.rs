use std::sync::Arc;

use rhd_ai::client::ToolCall;
use rhd_fsm::{RolesFsm, TodoListFsm, ToolLoopFsm};
use rhd_mcp_client::ToolResult;
use tokio::sync::broadcast;

use crate::error::ChatError;
use crate::event::ChatEvent;
use crate::manager::ChatManager;
use crate::ProjectProvider;

use super::builtin::{
    RHD_SET_ROLE_TOOL_NAME, RHD_SET_TODO_LIST_TOOL_NAME, handle_rhd_set_role,
    handle_rhd_set_todo_list,
};

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

    /// Check if a tool call was intercepted by any helper FSM
    pub fn is_tool_intercepted(&self, tool_call_id: &str) -> bool {
        self.todo_list_fsm.is_intercepted(tool_call_id) || self.roles_fsm.is_intercepted(tool_call_id)
    }

    /// Handle a built-in tool call
    ///
    /// This method is called by the async wrapper when a built-in tool call
    /// is intercepted. It delegates to the appropriate helper FSM and then
    /// calls the actual handler function.
    pub async fn handle_builtin_tool<P: ProjectProvider>(
        &mut self,
        manager: &ChatManager<P>,
        chat_id: i64,
        tool_call: &ToolCall,
        event_sender: &broadcast::Sender<ChatEvent>,
    ) -> Result<ToolResult, ChatError> {
        let tool_name = &tool_call.function.name;
        let arguments = &tool_call.function.arguments;

        match tool_name.as_str() {
            RHD_SET_TODO_LIST_TOOL_NAME => {
                // Update FSM state
                self.todo_list_fsm
                    .handle_set_todo_list(arguments)
                    .map_err(|e| ChatError::Internal(e))?;

                // Call the actual handler
                let result =
                    handle_rhd_set_todo_list(manager, chat_id, arguments, event_sender).await;
                Ok(result)
            }
            RHD_SET_ROLE_TOOL_NAME => {
                // Update FSM state
                self.roles_fsm
                    .handle_set_role(arguments)
                    .map_err(|e| ChatError::Internal(e))?;

                // Call the actual handler
                let result = handle_rhd_set_role(manager, chat_id, arguments, event_sender).await;
                Ok(result)
            }
            _ => Err(ChatError::Internal(format!(
                "Unknown built-in tool: {}",
                tool_name
            ))),
        }
    }

    /// Get the current todo list state
    pub fn todo_list_state(&self) -> &rhd_fsm::TodoListState {
        self.todo_list_fsm.state()
    }

    /// Get the current role state
    pub fn role_state(&self) -> &rhd_fsm::RoleState {
        self.roles_fsm.state()
    }
}

impl Default for BuiltinFsmManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_fsm_manager_creation() {
        let manager = BuiltinFsmManager::new();
        assert!(manager.todo_list_fsm.is_empty());
        assert!(!manager.roles_fsm.has_active_role());
    }

    #[test]
    fn test_is_tool_intercepted() {
        let mut manager = BuiltinFsmManager::new();

        // Record an intercepted tool call
        let tool_call = ToolCall {
            id: "call_1".to_string(),
            call_type: "function".to_string(),
            function: rhd_ai::client::FunctionCall {
                name: RHD_SET_TODO_LIST_TOOL_NAME.to_string(),
                arguments: r#"{"todos": "[ ] Task 1"}"#.to_string(),
            },
        };
        // Convert to rhd_fsm::ToolCall for recording
        let fsm_tool_call = rhd_fsm::ToolCall {
            id: tool_call.id.clone(),
            name: tool_call.function.name.clone(),
            arguments: tool_call.function.arguments.clone(),
        };
        manager.todo_list_fsm.record_intercepted_tool_call(fsm_tool_call);

        assert!(manager.is_tool_intercepted("call_1"));
        assert!(!manager.is_tool_intercepted("call_2"));
    }
}
