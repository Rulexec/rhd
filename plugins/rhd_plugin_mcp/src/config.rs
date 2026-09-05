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

/// Top-level shape of the (shared) config file. Unknown top-level keys —
/// sections belonging to other plugins or the launcher — are ignored;
/// strictness is enforced per server entry via [`McpServerEntry`].
#[derive(Debug, Deserialize)]
struct RawConfig {
    mcp: Vec<McpServerEntry>,
}

/// Load, resolve, and validate the config file.
///
/// Steps:
/// 1. Read + parse YAML (unknown top-level keys ignored; unknown fields
///    inside an `mcp:` entry are rejected).
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_config(body: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        write!(f, "{}", body).unwrap();
        f
    }

    #[test]
    fn parses_full_example_with_defaults() {
        std::env::set_var("MCP_TEST_ROOT_1", "/tmp/available");
        let f = write_config(
            r#"
mcp:
  - id: fs1
    name: filesystem
    cmd: npx
    args:
      - '-y'
      - '@modelcontextprotocol/server-filesystem'
      - env: MCP_TEST_ROOT_1
    cwd: /work/dir
    env:
      SOME_VAR: some-value
    registerOnTag: 'mcp:common'
  - name: search
    cmd: search-cmd
"#,
        );
        let cfg = load_config(f.path().to_str().unwrap()).unwrap();
        assert_eq!(cfg.servers.len(), 2);

        let fs = &cfg.servers[0];
        assert_eq!(fs.id, "fs1");
        assert_eq!(fs.name, "filesystem");
        assert_eq!(
            fs.args,
            vec!["-y", "@modelcontextprotocol/server-filesystem", "/tmp/available"]
        );
        assert_eq!(fs.cwd.as_deref(), Some("/work/dir"));
        assert_eq!(
            fs.env.get("SOME_VAR").map(String::as_str),
            Some("some-value")
        );
        assert_eq!(fs.register_on_tag.as_deref(), Some("mcp:common"));

        // id defaults to name; args/cwd/env/registerOnTag optional
        let s = &cfg.servers[1];
        assert_eq!(s.id, "search");
        assert!(s.args.is_empty());
        assert!(s.cwd.is_none());
        assert!(s.env.is_empty());
        assert!(s.register_on_tag.is_none());
    }

    #[test]
    fn missing_env_var_fails_startup() {
        let f = write_config(
            r#"
mcp:
  - name: fs
    cmd: npx
    args:
      - env: MCP_TEST_DEFINITELY_UNSET_VAR
"#,
        );
        let err = load_config(f.path().to_str().unwrap()).unwrap_err();
        assert!(matches!(err, ConfigError::EnvVarMissing { .. }));
        assert!(err.to_string().contains("MCP_TEST_DEFINITELY_UNSET_VAR"));
    }

    #[test]
    fn rejects_duplicate_names() {
        let f = write_config(
            r#"
mcp:
  - name: fs
    cmd: a
  - name: fs
    cmd: b
"#,
        );
        assert!(matches!(
            load_config(f.path().to_str().unwrap()).unwrap_err(),
            ConfigError::DuplicateName(_)
        ));
    }

    #[test]
    fn rejects_duplicate_ids() {
        let f = write_config(
            r#"
mcp:
  - id: x
    name: a
    cmd: c
  - id: x
    name: b
    cmd: c
"#,
        );
        assert!(matches!(
            load_config(f.path().to_str().unwrap()).unwrap_err(),
            ConfigError::DuplicateId(_)
        ));
    }

    #[test]
    fn rejects_unknown_fields() {
        let f = write_config(
            r#"
mcp:
  - name: fs
    cmd: a
    bogus: 1
"#,
        );
        assert!(matches!(
            load_config(f.path().to_str().unwrap()).unwrap_err(),
            ConfigError::Parse(_)
        ));
    }

    #[test]
    fn rejects_empty_server_list() {
        let f = write_config("mcp: []\n");
        assert!(matches!(
            load_config(f.path().to_str().unwrap()).unwrap_err(),
            ConfigError::Validation(_)
        ));
    }

    #[test]
    fn ignores_unknown_top_level_keys() {
        // The shared rhd.yaml carries sections for other plugins/tools;
        // only `mcp:` is ours, the rest must be skipped.
        let f = write_config(
            r#"
credentialsConfig: ../credentials.yaml
ai_completions:
  models:
    default:
      alias: qwen
systemPrompts:
  warhammer: ./systemPrompts/warhammer.md
children:
  - name: chat_server
    cmd: rhd_chat_server
mcp:
  - name: fs
    cmd: npx
"#,
        );
        let cfg = load_config(f.path().to_str().unwrap()).unwrap();
        assert_eq!(cfg.servers.len(), 1);
        assert_eq!(cfg.servers[0].name, "fs");
    }
}
