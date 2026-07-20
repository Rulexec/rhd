use rhd_api::project::{McpRef, Role};
use rhd_mcp_client::client::McpClient;
use crate::{McpStatus, ProjectProvider};
use std::fs;
use std::sync::Arc;

pub fn cleanup(path: &str) {
    let _ = fs::remove_file(path);
    let _ = fs::remove_file(format!("{}-wal", path));
    let _ = fs::remove_file(format!("{}-shm", path));
}

pub struct MockProjectProvider {
    pub roles: std::collections::HashMap<String, Vec<Role>>,
    pub role_prompts: std::collections::HashMap<(String, String), String>,
}

#[async_trait::async_trait]
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
