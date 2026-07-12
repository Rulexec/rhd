pub mod error;
pub mod event;
pub mod state;
pub mod manager;
pub mod stream;
pub mod tools;
pub mod projects;
pub mod chat_log;

pub use error::ChatError;
pub use event::ChatEvent;
pub use manager::ChatManager;
pub use state::StreamState;

use std::sync::Arc;
use async_trait::async_trait;
use rhd_api::project::{McpRef, Role};
use rhd_mcp_client::client::McpClient;

#[derive(Debug, Clone)]
pub enum McpStatus {
    Connecting,
    Connected,
    Failed(String),
}

impl McpStatus {
    pub fn is_connected(&self) -> bool {
        matches!(self, McpStatus::Connected)
    }
}

#[async_trait]
pub trait ProjectProvider: Send + Sync {
    async fn get_mcp_status(&self, project_name: &str) -> Vec<(String, McpStatus)>;
    fn get_project_system_prompt(&self, project_name: &str) -> Option<String>;
    fn get_project_mcp_refs(&self, project_name: &str) -> Vec<McpRef>;
    async fn get_mcp_clients(&self, project_name: &str) -> Vec<(String, Arc<McpClient>)>;
    async fn spawn_project_mcp(&self, project_name: &str) -> Result<(), String>;
    
    /// Returns all roles defined in a project
    fn get_project_roles(&self, project_name: &str) -> Vec<Role>;
    
    /// Returns the system prompt for a specific role in a project
    fn get_role_system_prompt(&self, project_name: &str, role_name: &str) -> Option<String>;
}
