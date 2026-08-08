use std::sync::Arc;

use super::event::ToolLoopFsmEvent;

/// Unique identifier for a registered listener
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ToolLoopListenerId(pub usize);

/// Type alias for listener callback function
pub type ToolLoopListenerCallback = Arc<dyn Fn(ToolLoopFsmEvent) + Send + Sync>;

/// Manages a collection of listeners for FSM events
pub struct ToolLoopListenerManager {
    listeners: Vec<(ToolLoopListenerId, ToolLoopListenerCallback)>,
    next_id: usize,
}

impl ToolLoopListenerManager {
    /// Create a new listener manager
    pub fn new() -> Self {
        Self {
            listeners: Vec::new(),
            next_id: 0,
        }
    }

    /// Register a new listener and return its ID
    pub fn add_listener(&mut self, callback: ToolLoopListenerCallback) -> ToolLoopListenerId {
        let id = ToolLoopListenerId(self.next_id);
        self.next_id += 1;
        self.listeners.push((id, callback));
        id
    }

    /// Remove a listener by its ID
    pub fn remove_listener(&mut self, id: ToolLoopListenerId) -> bool {
        let initial_len = self.listeners.len();
        self.listeners.retain(|(listener_id, _)| *listener_id != id);
        self.listeners.len() < initial_len
    }

    /// Emit an event to all registered listeners
    pub fn emit(&self, event: ToolLoopFsmEvent) {
        for (_, callback) in &self.listeners {
            callback(event.clone());
        }
    }

    /// Get the number of registered listeners
    pub fn listener_count(&self) -> usize {
        self.listeners.len()
    }
}

impl Default for ToolLoopListenerManager {
    fn default() -> Self {
        Self::new()
    }
}
