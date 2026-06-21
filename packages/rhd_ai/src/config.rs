use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct ModelConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read models directory {path}: {source}")]
    ModelsDirRead {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("failed to read model file {path}: {source}")]
    FileRead {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("invalid yaml in {path}: {source}")]
    YamlParse {
        path: PathBuf,
        source: serde_yaml::Error,
    },

    #[error("no model files found in {path}")]
    NoModels { path: PathBuf },
}

pub fn load_models(models_dir: &Path) -> Result<HashMap<String, ModelConfig>, ConfigError> {
    let entries = models_dir
        .read_dir()
        .map_err(|source| ConfigError::ModelsDirRead {
            path: models_dir.to_path_buf(),
            source,
        })?;

    let mut models = HashMap::new();

    for entry in entries {
        let entry = entry.map_err(|source| ConfigError::ModelsDirRead {
            path: models_dir.to_path_buf(),
            source,
        })?;

        let file_path = entry.path();

        if file_path.extension().and_then(|ext| ext.to_str()) != Some("yaml")
            && file_path.extension().and_then(|ext| ext.to_str()) != Some("yml")
        {
            continue;
        }

        let model_name = file_path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_default()
            .to_string();

        let contents =
            std::fs::read_to_string(&file_path).map_err(|source| ConfigError::FileRead {
                path: file_path.clone(),
                source,
            })?;

        let config: ModelConfig =
            serde_yaml::from_str(&contents).map_err(|source| ConfigError::YamlParse {
                path: file_path.clone(),
                source,
            })?;

        models.insert(model_name, config);
    }

    if models.is_empty() {
        return Err(ConfigError::NoModels {
            path: models_dir.to_path_buf(),
        });
    }

    Ok(models)
}
