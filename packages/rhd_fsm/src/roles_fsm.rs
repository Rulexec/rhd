use std::sync::atomic::Ordering;
use std::sync::Arc;

use crate::tool_loop_fsm::{ToolCall, ToolLoopFsmEvent, ToolLoopListenerId};

/// Represents the current role state
#[derive(Debug, Clone, PartialEq)]
pub enum RoleState {
    /// No role is active
    None,
    /// A role is active
    Active { role_name: String },
}

/// FSM for managing role state and intercepting rhd_set_role tool calls
pub struct RolesFsm {
    state: RoleState,
    listener_id: Option<ToolLoopListenerId>,
    intercepted_tool_calls: Vec<ToolCall>,
}

impl RolesFsm {
    /// Create a new RolesFsm with no active role
    pub fn new() -> Self {
        Self {
            state: RoleState::None,
            listener_id: None,
            intercepted_tool_calls: Vec::new(),
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
        let intercepted = Arc::new(std::sync::Mutex::new(Vec::new()));
        let intercepted_clone = intercepted.clone();

        Arc::new(move |event: ToolLoopFsmEvent| {
            if let ToolLoopFsmEvent::ToolCallRequested { tool_call, propagate } = event {
                if tool_call.name == "rhd_set_role" {
                    // Track the intercepted tool call
                    let mut intercepted_guard = intercepted_clone.lock().unwrap();
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
        std::mem::take(&mut self.intercepted_tool_calls)
    }

    /// Check if a tool call was intercepted by this FSM
    pub fn is_intercepted(&self, tool_call_id: &str) -> bool {
        self.intercepted_tool_calls.iter().any(|tc| tc.id == tool_call_id)
    }

    /// Handle the role switch directly
    ///
    /// This method is called by the async wrapper when a `rhd_set_role` tool call
    /// is intercepted. It parses the arguments and updates the state.
    pub fn handle_set_role(&mut self, arguments: &str) -> Result<String, String> {
        let json: serde_json::Value = serde_json::from_str(arguments)
            .map_err(|e| format!("Failed to parse arguments: {}", e))?;

        let role_name = json
            .get("role_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Missing 'role_name' field".to_string())?;

        self.state = RoleState::Active {
            role_name: role_name.to_string(),
        };

        Ok(format!("Successfully switched to role '{}'", role_name))
    }

    /// Record an intercepted tool call (called by the listener)
    pub fn record_intercepted_tool_call(&mut self, tool_call: ToolCall) {
        self.intercepted_tool_calls.push(tool_call);
    }
}

impl Default for RolesFsm {
    fn default() -> Self {
        Self::new()
    }
}

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
        let arguments = r#"{"role_name": "developer"}"#;

        let result = fsm.handle_set_role(arguments).unwrap();

        assert!(fsm.has_active_role());
        assert_eq!(fsm.active_role(), Some("developer"));
        assert!(result.contains("developer"));
    }

    #[test]
    fn test_handle_set_role_missing_field() {
        let mut fsm = RolesFsm::new();
        let arguments = r#"{"invalid": "data"}"#;

        let result = fsm.handle_set_role(arguments);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing 'role_name' field"));
    }

    #[test]
    fn test_intercepted_tool_calls() {
        let mut fsm = RolesFsm::new();
        let tool_call = ToolCall {
            id: "call_1".to_string(),
            name: "rhd_set_role".to_string(),
            arguments: r#"{"role_name": "reviewer"}"#.to_string(),
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
