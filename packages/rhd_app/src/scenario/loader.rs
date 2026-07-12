use std::collections::HashMap;
use std::path::Path;

use thiserror::Error;

use super::{Action, Scenario};

#[derive(Debug, Error)]
pub enum ScenarioLoadError {
    #[error("failed to read scenario file '{path}': {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to parse scenario YAML at {path}:{line}:{column}: {message}")]
    Parse {
        path: String,
        line: usize,
        column: usize,
        message: String,
    },
    #[error("scenario validation failed at {path}: {message}")]
    Validation { path: String, message: String },
}

pub fn load_scenario(path: &Path) -> Result<Scenario, ScenarioLoadError> {
    let path_str = path.display().to_string();
    let contents = std::fs::read_to_string(path).map_err(|source| ScenarioLoadError::Io {
        path: path_str.clone(),
        source,
    })?;

    let mut scenario: Scenario = serde_yaml::from_str(&contents).map_err(|e| ScenarioLoadError::Parse {
        path: path_str.clone(),
        line: e.location().map(|l| l.line()).unwrap_or(0),
        column: e.location().map(|l| l.column()).unwrap_or(0),
        message: e.to_string(),
    })?;
    scenario.apply_env_vars();
    validate_scenario(&scenario, &path_str)?;
    Ok(scenario)
}

fn validate_scenario(scenario: &Scenario, path: &str) -> Result<(), ScenarioLoadError> {
    if scenario.actions.is_empty() {
        return Err(ScenarioLoadError::Validation {
            path: path.to_string(),
            message: "scenario must have at least one action".into(),
        });
    }

    let mut seen_names = std::collections::HashSet::new();
    for action in &scenario.actions {
        if let Some(name) = action.name() {
            if name.is_empty() {
                return Err(ScenarioLoadError::Validation {
                    path: path.to_string(),
                    message: "action name must not be empty".into(),
                });
            }
            if !seen_names.insert(name.to_string()) {
                return Err(ScenarioLoadError::Validation {
                    path: path.to_string(),
                    message: format!("duplicate action name: '{name}'"),
                });
            }
        }

        if let Action::AiChat(chat) = action {
            if let Some(mcp_refs) = &chat.mcp {
                let mut seen_ids = std::collections::HashSet::new();
                for mcp_ref in mcp_refs {
                    let eid = mcp_ref.effective_id().to_string();
                    if !seen_ids.insert(eid.clone()) {
                        return Err(ScenarioLoadError::Validation {
                            path: path.to_string(),
                            message: format!(
                                "duplicate MCP id '{}' in aiChat step '{}'",
                                eid,
                                chat.name.as_deref().unwrap_or("<unnamed>")
                            ),
                        });
                    }
                }
            }
        }
    }

    Ok(())
}

pub fn load_scenarios_dir(dir: &Path) -> Result<HashMap<String, Scenario>, ScenarioLoadError> {
    let mut scenarios = HashMap::new();
    let entries = dir.read_dir().map_err(|source| ScenarioLoadError::Io {
        path: dir.display().to_string(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| ScenarioLoadError::Io {
            path: dir.display().to_string(),
            source,
        })?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let scenario_file = path.join("scenario.yaml");
        if !scenario_file.exists() {
            continue;
        }
        let scenario = load_scenario(&scenario_file)?;
        let dir_name = path.file_name()
            .expect("directory must have a name")
            .to_string_lossy()
            .into_owned();
        scenarios.insert(dir_name, scenario);
    }
    Ok(scenarios)
}
