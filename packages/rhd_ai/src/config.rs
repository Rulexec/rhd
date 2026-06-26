use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum ApiKeySource {
    Plain(String),
    Cred { cred: String },
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawModelConfig {
    pub base_url: String,
    pub api_key: ApiKeySource,
    pub model: String,
    #[serde(default)]
    pub input_token_price: Option<f64>,
    #[serde(default)]
    pub output_token_price: Option<f64>,
    #[serde(default)]
    pub price_tiers: Option<Vec<rhd_api::TokenPriceTier>>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawAliasConfig {
    pub alias: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
enum RawModelEntry {
    Alias(RawAliasConfig),
    Full(RawModelConfig),
}

#[derive(Debug, Clone)]
pub struct ModelConfig {
    pub model_id: String,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub input_token_price: Option<f64>,
    pub output_token_price: Option<f64>,
    pub price_tiers: Option<Vec<rhd_api::TokenPriceTier>>,
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

    #[error("invalid yaml in {path}:{line}:{column}: {message}")]
    YamlParse {
        path: PathBuf,
        line: usize,
        column: usize,
        message: String,
    },

    #[error("no model files found in {path}")]
    NoModels { path: PathBuf },

    #[error("api key references credential '{cred}' but not found in credentials file")]
    CredentialNotFound { cred: String },

    #[error("api key environment variable '{var}' is not set or empty")]
    EnvVarNotSet { var: String },

    #[error("alias '{alias}' references unknown model '{target}'")]
    AliasTargetNotFound { alias: String, target: String },

    #[error("circular alias chain detected: '{chain}'")]
    CircularAlias { chain: String },
}

pub fn resolve_api_key(
    source: &ApiKeySource,
    credentials: &HashMap<String, String>,
) -> Result<String, ConfigError> {
    match source {
        ApiKeySource::Plain(s) => {
            if s.starts_with('$') && s.len() > 1 && s[1..].chars().all(|c| c.is_alphanumeric() || c == '_') {
                let var_name = &s[1..];
                match std::env::var(var_name) {
                    Ok(val) if !val.is_empty() => Ok(val),
                    _ => Err(ConfigError::EnvVarNotSet {
                        var: var_name.to_string(),
                    }),
                }
            } else {
                Ok(s.clone())
            }
        }
        ApiKeySource::Cred { cred } => credentials.get(cred).cloned().ok_or_else(|| ConfigError::CredentialNotFound {
            cred: cred.clone(),
        }),
    }
}

pub fn load_models(
    models_dir: &Path,
    credentials: &HashMap<String, String>,
) -> Result<HashMap<String, ModelConfig>, ConfigError> {
    let entries = models_dir
        .read_dir()
        .map_err(|source| ConfigError::ModelsDirRead {
            path: models_dir.to_path_buf(),
            source,
        })?;

    let mut raw_entries: HashMap<String, RawModelEntry> = HashMap::new();

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

        let raw_entry: RawModelEntry =
            serde_yaml::from_str(&contents).map_err(|e| ConfigError::YamlParse {
                path: file_path.clone(),
                line: e.location().map(|l| l.line()).unwrap_or(0),
                column: e.location().map(|l| l.column()).unwrap_or(0),
                message: e.to_string(),
            })?;

        raw_entries.insert(model_name, raw_entry);
    }

    if raw_entries.is_empty() {
        return Err(ConfigError::NoModels {
            path: models_dir.to_path_buf(),
        });
    }

    let mut models: HashMap<String, ModelConfig> = HashMap::new();

    for (name, entry) in &raw_entries {
        match entry {
            RawModelEntry::Full(raw_config) => {
                let resolved_api_key = resolve_api_key(&raw_config.api_key, credentials)?;
                let config = ModelConfig {
                    model_id: name.clone(),
                    base_url: rhd_util::substitute_env_vars(&raw_config.base_url),
                    api_key: resolved_api_key,
                    model: rhd_util::substitute_env_vars(&raw_config.model),
                    input_token_price: raw_config.input_token_price,
                    output_token_price: raw_config.output_token_price,
                    price_tiers: raw_config.price_tiers.clone(),
                };
                models.insert(name.clone(), config);
            }
            RawModelEntry::Alias(_) => {}
        }
    }

    for (alias_name, entry) in &raw_entries {
        if let RawModelEntry::Alias(alias_config) = entry {
            let resolved = resolve_alias_chain(alias_name, &alias_config.alias, &raw_entries)?;
            let config = match raw_entries.get(&resolved) {
                Some(RawModelEntry::Full(raw_config)) => {
                    let resolved_api_key = resolve_api_key(&raw_config.api_key, credentials)?;
                    ModelConfig {
                        model_id: resolved.clone(),
                        base_url: rhd_util::substitute_env_vars(&raw_config.base_url),
                        api_key: resolved_api_key,
                        model: rhd_util::substitute_env_vars(&raw_config.model),
                        input_token_price: raw_config.input_token_price,
                        output_token_price: raw_config.output_token_price,
                        price_tiers: raw_config.price_tiers.clone(),
                    }
                }
                _ => {
                    return Err(ConfigError::AliasTargetNotFound {
                        alias: alias_name.clone(),
                        target: alias_config.alias.clone(),
                    });
                }
            };
            models.insert(alias_name.clone(), config);
        }
    }

    Ok(models)
}

fn resolve_alias_chain(
    alias_name: &str,
    target: &str,
    raw_entries: &HashMap<String, RawModelEntry>,
) -> Result<String, ConfigError> {
    let mut visited = vec![alias_name.to_string()];
    let mut current = target.to_string();

    loop {
        if visited.contains(&current) {
            visited.push(current);
            return Err(ConfigError::CircularAlias {
                chain: visited.join(" -> "),
            });
        }

        match raw_entries.get(&current) {
            Some(RawModelEntry::Full(_)) => return Ok(current),
            Some(RawModelEntry::Alias(alias_config)) => {
                visited.push(current.clone());
                current = alias_config.alias.clone();
            }
            None => {
                return Err(ConfigError::AliasTargetNotFound {
                    alias: alias_name.to_string(),
                    target: target.to_string(),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backward_compatibility_plain_string() {
        let yaml = r#"
baseUrl: "https://api.openai.com/v1"
apiKey: "sk-test-key"
model: "gpt-4"
"#;
        let config: RawModelConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(matches!(config.api_key, ApiKeySource::Plain(ref s) if s == "sk-test-key"));
    }

    #[test]
    fn test_backward_compatibility_env_var() {
        let yaml = r#"
baseUrl: "https://api.openai.com/v1"
apiKey: "$MY_API_KEY"
model: "gpt-4"
"#;
        let config: RawModelConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(matches!(config.api_key, ApiKeySource::Plain(ref s) if s == "$MY_API_KEY"));
    }

    #[test]
    fn test_credential_reference() {
        let yaml = r#"
baseUrl: "https://api.openai.com/v1"
apiKey:
  cred: myApiKey
model: "gpt-4"
"#;
        let config: RawModelConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(matches!(config.api_key, ApiKeySource::Cred { ref cred } if cred == "myApiKey"));
    }

    #[test]
    fn test_resolve_api_key_plain_string() {
        let source = ApiKeySource::Plain("sk-test-key".to_string());
        let credentials = HashMap::new();
        let result = resolve_api_key(&source, &credentials).unwrap();
        assert_eq!(result, "sk-test-key");
    }

    #[test]
    fn test_resolve_api_key_env_var_exists() {
        std::env::set_var("TEST_API_KEY_123", "sk-from-env");
        let source = ApiKeySource::Plain("$TEST_API_KEY_123".to_string());
        let credentials = HashMap::new();
        let result = resolve_api_key(&source, &credentials).unwrap();
        assert_eq!(result, "sk-from-env");
        std::env::remove_var("TEST_API_KEY_123");
    }

    #[test]
    fn test_resolve_api_key_env_var_not_set() {
        let source = ApiKeySource::Plain("$NONEXISTENT_VAR_XYZ".to_string());
        let credentials = HashMap::new();
        let result = resolve_api_key(&source, &credentials);
        assert!(matches!(result, Err(ConfigError::EnvVarNotSet { ref var }) if var == "NONEXISTENT_VAR_XYZ"));
    }

    #[test]
    fn test_resolve_api_key_partial_env_var_no_substitution() {
        std::env::set_var("TEST_PARTIAL_VAR", "value");
        let source = ApiKeySource::Plain("sk-$TEST_PARTIAL_VAR".to_string());
        let credentials = HashMap::new();
        let result = resolve_api_key(&source, &credentials).unwrap();
        assert_eq!(result, "sk-$TEST_PARTIAL_VAR");
        std::env::remove_var("TEST_PARTIAL_VAR");
    }

    #[test]
    fn test_resolve_api_key_credential_exists() {
        let source = ApiKeySource::Cred { cred: "myKey".to_string() };
        let mut credentials = HashMap::new();
        credentials.insert("myKey".to_string(), "sk-credential-value".to_string());
        let result = resolve_api_key(&source, &credentials).unwrap();
        assert_eq!(result, "sk-credential-value");
    }

    #[test]
    fn test_resolve_api_key_credential_not_found() {
        let source = ApiKeySource::Cred { cred: "missingKey".to_string() };
        let credentials = HashMap::new();
        let result = resolve_api_key(&source, &credentials);
        assert!(matches!(result, Err(ConfigError::CredentialNotFound { ref cred }) if cred == "missingKey"));
    }
}
