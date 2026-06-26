use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
pub struct DaemonConfig {
    #[serde(default = "default_scenarios_dir")]
    pub scenarios_dir: PathBuf,
    #[serde(default = "default_models_dir")]
    pub models_dir: PathBuf,
    #[serde(default = "default_mcp_dir")]
    pub mcp_dir: PathBuf,
    #[serde(default)]
    pub default_model: Option<String>,
    #[serde(default)]
    pub logs: Option<PathBuf>,
    #[serde(default)]
    pub ws_port: Option<u16>,
    #[serde(default = "default_db_dir")]
    pub db_dir: PathBuf,
    #[serde(default)]
    pub credentials_config: Option<PathBuf>,
}

fn default_scenarios_dir() -> PathBuf {
    PathBuf::from("scenarios")
}

fn default_models_dir() -> PathBuf {
    PathBuf::from("models")
}

fn default_mcp_dir() -> PathBuf {
    PathBuf::from("mcp")
}

fn default_db_dir() -> PathBuf {
    PathBuf::from("rhd_db")
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            scenarios_dir: default_scenarios_dir(),
            models_dir: default_models_dir(),
            mcp_dir: default_mcp_dir(),
            default_model: None,
            logs: None,
            ws_port: None,
            db_dir: default_db_dir(),
            credentials_config: None,
        }
    }
}

pub fn load_config(path: &Path) -> Result<DaemonConfig, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read config '{}': {e}", path.display()))?;
    let mut config: DaemonConfig = serde_yaml::from_str(&content)
        .map_err(|e| format!("failed to parse config '{}': {e}", path.display()))?;
    
    if let Some(cred_path) = &config.credentials_config {
        if let Some(config_dir) = path.parent() {
            config.credentials_config = Some(config_dir.join(cred_path));
        }
    }
    
    Ok(config)
}
