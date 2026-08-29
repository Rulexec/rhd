# Phase 1: Project Setup & Configuration

## Overview

This phase establishes the plugin project structure and implements configuration loading with path resolution and validation. This creates the foundation for all subsequent work.

**Scope:**
- Create plugin directory structure
- Implement configuration loading from YAML
- Resolve prompt file paths relative to config directory
- Validate all prompt files exist at startup
- Cache prompt file contents in memory

**Out of Scope:**
- Plugin lifecycle (connect, register, monitor)
- System prompt injection logic
- Event handling

## Files to Create/Modify

### 1. `plugins/rhd_plugin_system_prompt/Cargo.toml`

**Create new file:**

```toml
[package]
name = "rhd_plugin_system_prompt"
version = "0.1.0"
edition = "2021"

[dependencies]
rhd_chat_client = { path = "../../packages/rhd_chat_client" }
rhd_chat_api = { path = "../../packages/rhd_chat_api" }
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
clap = { version = "4", features = ["derive"] }
thiserror = "1"

[dev-dependencies]
tempfile = "3"
```

**Rationale:** Dependencies match the existing `rhd_plugin_ai_completions` plugin for consistency.

### 2. `plugins/rhd_plugin_system_prompt/src/lib.rs`

**Create new file:**

```rust
//! System prompt plugin for RHD chat system.
//!
//! This plugin automatically injects system prompts into chats based on chat tags.
//! It monitors chats for tags like `systemPrompt:warhammer` and adds corresponding
//! system prompt messages from configured markdown files.

pub mod config;
pub mod plugin;
pub mod system_prompt;
```

**Rationale:** Module structure matches the existing plugin pattern.

### 3. `plugins/rhd_plugin_system_prompt/src/config.rs`

**Create new file with the following structure:**

```rust
//! Configuration loading and validation for the system prompt plugin.

use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// Plugin configuration loaded from YAML file.
#[derive(Debug, Clone, Deserialize)]
pub struct PluginConfig {
    /// Map of prompt names to file paths.
    /// Example: { "warhammer": "./prompts/warhammer.md" }
    #[serde(rename = "systemPrompts")]
    pub system_prompts: HashMap<String, String>,
}

/// Cached prompt content with its source path.
#[derive(Debug, Clone)]
pub struct CachedPrompt {
    /// The name of the prompt (e.g., "warhammer")
    pub name: String,
    /// The content of the prompt file
    pub content: String,
    /// The resolved absolute path to the file
    pub path: String,
}

/// Load and validate plugin configuration.
///
/// This function:
/// 1. Reads the YAML config file
/// 2. Resolves relative paths against the config file's directory
/// 3. Reads and caches all prompt file contents
/// 4. Fails if any prompt file is missing or unreadable
pub fn load_config(path: &str) -> Result<(PluginConfig, Vec<CachedPrompt>), ConfigError> {
    let config_path = Path::new(path);
    let config_dir = config_path.parent().ok_or_else(|| ConfigError::MainConfigFileRead {
        path: path.to_string(),
        details: "Cannot determine config file directory".to_string(),
    })?;

    let config_content = std::fs::read_to_string(path).map_err(|e| ConfigError::MainConfigFileRead {
        path: path.to_string(),
        details: e.to_string(),
    })?;

    let config: PluginConfig = serde_yaml::from_str(&config_content)
        .map_err(|e| ConfigError::Parse(e.to_string()))?;

    // Validate and cache all prompt files
    let mut cached_prompts = Vec::new();
    for (name, prompt_path) in &config.system_prompts {
        let resolved_path = resolve_path(prompt_path, config_dir);
        let content = std::fs::read_to_string(&resolved_path).map_err(|e| ConfigError::PromptFileRead {
            name: name.clone(),
            path: resolved_path.clone(),
            details: e.to_string(),
        })?;

        cached_prompts.push(CachedPrompt {
            name: name.clone(),
            content,
            path: resolved_path,
        });
    }

    Ok((config, cached_prompts))
}

/// Resolve a path relative to a base directory.
/// If the path is absolute, return it as-is.
fn resolve_path(path: &str, base_dir: &Path) -> String {
    let prompt_path = Path::new(path);
    if prompt_path.is_absolute() {
        path.to_string()
    } else {
        base_dir.join(prompt_path).to_string_lossy().to_string()
    }
}

/// Errors that can occur during configuration loading.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read main config file '{path}': {details}")]
    MainConfigFileRead { path: String, details: String },

    #[error("failed to read prompt file '{name}' at '{path}': {details}")]
    PromptFileRead {
        name: String,
        path: String,
        details: String,
    },

    #[error("failed to parse config: {0}")]
    Parse(String),
}
```

