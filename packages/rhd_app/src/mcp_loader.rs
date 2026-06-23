use std::collections::HashMap;
use std::path::Path;

use thiserror::Error;

use rhd_mcp_client::McpConfig;

#[derive(Debug, Error)]
pub enum McpLoadError {
    #[error("failed to read MCP config file '{path}': {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to parse MCP config YAML at {path}:{line}:{column}: {message}")]
    Parse {
        path: String,
        line: usize,
        column: usize,
        message: String,
    },
}

pub fn load_mcp_config(path: &Path) -> Result<McpConfig, McpLoadError> {
    let path_str = path.display().to_string();
    let contents = std::fs::read_to_string(path).map_err(|source| McpLoadError::Io {
        path: path_str.clone(),
        source,
    })?;

    let mut config: McpConfig = serde_yaml::from_str(&contents).map_err(|e| McpLoadError::Parse {
        path: path_str.clone(),
        line: e.location().map(|l| l.line()).unwrap_or(0),
        column: e.location().map(|l| l.column()).unwrap_or(0),
        message: e.to_string(),
    })?;
    
    // Apply environment variable substitution
    config.cmd = config.cmd.map(|c| rhd_util::substitute_env_vars(&c));
    config.args = config.args.iter().map(|a| rhd_util::substitute_env_vars(a)).collect();
    config.cwd = config.cwd.map(|d| rhd_util::substitute_env_vars(&d));
    config.env = config.env.into_iter().map(|(k, v)| (k, rhd_util::substitute_env_vars(&v))).collect();
    
    Ok(config)
}

pub fn load_mcp_dir(dir: &Path) -> Result<HashMap<String, McpConfig>, McpLoadError> {
    let mut configs = HashMap::new();
    
    if !dir.exists() {
        return Ok(configs);
    }
    
    let entries = dir.read_dir().map_err(|source| McpLoadError::Io {
        path: dir.display().to_string(),
        source,
    })?;
    
    for entry in entries {
        let entry = entry.map_err(|source| McpLoadError::Io {
            path: dir.display().to_string(),
            source,
        })?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let config_file = path.join("mcp.yaml");
        if !config_file.exists() {
            continue;
        }
        let config = load_mcp_config(&config_file)?;
        let dir_name = path.file_name()
            .expect("directory must have a name")
            .to_string_lossy()
            .into_owned();
        configs.insert(dir_name, config);
    }
    Ok(configs)
}
