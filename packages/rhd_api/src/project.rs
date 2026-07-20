use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Role {
    pub name: String,
    pub system_prompt: String,
    pub when_to_use: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub name: String,
    pub path: PathBuf,
    pub mcp_configs: Vec<McpRef>,
    pub system_prompt: Option<String>,
    pub roles: Vec<Role>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpRef {
    pub name: String,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub args: Option<Vec<String>>,
    #[serde(default)]
    pub env: Option<HashMap<String, String>>,
}

impl McpRef {
    pub fn effective_id(&self) -> &str {
        self.id.as_deref().unwrap_or(&self.name)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInfo {
    pub name: String,
    pub has_mcp: bool,
    pub has_system_prompt: bool,
    pub has_roles: bool,
    pub role_names: Vec<String>,
}

impl From<&Project> for ProjectInfo {
    fn from(project: &Project) -> Self {
        Self {
            name: project.name.clone(),
            has_mcp: !project.mcp_configs.is_empty(),
            has_system_prompt: project.system_prompt.is_some(),
            has_roles: !project.roles.is_empty(),
            role_names: project.roles.iter().map(|r| r.name.clone()).collect(),
        }
    }
}