**Key Implementation Details:**

1. **Path Resolution**: The `resolve_path` function checks if the path is absolute. If not, it joins it with the config directory.

2. **Validation**: The `load_config` function iterates through all configured prompts and attempts to read each file. If any file is missing or unreadable, it returns a `ConfigError::PromptFileRead` error.

3. **Caching**: All prompt contents are read once and stored in `CachedPrompt` structs, which are returned alongside the config.

### 4. `Cargo.toml` (workspace root)

**Modify existing file:**

Add `plugins/rhd_plugin_system_prompt` to the `members` array in the workspace definition.

**Location:** After the existing `plugins/rhd_plugin_ai_completions` entry.

```toml
[workspace]
members = [
    # ... existing members ...
    "plugins/rhd_plugin_ai_completions",
    "plugins/rhd_plugin_system_prompt",  # Add this line
]
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
        // Create a temporary config file
        let mut config_file = NamedTempFile::new().unwrap();
        writeln!(config_file, r#"
systemPrompts:
  warhammer: ./prompts/warhammer.md
  jokeTeller: ./prompts/jokes.md
"#).unwrap();

        // Create temporary prompt files
        let config_dir = config_file.path().parent().unwrap();
        std::fs::create_dir_all(config_dir.join("prompts")).unwrap();
        std::fs::write(config_dir.join("prompts/warhammer.md"), "Warhammer prompt content").unwrap();
        std::fs::write(config_dir.join("prompts/jokes.md"), "Joke prompt content").unwrap();

        let (config, cached) = load_config(config_file.path().to_str().unwrap()).unwrap();
        
        assert_eq!(config.system_prompts.len(), 2);
        assert_eq!(cached.len(), 2);
        assert!(cached.iter().any(|p| p.name == "warhammer"));
        assert!(cached.iter().any(|p| p.name == "jokeTeller"));
    }

    #[test]
    fn test_load_config_missing_prompt_file() {
        let mut config_file = NamedTempFile::new().unwrap();
        writeln!(config_file, r#"
systemPrompts:
  warhammer: ./prompts/nonexistent.md
"#).unwrap();

        let result = load_config(config_file.path().to_str().unwrap());
        assert!(result.is_err());
        match result.unwrap_err() {
            ConfigError::PromptFileRead { name, .. } => {
                assert_eq!(name, "warhammer");
            }
            _ => panic!("Expected PromptFileRead error"),
        }
    }

    #[test]
    fn test_resolve_path_relative() {
        let base = Path::new("/config/dir");
        let resolved = resolve_path("./prompts/test.md", base);
        assert_eq!(resolved, "/config/dir/prompts/test.md");
    }

    #[test]
    fn test_resolve_path_absolute() {
        let base = Path::new("/config/dir");
        let resolved = resolve_path("/absolute/path/test.md", base);
        assert_eq!(resolved, "/absolute/path/test.md");
    }
}
```

## Implementation Notes

1. **Error Messages**: Error messages include the prompt name and path to make debugging easier.

2. **Path Resolution**: Relative paths are resolved against the config file's directory, not the current working directory. This matches the behavior of `credentialsConfig` in `rhd_plugin_ai_completions`.

3. **Caching Strategy**: All prompt contents are loaded into memory at startup. This is acceptable because:
   - Prompt files are typically small (a few KB)
   - It avoids repeated I/O during the main loop
   - It ensures consistent prompt content throughout the plugin's lifetime

4. **Validation Timing**: Validation happens at startup, not lazily. This ensures configuration errors are caught immediately.

## Dependencies

- **None** - This is the foundation phase
- This phase must be completed before Phase 2

## Success Criteria

- [ ] Plugin compiles successfully with `cargo build`
- [ ] Config loading works with relative paths
- [ ] Config loading works with absolute paths
- [ ] Missing prompt files cause startup failure with clear error message
- [ ] Prompt contents are cached and accessible via `CachedPrompt` structs
- [ ] All unit tests pass
