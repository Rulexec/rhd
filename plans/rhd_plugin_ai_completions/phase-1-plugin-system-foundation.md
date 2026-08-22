# Phase 1: Plugin System Foundation

## Overview

This phase establishes the plugin infrastructure, documentation, and base structure for plugin implementations. It creates the foundational files and documentation that all subsequent phases will build upon.

**Scope**:
- Create `plugins/` directory structure
- Create comprehensive plugin system documentation
- Create base plugin package structure for `rhd_plugin_ai_completions`
- Define configuration format and loading logic

**Out of Scope**:
- Actual plugin logic implementation (Phase 4)
- Server-side API enhancements (Phase 2)
- Chat client enhancements (Phase 3)
- Integration testing (Phase 5)

## Files to Create

### 1. `plugins/README.md`

**Purpose**: Comprehensive documentation of the plugin system architecture, lifecycle, and conventions.

**Content Structure**:
```markdown
# RHD Plugin System

## Overview
Brief description of the plugin system and its purpose.

## Plugin Lifecycle
1. **Connect**: Plugin connects to chat server via WebSocket
2. **Register**: Plugin calls `registerPlugin` with unique ID
3. **Get Pending Acks**: Plugin calls `getPendingAcks` to recover missed events
4. **Subscribe**: Plugin subscribes to relevant events (chat list, individual chats, plugins list)
5. **React**: Plugin responds to events based on its logic

## Event Model
- **Custom Events**: Plugins can emit custom events via `sendCustomEvent`
- **Acknowledgments**: Plugins acknowledge events via `ackCustomEvent`
- **Pending Acks**: Events that haven't been acknowledged by the current plugin

## Plugin Responsibilities
Each plugin should document:
- Which events it listens for
- What tags it adds to messages/chat
- What events it emits and when
- Configuration format

## Configuration Conventions
- Plugins accept config file path as command-line argument
- Config files use YAML format
- Credentials should be in separate file referenced by config
- Use environment variable substitution where appropriate

## Creating a New Plugin
Step-by-step guide for creating a new plugin:
1. Create package in `plugins/` directory
2. Define `Cargo.toml` with required dependencies
3. Implement configuration loading
4. Implement main plugin logic
5. Create README documenting plugin behavior

## Plugin README Template
Template for individual plugin READMEs:
- Trigger conditions
- Events emitted
- Tags added
- Configuration format
- Dependencies
```

### 2. `plugins/rhd_plugin_ai_completions/README.md`

**Purpose**: Document this specific plugin's behavior, triggers, events, and tags.

**Content Structure**:
```markdown
# RHD Plugin AI Completions

## Overview
This plugin monitors chats and triggers AI completions when:
1. There are queued messages and no unresolved tool calls
2. All tool calls from the last assistant message are resolved (tool loop continuation)

## Trigger Conditions

### Queued Messages Mode
- Chat has non-zero queued messages count
- Chat has no unresolved tool calls
- Chat does not have `ai_completions:error` tag

### Tool Loop Continuation Mode
- Last assistant message has tool calls
- All tool calls have corresponding tool result messages
- Chat does not have `ai_completions:error` tag

## Events Emitted

### `ai_completions:preRequest`
Emitted before making AI completion request.

**Payload**:
```json
{
  "chatId": 123,
  "triggerReason": "queuedMessages" | "toolLoopContinuation"
}
```

**Purpose**: Allows other plugins to:
- Add their own messages to the queue
- Modify existing queued messages
- Perform cleanup or logging
- Block the request by not acknowledging

**Wait Logic**: Plugin waits for all other plugins to acknowledge this event before proceeding.

## Tags Added

### `ai_completions:error`
Added to chat when AI request fails.

**When**: AI completion request returns error (timeout, API error, etc.)

**Effect**: Plugin skips chats with this tag (no further processing).

## Messages Added

### Error Messages
When AI request fails, plugin adds message with:
- **Role**: `ai_completions:error`
- **Content**: Error details (error message, timeout info, etc.)

**Filtering**: These messages are filtered out when building AI requests (only `user`, `assistant`, `system`, `tool` roles are sent to AI).

## Configuration Format

```yaml
credentialsConfig: ../credentials.yaml
ai_completions:
  models:
    default:
      alias: qwen
    qwen:
      baseUrl: "https://api.example.com/v1"
      apiKey:
        cred: alibabaApiKey
      model: "qwen3.7-plus"
