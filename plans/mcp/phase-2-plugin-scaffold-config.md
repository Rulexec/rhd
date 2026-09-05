# Phase 2: Scaffold `rhd_plugin_mcp` — crate, CLI args, config loading

## Overview

Create the new plugin crate `plugins/rhd_plugin_mcp` as a compiling binary that:
1. Parses CLI args: `--server-url`, `--plugin-id` (default `mcp`), `--worktree <workTreeId>`, `--config <configPath>`.
2. Loads and validates the YAML config: a list of MCP servers with `id` (optional, defaults to `name`), `name`, `cmd`, `args` (literals or `env: VAR` substitutions), `cwd` (optional, defaults to the plugin's working directory), `env` (optional map for the server process), `registerOnTag` (optional).

At the end of this phase the binary runs, prints parsed config, and exits — no chat-server interaction yet (Phase 4).

**Scope:**
- In: workspace membership, crate skeleton, `main.rs`, `config.rs`, stub `plugin.rs`, `README.md`.
- Out: MCP server spawning (Phase 3), lifecycle/gating/registration (Phase 4), tool execution (Phase 5), tests (Phase 6).

**Depends on:** nothing (parallel with Phase 1).

## Files to Create/Modify

### 1. `Cargo.toml` (workspace root)

**Modification:** add the member after `plugins/rhd_plugin_todo_list`:

```toml
members = [
    ...
    "plugins/rhd_plugin_todo_list",
    "plugins/rhd_plugin_mcp",
]
```

### 2. `plugins/rhd_plugin_mcp/Cargo.toml`

**Create** (mirrors `rhd_plugin_todo_list/Cargo.toml`):

```toml
[package]
name = "rhd_plugin_mcp"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "rhd_plugin_mcp"
path = "src/main.rs"

[dependencies]
rhd_chat_api = { path = "../../packages/rhd_chat_api" }
rhd_chat_client = { path = "../../packages/rhd_chat_client" }
rhd_mcp_client = { path = "../../packages/rhd_mcp_client" }
tokio = { workspace = true, features = ["full"] }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
serde_yaml = { workspace = true }
thiserror = { workspace = true }
clap = { workspace = true, features = ["derive"] }

[dev-dependencies]
tempfile = "3"
```

### 3. `plugins/rhd_plugin_mcp/src/lib.rs`

**Create:**

```rust
//! MCP servers plugin library.
//!
//! Exposes plugin internals for testing and reuse.

pub mod config;
pub mod plugin;
```

(Phases 3–5 append `pub mod gating;`, `pub mod mcp_pool;`, `pub mod tool_handler;`.)

### 4. `plugins/rhd_plugin_mcp/src/main.rs`

**Create:**

```rust
//! MCP servers plugin for RHD chat system.
//!
//! Spawns configured MCP servers, registers their tools on eligible chats
//! (prefixed `<name>:`), and executes tool calls, pushing results back.

use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod config;
mod plugin;

#[derive(Parser, Debug)]
#[command(name = "rhd_plugin_mcp")]
#[command(about = "MCP servers plugin for RHD chat system")]
struct Args {
    /// WebSocket URL of the chat server
    #[arg(long)]
    server_url: String,

    /// Plugin ID (defaults to "mcp")
    #[arg(long, default_value = "mcp")]
    plugin_id: String,

    /// Only serve chats tagged `worktree:<workTreeId>`. When omitted,
    /// serve only chats that carry no `worktree:*` tag.
    #[arg(long)]
    worktree: Option<String>,

    /// Path to configuration file
    #[arg(long)]
    config: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env().add_directive("rhd_plugin_mcp=info".parse()?))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args = Args::parse();

    tracing::info!("Starting rhd_plugin_mcp");
    tracing::info!("Server URL: {}", args.server_url);
    tracing::info!("Plugin ID: {}", args.plugin_id);
    tracing::info!("Worktree filter: {:?}", args.worktree);

    let config = config::load_config(&args.config)?;
    tracing::info!("Loaded {} MCP server(s)", config.servers.len());

    plugin::run_plugin(
        &args.server_url,
        &args.plugin_id,
        args.worktree.as_deref(),
        config,
    )
    .await?;

    Ok(())
}
```

### 5. `plugins/rhd_plugin_mcp/src/config.rs`

**Create** — full implementation:

```rust
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
```

### 6. `plugins/rhd_plugin_mcp/src/plugin.rs` (stub, replaced in Phase 4)

**Create:**

```rust
//! Core plugin lifecycle (implemented in Phase 4).

use crate::config::PluginConfig;

#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("config error: {0}")]
    Config(String),
}

pub async fn run_plugin(
    server_url: &str,
    plugin_id: &str,
    worktree: Option<&str>,
    config: PluginConfig,
) -> Result<(), PluginError> {
    tracing::info!(
        server_url,
        plugin_id,
        worktree = ?worktree,
        servers = config.servers.len(),
        "plugin lifecycle not yet implemented (Phase 4)"
    );
    Ok(())
}
```

### 7. `plugins/rhd_plugin_mcp/README.md`

**Create** per the `plugins/README.md` template. Required content:

```markdown
# MCP Plugin

## Overview
Bridges external MCP (Model Context Protocol) servers into RHD chats.
Spawns configured servers at startup, registers their tools on eligible
chats prefixed `<name>:`, executes tool calls, and pushes results back
as `tool`-role messages.

## CLI Usage
    rhd_plugin_mcp --server-url ws://127.0.0.1:8080/ --plugin-id mcp \
      [--worktree <workTreeId>] --config <configPath>

## Trigger Conditions (chat gating)
- With `--worktree W`: only chats tagged exactly `worktree:W`.
- Without `--worktree`: only chats carrying NO `worktree:*` tag.
- Per server: if `registerOnTag` is set, the server's tools register only
  on chats carrying that exact tag (AND with the worktree rule).
- Registration is additive: a chat that gains an eligible tag later gets the
  remaining servers' tools registered; tools are never unregistered.

## Configuration Format
```yaml
mcp:
  - id: fs1                      # optional, defaults to name
    name: filesystem             # tool prefix: "filesystem:"
    cmd: npx
    args:
      - '-y'
      - '@modelcontextprotocol/server-filesystem'
      - env: AVAILABLE_ROOT      # resolved from plugin environment at startup
    cwd: /abs/or/relative/path   # optional; default: plugin's working directory
    env:                         # optional; extra vars for the server process
      SOME_VAR: some-value
    registerOnTag: 'mcp:common'  # optional; exact chat tag gate
```
Missing `env:` variables fail startup with a clear error. Duplicate `name`
or `id` values are rejected.

## Events Emitted
None.

## Tags Added
None (reads chat tags; does not mutate them).

## Tags Consumed
- `worktree:<id>` — worktree gating (see Trigger Conditions).
- value of `registerOnTag` per server (e.g. `mcp:common`).

## Tool Call Handling
- Subscribes to `assistantMessageWithToolCalls` for its registered prefixed
  names; routes `{name}:{tool}` to the owning server; answers every call with
  a `tool`-role message carrying `toolCallId`.
- Duplicate guard: skips calls already answered (tool message with same id).
- MCP/transport errors are returned as tool content, not plugin crashes.

## Dependencies
- **rhd_chat_client / rhd_chat_api**: chat server interaction.
- **rhd_mcp_client**: MCP stdio JSON-RPC client.
- **rhd_plugin_ai_completions**: executes the tool loop; this plugin answers
  the tool calls it registered.

## Error Handling
- Startup: any server spawn/initialize failure aborts the plugin (fail-fast).
- Runtime: per-call errors surface as tool results; plugin keeps running.
- Caveat: the stdio transport assumes one response line per request; MCP
  servers that emit unsolicited notifications are not supported.
```

## Tests

Unit tests for `config.rs` are authored in Phase 6 (`plans/mcp/phase-6-tests.md`).

## Verification

```bash
cargo check -p rhd_plugin_mcp
cargo run -p rhd_plugin_mcp -- --help   # shows all four flags
```

## Implementation Notes

1. **`--server-url` naming:** matches existing plugins (`todo_list`, `system_prompt`, `ai_completions`); the user's "host" maps to this flag.
2. **`deny_unknown_fields` + `rename_all = "camelCase"`** per `memory/development.md` conventions (`registerOnTag` in YAML ↔ `register_on_tag` in Rust).
3. **`ArgValue` untagged enum** parses both plain strings and `- env: VAR` mappings in a `Vec`. YAML `- env: AVAILABLE_ROOT` deserializes into `Env { env: "AVAILABLE_ROOT" }`.
4. **`cwd: None` means inherit:** `McpClient::connect` passes `Option<&str>` to `Command::current_dir`; `None` leaves the child in the plugin's cwd — exactly the required default.
5. **Server-level `env` map values are literals** (no `env:` indirection inside the map) to keep the schema simple; indirection exists only for `args`.
6. **Stub `plugin.rs`** keeps the crate compiling end-to-end at this phase; Phase 4 replaces its body.

## Dependencies

- None (parallel with Phase 1).
- Required by Phases 3, 4, 5 (types `PluginConfig`/`ResolvedServer`, CLI args, module layout).
