use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use rhd_api::project::{McpRef, Project, ProjectInfo};
use rhd_chat::{McpStatus, ProjectProvider};
use rhd_mcp_client::client::McpClient;
use rhd_mcp_client::McpConfig;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::mcp_cache::McpServerCache;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub enum McpStatusDto {
    Connecting,
    Connected,
    Failed { error: String },
}

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
                    eprintln!("MCP '{}' spawned (id: '{}', name: '{}', cmd: '{}', cwd: {:?}, PID: {:?})", 
                        mcp_ref.name, mcp_id, mcp_name, cmd_for_log, cwd_for_log, pid);
                    let mut clients = self.mcp_clients.lock().await;
                    clients.insert(status_key.clone(), client);
                    let mut status_map = self.mcp_status.lock().await;
                    status_map.insert(status_key, McpStatus::Connected);
                }
                Err(err) => {
                    let error_msg = err.to_string();
                    eprintln!("MCP '{}' spawn failed (id: '{}', name: '{}', cmd: '{}', cwd: {:?}): {}", 
                        mcp_ref.name, mcp_id, mcp_name, cmd_for_log, cwd_for_log, error_msg);
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use rhd_api::project::McpRef;
    use std::path::PathBuf;

    fn make_project(name: &str, mcp_count: usize) -> Project {
        let mcp_configs: Vec<McpRef> = (0..mcp_count)
            .map(|i| McpRef {
                name: format!("mcp-{}", i),
                id: None,
                args: None,
                env: None,
            })
            .collect();
        Project {
            name: name.to_string(),
            path: PathBuf::from(format!("/tmp/{}", name)),
            mcp_configs,
            system_prompt: Some("test prompt".to_string()),
        }
    }

    fn make_manager(projects: Vec<Project>) -> ProjectManager {
        ProjectManager::new(projects, HashMap::new(), Arc::new(McpServerCache::new()))
    }

    #[test]
    fn test_list_projects_empty() {
        let manager = make_manager(vec![]);
        assert!(manager.list_projects().is_empty());
    }

    #[test]
    fn test_list_projects_sorted() {
        let manager = make_manager(vec![
            make_project("charlie", 0),
            make_project("alpha", 0),
            make_project("bravo", 0),
        ]);
        let infos = manager.list_projects();
        assert_eq!(infos.len(), 3);
        assert_eq!(infos[0].name, "alpha");
        assert_eq!(infos[1].name, "bravo");
        assert_eq!(infos[2].name, "charlie");
    }

    #[test]
    fn test_get_project_found() {
        let manager = make_manager(vec![make_project("test", 1)]);
        let project = manager.get_project("test");
        assert!(project.is_some());
        assert_eq!(project.unwrap().name, "test");
    }

    #[test]
    fn test_get_project_not_found() {
        let manager = make_manager(vec![make_project("test", 1)]);
        assert!(manager.get_project("nonexistent").is_none());
    }

    #[tokio::test]
    async fn test_get_mcp_status_empty() {
        let manager = make_manager(vec![make_project("test", 0)]);
        let status = manager.get_mcp_status("test").await;
        assert!(status.is_empty());
    }

    #[tokio::test]
    async fn test_get_mcp_clients_empty() {
        let manager = make_manager(vec![make_project("test", 0)]);
        let clients = manager.get_mcp_clients("test").await;
        assert!(clients.is_empty());
    }

    #[test]
    fn test_project_info_has_mcp() {
        let manager = make_manager(vec![
            make_project("with-mcp", 2),
            make_project("without-mcp", 0),
        ]);
        let infos = manager.list_projects();
        let with_mcp = infos.iter().find(|i| i.name == "with-mcp").unwrap();
        let without_mcp = infos.iter().find(|i| i.name == "without-mcp").unwrap();
        assert!(with_mcp.has_mcp);
        assert!(!without_mcp.has_mcp);
    }

    #[test]
    fn test_project_info_has_system_prompt() {
        let mut project = make_project("test", 0);
        project.system_prompt = Some("prompt".to_string());
        let manager = make_manager(vec![project]);
        let info = &manager.list_projects()[0];
        assert!(info.has_system_prompt);
    }

    #[test]
    fn test_mcp_status_is_connected() {
        assert!(McpStatus::Connected.is_connected());
        assert!(!McpStatus::Connecting.is_connected());
        assert!(!McpStatus::Failed("err".to_string()).is_connected());
    }
}
