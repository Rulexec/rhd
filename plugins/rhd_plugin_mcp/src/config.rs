//! Configuration loading and validation for the MCP plugin.

use serde::Deserialize;
use std::collections::{HashMap, HashSet};

/// One item in an MCP server's `args` list: either a literal string or an
/// `env: VAR` mapping resolved from the plugin's environment at startup.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum ArgValue {
    Literal(String),
    Env { env: String },
}

/// Raw MCP server entry as written in the YAML config.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpServerEntry {
    /// Internal identity; defaults to `name` when omitted.
    #[serde(default)]
    pub id: Option<String>,
    /// Tool-name prefix: tools are registered as `{name}:{tool}`.
    pub name: String,
    /// Executable to spawn.
    pub cmd: String,
    /// Arguments; items may be literals or `env: VAR` references.
    #[serde(default)]
    pub args: Vec<ArgValue>,
    /// Working directory for the server process. None → inherit the plugin's cwd.
    #[serde(default)]
    pub cwd: Option<String>,
    /// Extra environment variables injected into the server process.
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Register this server's tools only on chats carrying this exact tag.
    #[serde(default)]
    pub register_on_tag: Option<String>,
}

/// A validated server: `id` defaulted, `env:` args resolved to literals.
#[derive(Debug, Clone)]
pub struct ResolvedServer {
    pub id: String,
    pub name: String,
    pub cmd: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub env: HashMap<String, String>,
    pub register_on_tag: Option<String>,
}

/// Fully validated plugin configuration.
#[derive(Debug, Clone)]
pub struct PluginConfig {
    pub servers: Vec<ResolvedServer>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    mcp: Vec<McpServerEntry>,
}

/// Load, resolve, and validate the config file.
///
/// Steps:
/// 1. Read + parse YAML (strict: unknown fields rejected).
/// 2. Require at least one server.
/// 3. Default `id` to `name`.
/// 4. Resolve `env: VAR` args against the plugin's environment (missing → error).
/// 5. Reject duplicate `name`s (prefix collisions) and duplicate `id`s.
pub fn load_config(path: &str) -> Result<PluginConfig, ConfigError> {
    let content = std::fs::read_to_string(path).map_err(|e| ConfigError::Read {
        path: path.to_string(),
        details: e.to_string(),
    })?;

    let raw: RawConfig =
        serde_yaml::from_str(&content).map_err(|e| ConfigError::Parse(e.to_string()))?;

    if raw.mcp.is_empty() {
        return Err(ConfigError::Validation(
            "config defines no MCP servers under `mcp:`".to_string(),
        ));
    }

    let mut servers = Vec::with_capacity(raw.mcp.len());
    let mut seen_names: HashSet<String> = HashSet::new();
    let mut seen_ids: HashSet<String> = HashSet::new();

    for entry in raw.mcp {
        let id = entry.id.clone().unwrap_or_else(|| entry.name.clone());

        if !seen_names.insert(entry.name.clone()) {
            return Err(ConfigError::DuplicateName(entry.name));
        }
        if !seen_ids.insert(id.clone()) {
            return Err(ConfigError::DuplicateId(id));
        }

        let mut args = Vec::with_capacity(entry.args.len());
        for arg in &entry.args {
            match arg {
                ArgValue::Literal(s) => args.push(s.clone()),
                ArgValue::Env { env } => {
                    let value = std::env::var(env).map_err(|_| ConfigError::EnvVarMissing {
                        server: entry.name.clone(),
                        var: env.clone(),
                    })?;
                    args.push(value);
                }
            }
        }

        servers.push(ResolvedServer {
            id,
            name: entry.name,
            cmd: entry.cmd,
            args,
            cwd: entry.cwd,
            env: entry.env,
            register_on_tag: entry.register_on_tag,
        });
    }

    Ok(PluginConfig { servers })
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read config file '{path}': {details}")]
    Read { path: String, details: String },
    #[error("failed to parse config: {0}")]
    Parse(String),
    #[error("environment variable '{var}' is not set (required by server '{server}')")]
    EnvVarMissing { server: String, var: String },
    #[error("duplicate MCP server name '{0}' (tool prefixes would collide)")]
    DuplicateName(String),
    #[error("duplicate MCP server id '{0}'")]
    DuplicateId(String),
    #[error("config validation error: {0}")]
    Validation(String),
}
