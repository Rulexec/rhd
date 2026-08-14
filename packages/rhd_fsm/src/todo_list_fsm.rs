use std::sync::atomic::Ordering;
use std::sync::Arc;

use crate::tool_loop_fsm::{ToolCall, ToolLoopFsmEvent, ToolLoopListenerId};

/// Represents the current state of the todo list
#[derive(Debug, Clone, PartialEq)]
pub enum TodoListState {
    /// No todo list has been set yet
    Empty,
    /// Todo list has been set with raw markdown content
    Active { todos: String },
}

/// FSM for managing todo list state and intercepting rhd_set_todo_list tool calls
pub struct TodoListFsm {
    state: TodoListState,
    listener_id: Option<ToolLoopListenerId>,
    intercepted_tool_calls: Arc<std::sync::Mutex<Vec<ToolCall>>>,
}

impl TodoListFsm {
    /// Create a new TodoListFsm in Empty state
    pub fn new() -> Self {
        Self {
            state: TodoListState::Empty,
            listener_id: None,
            intercepted_tool_calls: Arc::new(std::sync::Mutex::new(Vec::new())),
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

    /// Get todos string if active
    pub fn todos(&self) -> Option<&str> {
        match &self.state {
            TodoListState::Active { todos } => Some(todos),
            TodoListState::Empty => None,
        }
    }

    /// Create a listener callback for this FSM
    ///
    /// This callback intercepts `ToolCallRequested` events for `rhd_set_todo_list` tool
    /// and sets `propagate` to false to prevent the tool call from being executed by MCP.
    pub fn create_listener(&mut self) -> Arc<dyn Fn(ToolLoopFsmEvent) + Send + Sync> {
        let intercepted = self.intercepted_tool_calls.clone();

        Arc::new(move |event: ToolLoopFsmEvent| {
            if let ToolLoopFsmEvent::ToolCallRequested { tool_call, propagate } = event {
                if tool_call.name == "rhd_set_todo_list" {
                    // Track the intercepted tool call
                    let mut intercepted_guard = intercepted.lock().unwrap();
                    intercepted_guard.push(tool_call.clone());

                    // Prevent propagation to MCP
                    propagate.store(false, Ordering::SeqCst);
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

    /// Take and clear the list of intercepted tool calls
    pub fn take_intercepted_tool_calls(&mut self) -> Vec<ToolCall> {
        std::mem::take(&mut *self.intercepted_tool_calls.lock().unwrap())
    }

    /// Check if a tool call was intercepted by this FSM
    pub fn is_intercepted(&self, tool_call_id: &str) -> bool {
        self.intercepted_tool_calls.lock().unwrap().iter().any(|tc| tc.id == tool_call_id)
    }

    /// Handle the todo list update directly
    ///
    /// This method is called by the async wrapper when a `rhd_set_todo_list` tool call
    /// is intercepted. It parses the arguments and updates the state.
    pub fn handle_set_todo_list(&mut self, arguments: &str) -> Result<String, String> {
        let json: serde_json::Value = serde_json::from_str(arguments)
            .map_err(|e| format!("Failed to parse arguments: {}", e))?;

        let todos = json
            .get("todos")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing 'todos' field".to_string())?;

        self.state = TodoListState::Active {
            todos: todos.to_string(),
        };

        Ok(format!("Todo list updated successfully with {} items", todos.lines().count()))
    }

    /// Record an intercepted tool call (called by the listener)
    pub fn record_intercepted_tool_call(&mut self, tool_call: ToolCall) {
        self.intercepted_tool_calls.lock().unwrap().push(tool_call);
    }
}

impl Default for TodoListFsm {
    fn default() -> Self {
        Self::new()
    }
}

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
        let arguments = r#"{"todos": "[ ] Task 1\n[x] Task 2"}"#;

        let result = fsm.handle_set_todo_list(arguments).unwrap();

        assert!(!fsm.is_empty());
        assert_eq!(fsm.todos(), Some("[ ] Task 1\n[x] Task 2"));
        assert!(result.contains("2 items"));
    }

    #[test]
    fn test_handle_set_todo_list_missing_field() {
        let mut fsm = TodoListFsm::new();
        let arguments = r#"{"invalid": "data"}"#;

        let result = fsm.handle_set_todo_list(arguments);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing 'todos' field"));
    }

    #[test]
    fn test_intercepted_tool_calls() {
        let mut fsm = TodoListFsm::new();
        let tool_call = ToolCall {
            id: "call_1".to_string(),
            name: "rhd_set_todo_list".to_string(),
            arguments: r#"{"todos": "[ ] Task 1"}"#.to_string(),
        };

        fsm.record_intercepted_tool_call(tool_call.clone());

        assert!(fsm.is_intercepted("call_1"));
        assert!(!fsm.is_intercepted("call_2"));

        let intercepted = fsm.take_intercepted_tool_calls();
        assert_eq!(intercepted.len(), 1);
        assert_eq!(intercepted[0].id, "call_1");

        // After taking, should be empty
        assert!(!fsm.is_intercepted("call_1"));
    }
}
