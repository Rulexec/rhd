use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct PluginConfig {
    #[serde(rename = "credentialsConfig")]
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
    #[serde(rename = "baseUrl")]
    pub base_url: Option<String>,
    #[serde(rename = "apiKey")]
    pub api_key: Option<ApiKeyConfig>,
    pub model: Option<String>,
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
