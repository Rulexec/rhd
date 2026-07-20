use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use rhd_api::project::{McpRef, Project, ProjectInfo, Role};
use rhd_chat::{McpStatus, ProjectProvider};
use rhd_mcp_client::client::McpClient;
use rhd_mcp_client::McpConfig;
use tokio::sync::Mutex;
use tracing::{info, error};

use crate::mcp_cache::McpServerCache;


pub struct ProjectManager {
    projects: HashMap<String, Project>,
    mcp_configs: HashMap<String, McpConfig>,
    mcp_clients: Arc<Mutex<HashMap<String, Arc<McpClient>>>>,
    mcp_status: Arc<Mutex<HashMap<String, McpStatus>>>,
    mcp_cache: Arc<McpServerCache>,
}

impl ProjectManager {
    pub fn new(projects: Vec<Project>, mcp_configs: HashMap<String, McpConfig>, mcp_cache: Arc<McpServerCache>) -> Self {
        let mut project_map = HashMap::new();
        for project in projects {
            project_map.insert(project.name.clone(), project);
        }
        Self {
            projects: project_map,
            mcp_configs,
            mcp_clients: Arc::new(Mutex::new(HashMap::new())),
            mcp_status: Arc::new(Mutex::new(HashMap::new())),
            mcp_cache,
        }
    }

    pub fn list_projects(&self) -> Vec<ProjectInfo> {
        let mut infos: Vec<ProjectInfo> = self.projects.values().map(ProjectInfo::from).collect();
        infos.sort_by(|a, b| a.name.cmp(&b.name));
        infos
    }

    pub fn get_project(&self, name: &str) -> Option<&Project> {
        self.projects.get(name)
    }

    pub async fn get_mcp_status(&self, project_name: &str) -> Vec<(String, McpStatus)> {
        let status_map = self.mcp_status.lock().await;
        let prefix = format!("{}:", project_name);
        let mut result: Vec<(String, McpStatus)> = status_map
            .iter()
            .filter(|(key, _)| key.starts_with(&prefix))
            .map(|(key, status)| {
                let mcp_name = key.strip_prefix(&prefix).unwrap_or(key).to_string();
                (mcp_name, status.clone())
            })
            .collect();
        result.sort_by(|a, b| a.0.cmp(&b.0));
        result
    }

    #[tracing::instrument(level = "info", skip(self), fields(project_name = %project_name))]
    pub async fn spawn_project_mcp(
        &self,
        project_name: &str,
    ) -> Result<(), String> {
        let project = self
            .projects
            .get(project_name)
            .ok_or_else(|| format!("project not found: {}", project_name))?;

        for mcp_ref in &project.mcp_configs {
            let status_key = format!("{}:{}", project_name, mcp_ref.effective_id());

            {
                let mut status_map = self.mcp_status.lock().await;
                status_map.insert(status_key.clone(), McpStatus::Connecting);
            }

            let base_config = self.mcp_configs.get(&mcp_ref.name)
                .ok_or_else(|| format!("MCP config '{}' not found in mcp/ directory", mcp_ref.name))?;

            let mcp_config = McpConfig {
                name: base_config.name.clone(),
                cmd: base_config.cmd.clone(),
                args: mcp_ref.args.clone().unwrap_or_else(|| base_config.args.clone()),
                cwd: base_config.cwd.clone(),
                env: {
                    let mut env = base_config.env.clone();
                    if let Some(ref override_env) = mcp_ref.env {
                        env.extend(override_env.clone());
                    }
                    env
                },
            };

            let cmd_for_log = mcp_config.cmd.as_deref().unwrap_or("").to_string();
            let cwd_for_log = mcp_config.cwd.clone();
            let mcp_id = mcp_ref.effective_id().to_string();
            let mcp_name = mcp_ref.name.clone();

            match self.mcp_cache.get_or_spawn(&mcp_config).await {
                Ok(client) => {
                    let pid = client.pid().await;
                    info!(
                        mcp_ref = %mcp_ref.name,
                        mcp_id = %mcp_id,
                        mcp_name = %mcp_name,
                        cmd = %cmd_for_log,
                        ?cwd_for_log,
                        ?pid,
                        "MCP server spawned"
                    );
                    let mut clients = self.mcp_clients.lock().await;
                    clients.insert(status_key.clone(), client);
                    let mut status_map = self.mcp_status.lock().await;
                    status_map.insert(status_key, McpStatus::Connected);
                }
                Err(err) => {
                    let error_msg = err.to_string();
                    error!(
                        mcp_ref = %mcp_ref.name,
                        mcp_id = %mcp_id,
                        mcp_name = %mcp_name,
                        cmd = %cmd_for_log,
                        ?cwd_for_log,
                        error = %error_msg,
                        "MCP server spawn failed"
                    );
                    let mut status_map = self.mcp_status.lock().await;
                    status_map.insert(status_key, McpStatus::Failed(error_msg));
                }
            }
        }

        Ok(())
    }

    pub async fn get_mcp_clients(&self, project_name: &str) -> Vec<(String, Arc<McpClient>)> {
        let clients = self.mcp_clients.lock().await;
        let prefix = format!("{}:", project_name);
        let mut result: Vec<(String, Arc<McpClient>)> = clients
            .iter()
            .filter(|(key, _)| key.starts_with(&prefix))
            .map(|(key, client)| {
                let mcp_name = key.strip_prefix(&prefix).unwrap_or(key).to_string();
                (mcp_name, client.clone())
            })
            .collect();
        result.sort_by(|a, b| a.0.cmp(&b.0));
        result
    }
    pub fn get_project_roles(&self, project_name: &str) -> Vec<Role> {
        self.projects
            .get(project_name)
            .map(|p| p.roles.clone())
            .unwrap_or_default()
    }

    pub fn get_role_system_prompt(&self, project_name: &str, role_name: &str) -> Option<String> {
        self.projects
            .get(project_name)
            .and_then(|p| p.roles.iter().find(|r| r.name == role_name))
            .map(|r| r.system_prompt.clone())
    }
}

#[async_trait]
impl ProjectProvider for ProjectManager {
    async fn get_mcp_status(&self, project_name: &str) -> Vec<(String, McpStatus)> {
        ProjectManager::get_mcp_status(self, project_name).await
    }

    fn get_project_system_prompt(&self, project_name: &str) -> Option<String> {
        self.projects
            .get(project_name)
            .and_then(|p| p.system_prompt.clone())
    }

    fn get_project_mcp_refs(&self, project_name: &str) -> Vec<McpRef> {
        self.projects
            .get(project_name)
            .map(|p| p.mcp_configs.clone())
            .unwrap_or_default()
    }

    async fn get_mcp_clients(&self, project_name: &str) -> Vec<(String, Arc<McpClient>)> {
        ProjectManager::get_mcp_clients(self, project_name).await
    }

    async fn spawn_project_mcp(&self, project_name: &str) -> Result<(), String> {
        ProjectManager::spawn_project_mcp(self, project_name).await
    }

    fn get_project_roles(&self, project_name: &str) -> Vec<Role> {
        ProjectManager::get_project_roles(self, project_name)
    }

    fn get_role_system_prompt(&self, project_name: &str, role_name: &str) -> Option<String> {
        ProjectManager::get_role_system_prompt(self, project_name, role_name)
    }
}

#[cfg(test)]
mod tests;
