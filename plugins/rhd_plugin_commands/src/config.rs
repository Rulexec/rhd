//! Configuration loading and validation for the commands plugin.
//!
//! The YAML surface is a strict `commands:` map (`deny_unknown_fields`) in
//! which every value is one of four shapes:
//!
//! 1. a bare path string (prompt shorthand, role `user`),
//! 2. a single tagged spec (`type: chat_tags | message_tags | prompt`),
//! 3. a list mixing both shapes (multi-step command).
//!
//! Loading resolves every prompt file against the config file's directory and
//! caches its content verbatim (markdown, no content parsing), failing fast on
//! any problem — same convention as the system_prompt plugin.

use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Raw plugin configuration loaded from the YAML file.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginConfig {
    pub commands: HashMap<String, CommandDef>,
}

/// A command definition: either a single step or an ordered multi-step list.
///
/// Variant order is load-bearing: serde's untagged matching tries variants in
/// declaration order, so `Multi` must come first to capture YAML sequences.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum CommandDef {
    Multi(Vec<CommandStep>),
    Single(CommandStep),
}

/// One step of a command: a bare prompt-path string or a tagged spec.
///
/// Variant order is load-bearing: `PromptPath` must be tried before `Spec` so
/// bare strings resolve to the prompt shorthand.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum CommandStep {
    /// Shorthand: a path to a markdown prompt file, inserted as role=user.
    PromptPath(String),
    Spec(CommandSpec),
}

/// Tagged command spec (`type` discriminator, snake_case values).
///
/// Note: `deny_unknown_fields` is applied at the enum level (the serde version
/// in use does not accept it on individual variants); the internally tagged
/// `type` field itself is consumed by the tag machinery, not by the variants.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum CommandSpec {
    ChatTags {
        #[serde(default)]
        add: Vec<String>,
        #[serde(default)]
        remove: Vec<String>,
    },
    MessageTags {
        #[serde(default)]
        add: Vec<String>,
        #[serde(default)]
        remove: Vec<String>,
    },
    Prompt {
        /// One of: user (default), system, assistant.
        #[serde(default = "default_prompt_role")]
        role: String,
        /// Path to a markdown file; resolved against the config directory.
        prompt: String,
    },
}

fn default_prompt_role() -> String {
    "user".to_string()
}

/// A fully validated, I/O-resolved command step (prompt contents cached).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedStep {
    ChatTags {
        add: Vec<String>,
        remove: Vec<String>,
    },
    MessageTags {
        add: Vec<String>,
        remove: Vec<String>,
    },
    Prompt {
        name: String,
        role: String,
        content: String,
    },
}

/// Registry of validated commands keyed by name, consumed by the executor.
#[derive(Debug, Clone)]
pub struct CommandRegistry {
    commands: HashMap<String, Vec<ResolvedStep>>,
}

impl CommandRegistry {
    /// Whether a command with this exact (case-sensitive) name is registered.
    pub fn has(&self, name: &str) -> bool {
        self.commands.contains_key(name)
    }

    /// Ordered steps of a registered command, or `None` when unknown.
    pub fn steps(&self, name: &str) -> Option<&[ResolvedStep]> {
        self.commands.get(name).map(Vec::as_slice)
    }

    /// The set of registered command names (input to `parser::parse`).
    pub fn names(&self) -> HashSet<String> {
        self.commands.keys().cloned().collect()
    }

    /// Number of registered commands.
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Whether the registry contains no commands.
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}

