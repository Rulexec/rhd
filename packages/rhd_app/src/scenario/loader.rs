use std::path::Path;

use thiserror::Error;

use super::Scenario;

#[derive(Debug, Error)]
pub enum ScenarioLoadError {
    #[error("failed to read scenario file '{path}': {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to parse scenario YAML: {0}")]
    Parse(#[from] serde_yaml::Error),
    #[error("scenario validation failed: {0}")]
    Validation(String),
}

pub fn load_scenario(path: &Path) -> Result<Scenario, ScenarioLoadError> {
    let path_str = path.display().to_string();
    let contents = std::fs::read_to_string(path).map_err(|source| ScenarioLoadError::Io {
        path: path_str,
        source,
    })?;

    let scenario: Scenario = serde_yaml::from_str(&contents)?;
    validate_scenario(&scenario)?;
    Ok(scenario)
}

fn validate_scenario(scenario: &Scenario) -> Result<(), ScenarioLoadError> {
    if scenario.name.is_empty() {
        return Err(ScenarioLoadError::Validation(
            "scenario name must not be empty".into(),
        ));
    }

    if scenario.actions.is_empty() {
        return Err(ScenarioLoadError::Validation(
            "scenario must have at least one action".into(),
        ));
    }

    let mut seen_names = std::collections::HashSet::new();
    for action in &scenario.actions {
        if let Some(name) = action.name() {
            if name.is_empty() {
                return Err(ScenarioLoadError::Validation(
                    "action name must not be empty".into(),
                ));
            }
            if !seen_names.insert(name.to_string()) {
                return Err(ScenarioLoadError::Validation(format!(
                    "duplicate action name: '{name}'"
                )));
            }
        }
    }

    Ok(())
}