```

**Credentials File** (`credentials.yaml`):
```yaml
alibabaApiKey: your-api-key-here
```

## Dependencies
- **rhd_chat_client**: For connecting to chat server
- **rhd_ai_client**: For making AI completion requests
- **Other plugins**: May execute tool calls made by AI

## Error Handling
- On AI request failure: adds error tag and error message
- Skips chats with error tag
- Filters out error messages when building AI requests
- Tool calls in AI response are executed by other plugins
```

### 3. `plugins/rhd_plugin_ai_completions/Cargo.toml`

**Purpose**: Package manifest for the AI completions plugin.

**Content**:
```toml
[package]
name = "rhd_plugin_ai_completions"
version = "0.1.0"
edition = "2021"

[dependencies]
rhd_chat_client = { path = "../../packages/rhd_chat_client" }
rhd_chat_api = { path = "../../packages/rhd_chat_api" }
rhd_ai_client = { path = "../../packages/rhd_ai_client" }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_yaml = "0.9"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
clap = { version = "4", features = ["derive"] }
thiserror = "1"
```

### 4. `plugins/rhd_plugin_ai_completions/src/main.rs`

**Purpose**: Entry point for the plugin binary.

**Implementation**:
```rust
use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod config;
mod plugin;

#[derive(Parser, Debug)]
#[command(name = "rhd_plugin_ai_completions")]
#[command(about = "AI completions plugin for RHD chat system")]
struct Args {
    /// WebSocket URL of the chat server
    #[arg(long)]
    server_url: String,

    /// Path to configuration file
    #[arg(long)]
    config: String,

    /// Plugin ID (defaults to "ai_completions")
    #[arg(long, default_value = "ai_completions")]
    plugin_id: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env().add_directive("rhd_plugin_ai_completions=info".parse()?))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Parse arguments
    let args = Args::parse();

    // Load configuration
    let config = config::load_config(&args.config)?;

    tracing::info!("Starting AI completions plugin");
    tracing::info!("Server URL: {}", args.server_url);
    tracing::info!("Plugin ID: {}", args.plugin_id);

    // Run plugin
    plugin::run_plugin(&args.server_url, &args.plugin_id, config).await?;

    Ok(())
}
```

### 5. `plugins/rhd_plugin_ai_completions/src/config.rs`

**Purpose**: Configuration structure and loading logic.

**Implementation**:
```rust
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct PluginConfig {
    pub credentials_config: String,
    pub ai_completions: AiCompletionsConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AiCompletionsConfig {
    pub models: HashMap<String, ModelConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModelConfig {
    pub alias: Option<String>,
    pub base_url: Option<String>,
    pub api_key: ApiKeyConfig,
    pub model: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiKeyConfig {
    pub cred: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CredentialsConfig {
    #[serde(flatten)]
    pub credentials: HashMap<String, String>,
}

pub fn load_config(path: &str) -> Result<PluginConfig, ConfigError> {
    let config_content = std::fs::read_to_string(path)
        .map_err(|e| ConfigError::FileRead(e.to_string()))?;
    
    let config: PluginConfig = serde_yaml::from_str(&config_content)
        .map_err(|e| ConfigError::Parse(e.to_string()))?;
    
    // Validate that default model exists
    if !config.ai_completions.models.contains_key("default") {
        return Err(ConfigError::Validation("Missing 'default' model configuration".to_string()));
    }
    
    // Resolve default model alias
    let default_model = &config.ai_completions.models["default"];
    if let Some(alias) = &default_model.alias {
        if !config.ai_completions.models.contains_key(alias) {
            return Err(ConfigError::Validation(format!("Alias '{}' not found in models", alias)));
        }
    }
    
    Ok(config)
}

pub fn load_credentials(path: &str) -> Result<CredentialsConfig, ConfigError> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| ConfigError::FileRead(e.to_string()))?;
    
    let creds: CredentialsConfig = serde_yaml::from_str(&content)
        .map_err(|e| ConfigError::Parse(e.to_string()))?;
    
    Ok(creds)
}

pub fn resolve_api_key(config: &PluginConfig, cred_name: &str) -> Result<String, ConfigError> {
    let credentials = load_credentials(&config.credentials_config)?;
    
    credentials.credentials.get(cred_name)
        .cloned()
        .ok_or_else(|| ConfigError::Validation(format!("Credential '{}' not found", cred_name)))
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read config file: {0}")]
    FileRead(String),
    #[error("failed to parse config: {0}")]
    Parse(String),
    #[error("config validation error: {0}")]
    Validation(String),
}
```

### 6. `plugins/rhd_plugin_ai_completions/src/plugin.rs`

**Purpose**: Core plugin logic and event handling (stub for Phase 1, full implementation in Phase 4).

