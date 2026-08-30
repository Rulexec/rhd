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
    let config_dir = config_path
        .parent()
        .ok_or_else(|| ConfigError::MainConfigFileRead {
            path: path.to_string(),
            details: "Cannot determine config file directory".to_string(),
        })?;

    let config_content =
        std::fs::read_to_string(path).map_err(|e| ConfigError::MainConfigFileRead {
            path: path.to_string(),
            details: e.to_string(),
        })?;

    let config: PluginConfig =
        serde_yaml::from_str(&config_content).map_err(|e| ConfigError::Parse(e.to_string()))?;

    // Validate and cache all prompt files
    let mut cached_prompts = Vec::new();
    for (name, prompt_path) in &config.system_prompts {
        let resolved_path = resolve_path(prompt_path, config_dir);
        let content =
            std::fs::read_to_string(&resolved_path).map_err(|e| ConfigError::PromptFileRead {
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
        // Strip leading "./" if present for cleaner path resolution
        let normalized = path.strip_prefix("./").unwrap_or(path);
        base_dir.join(normalized).to_string_lossy().to_string()
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_load_config_valid() {
        // Create a temporary config file
        let mut config_file = NamedTempFile::new().unwrap();
        writeln!(
            config_file,
            r#"
systemPrompts:
  warhammer: ./prompts/warhammer.md
  jokeTeller: ./prompts/jokes.md
"#
        )
        .unwrap();

        // Create temporary prompt files
        let config_dir = config_file.path().parent().unwrap();
        std::fs::create_dir_all(config_dir.join("prompts")).unwrap();
        std::fs::write(
            config_dir.join("prompts/warhammer.md"),
            "Warhammer prompt content",
        )
        .unwrap();
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
        writeln!(
            config_file,
            r#"
systemPrompts:
  warhammer: ./prompts/nonexistent.md
"#
        )
        .unwrap();

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