/// Load, validate, and resolve the plugin configuration.
///
/// This function:
/// 1. Reads the YAML config file (`Parse` / `MainConfigFileRead` on failure)
/// 2. Rejects command-name keys outside `[A-Za-z0-9_]`
/// 3. Resolves every step, reading prompt files relative to the config dir
/// 4. Fails fast on missing/unreadable prompt files, invalid prompt roles,
///    and tag commands with neither `add` nor `remove`
pub fn load_config(path: &str) -> Result<CommandRegistry, ConfigError> {
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

    let mut commands: HashMap<String, Vec<ResolvedStep>> = HashMap::new();
    for (name, def) in &config.commands {
        if !is_valid_command_name(name) {
            return Err(ConfigError::InvalidCommandName(name.clone()));
        }
        let steps = match def {
            CommandDef::Single(step) => vec![resolve_step(step, name, config_dir)?],
            CommandDef::Multi(steps) => steps
                .iter()
                .map(|step| resolve_step(step, name, config_dir))
                .collect::<Result<Vec<_>, _>>()?,
        };
        commands.insert(name.clone(), steps);
    }

    Ok(CommandRegistry { commands })
}

/// Validate a command-name key: non-empty, `[A-Za-z0-9_]` only
/// (the `^[A-Za-z0-9_]+$` equivalent without a regex dependency).
fn is_valid_command_name(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Resolve one raw step into its validated, prompt-cached form.
fn resolve_step(
    step: &CommandStep,
    name: &str,
    config_dir: &Path,
) -> Result<ResolvedStep, ConfigError> {
    match step {
        CommandStep::PromptPath(prompt) => Ok(ResolvedStep::Prompt {
            name: name.to_string(),
            role: default_prompt_role(),
            content: read_prompt(prompt, name, config_dir)?,
        }),
        CommandStep::Spec(CommandSpec::ChatTags { add, remove }) => {
            check_tags(name, add, remove)?;
            Ok(ResolvedStep::ChatTags {
                add: add.clone(),
                remove: remove.clone(),
            })
        }
        CommandStep::Spec(CommandSpec::MessageTags { add, remove }) => {
            check_tags(name, add, remove)?;
            Ok(ResolvedStep::MessageTags {
                add: add.clone(),
                remove: remove.clone(),
            })
        }
        CommandStep::Spec(CommandSpec::Prompt { role, prompt }) => {
            if !matches!(role.as_str(), "user" | "system" | "assistant") {
                return Err(ConfigError::InvalidPromptRole {
                    name: name.to_string(),
                    role: role.clone(),
                });
            }
            Ok(ResolvedStep::Prompt {
                name: name.to_string(),
                role: role.clone(),
                content: read_prompt(prompt, name, config_dir)?,
            })
        }
    }
}

/// Tag steps must do something (`add` or `remove` non-empty) and never carry
/// empty tag strings.
fn check_tags(name: &str, add: &[String], remove: &[String]) -> Result<(), ConfigError> {
    if add.is_empty() && remove.is_empty() {
        return Err(ConfigError::EmptyTags {
            name: name.to_string(),
        });
    }
    if add.iter().chain(remove.iter()).any(|tag| tag.is_empty()) {
        return Err(ConfigError::EmptyTag {
            name: name.to_string(),
        });
    }
    Ok(())
}

/// Read a prompt file verbatim, resolving relative paths against the config
/// directory (absolute paths are used as-is).
fn read_prompt(prompt: &str, name: &str, config_dir: &Path) -> Result<String, ConfigError> {
    let resolved_path = resolve_path(prompt, config_dir);
    let content =
        std::fs::read_to_string(&resolved_path).map_err(|e| ConfigError::PromptFileRead {
            name: name.to_string(),
            path: resolved_path,
            details: e.to_string(),
        })?;
    Ok(content)
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

    #[error("failed to parse config: {0}")]
    Parse(String),

    #[error("invalid command name '{0}': only [A-Za-z0-9_] allowed")]
    InvalidCommandName(String),

    #[error("failed to read prompt file for command '{name}' at '{path}': {details}")]
    PromptFileRead {
        name: String,
        path: String,
        details: String,
    },

    #[error("command '{name}' has prompt role '{role}' (allowed: user, system, assistant)")]
    InvalidPromptRole { name: String, role: String },

    #[error("command '{name}' has neither add nor remove tags")]
    EmptyTags { name: String },

    #[error("command '{name}' has an empty tag")]
    EmptyTag { name: String },
}

#[cfg(test)]
mod tests;