**Implementation**:
```rust
use rhd_chat_client::ChatClient;
use rhd_chat_api::RegisterPluginParams;

use crate::config::PluginConfig;

pub async fn run_plugin(
    server_url: &str,
    plugin_id: &str,
    config: PluginConfig,
) -> Result<(), PluginError> {
    // Connect to chat server
    let client = ChatClient::connect(server_url).await
        .map_err(|e| PluginError::Connection(e.to_string()))?;
    
    tracing::info!("Connected to chat server");
    
    // Register as plugin
    client.register_plugin(RegisterPluginParams {
        plugin_id: plugin_id.to_string(),
    }).await.map_err(|e| PluginError::Registration(e.to_string()))?;
    
    tracing::info!("Registered as plugin: {}", plugin_id);
    
    // Get pending acks
    let pending_acks = client.get_pending_acks(rhd_chat_api::GetPendingAcksParams {})
        .await
        .map_err(|e| PluginError::PendingAcks(e.to_string()))?;
    
    tracing::info!("Found {} pending acks", pending_acks.pending_events.len());
    
    // Process pending acks (stub - will be implemented in Phase 4)
    for event in pending_acks.pending_events {
        tracing::info!("Processing pending event: {}", event.event_name);
        // TODO: Implement pending ack processing in Phase 4
    }
    
    // TODO: Implement subscription and event handling in Phase 4
    tracing::info!("Plugin initialization complete");
    
    // Keep plugin running
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("failed to connect to server: {0}")]
    Connection(String),
    #[error("failed to register plugin: {0}")]
    Registration(String),
    #[error("failed to get pending acks: {0}")]
    PendingAcks(String),
}
```

## Tests

### Unit Tests for `config.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_load_config_valid() {
        let mut config_file = NamedTempFile::new().unwrap();
        writeln!(config_file, r#"
credentialsConfig: /tmp/creds.yaml
ai_completions:
  models:
    default:
      alias: qwen
    qwen:
      baseUrl: "https://api.example.com/v1"
      apiKey:
        cred: testKey
      model: "qwen3.7-plus"
"#).unwrap();

        let config = load_config(config_file.path().to_str().unwrap()).unwrap();
        assert!(config.ai_completions.models.contains_key("default"));
        assert!(config.ai_completions.models.contains_key("qwen"));
    }

    #[test]
    fn test_load_config_missing_default() {
        let mut config_file = NamedTempFile::new().unwrap();
        writeln!(config_file, r#"
credentialsConfig: /tmp/creds.yaml
ai_completions:
  models:
    qwen:
      baseUrl: "https://api.example.com/v1"
      apiKey:
        cred: testKey
      model: "qwen3.7-plus"
"#).unwrap();

        let result = load_config(config_file.path().to_str().unwrap());
        assert!(result.is_err());
    }

    #[test]
    fn test_load_credentials() {
        let mut creds_file = NamedTempFile::new().unwrap();
        writeln!(creds_file, r#"
testKey: my-secret-key
anotherKey: another-secret
"#).unwrap();

        let creds = load_credentials(creds_file.path().to_str().unwrap()).unwrap();
        assert_eq!(creds.credentials.get("testKey"), Some(&"my-secret-key".to_string()));
    }
}
```

## Implementation Notes

1. **Configuration Validation**: The config loader validates that:
   - The `default` model exists
   - If `default` has an alias, the aliased model exists
   - Credentials file can be loaded

2. **Error Handling**: All configuration errors are wrapped in `ConfigError` enum with clear error messages.

3. **Logging**: Uses `tracing` for structured logging with environment-based log level control.

4. **Plugin Lifecycle**: The plugin follows the standard lifecycle:
   - Connect → Register → Get Pending Acks → Subscribe → React

5. **Stub Implementation**: Phase 1 creates the structure but leaves the actual event handling logic for Phase 4.

## Dependencies

- **None**: This is the foundational phase
- **Must be completed before**: Phase 2, Phase 3, Phase 4

## Success Criteria

- [ ] `plugins/README.md` exists with comprehensive documentation
- [ ] `plugins/rhd_plugin_ai_completions/README.md` exists with plugin-specific documentation
- [ ] `plugins/rhd_plugin_ai_completions/Cargo.toml` exists with correct dependencies
- [ ] `plugins/rhd_plugin_ai_completions/src/main.rs` compiles and runs
- [ ] `plugins/rhd_plugin_ai_completions/src/config.rs` loads and validates configuration
- [ ] `plugins/rhd_plugin_ai_completions/src/plugin.rs` connects and registers with server
- [ ] All unit tests pass
- [ ] Plugin binary can be built with `cargo build`
