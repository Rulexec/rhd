//! YAML configuration for the proxy.

use std::collections::HashMap;
use std::path::Path;

use reqwest::Url;
use serde::Deserialize;

/// Root configuration: `{ proxy: {...} }`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub proxy: Proxy,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Proxy {
    /// Port to listen on (bound to 127.0.0.1).
    pub port: u16,
    pub target: Target,
    /// Per-model overrides, keyed by the exact `model` string from the request body.
    #[serde(default)]
    pub models: HashMap<String, ModelConfig>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    /// Absolute http(s) base URL to forward to, e.g. `https://example.org/raw/openrouter/v1`.
    pub path: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelConfig {
    /// Keys merged into the top level of the request body for this model.
    pub extra_body: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read config file {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse config: {0}")]
    Parse(#[from] serde_yaml::Error),
    #[error("invalid target path {0:?}: expected an absolute http(s) URL")]
    InvalidTargetPath(String),
}

impl Config {
    /// Parse and validate configuration from a YAML string.
    pub fn from_yaml(yaml: &str) -> Result<Self, ConfigError> {
        let config: Config = serde_yaml::from_str(yaml)?;
        config.proxy.target_url()?;
        Ok(config)
    }

    /// Read, parse, and validate configuration from a YAML file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let contents = std::fs::read_to_string(path).map_err(|source| ConfigError::Io {
            path: path.display().to_string(),
            source,
        })?;
        Self::from_yaml(&contents)
    }
}

impl Proxy {
    /// Parsed and validated target base URL.
    pub fn target_url(&self) -> Result<Url, ConfigError> {
        let url = Url::parse(&self.target.path)
            .map_err(|_| ConfigError::InvalidTargetPath(self.target.path.clone()))?;
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
            return Err(ConfigError::InvalidTargetPath(self.target.path.clone()));
        }
        Ok(url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &str = r#"
proxy:
  port: 1234
  target:
    path: https://example.org/raw/openrouter/v1
  models:
    "z-ai/glm-5.3":
      extraBody:
        provider:
          sort: throughput
          max_price: {"prompt": 1, "completion": 2}
"#;

    #[test]
    fn parses_example_config() {
        let config = Config::from_yaml(EXAMPLE).unwrap();
        assert_eq!(config.proxy.port, 1234);
        assert_eq!(
            config.proxy.target.path,
            "https://example.org/raw/openrouter/v1"
        );
        let model = config.proxy.models.get("z-ai/glm-5.3").unwrap();
        assert_eq!(model.extra_body["provider"]["sort"], "throughput");
        assert_eq!(model.extra_body["provider"]["max_price"]["prompt"], 1);
        assert_eq!(model.extra_body["provider"]["max_price"]["completion"], 2);
    }

    #[test]
    fn models_section_is_optional() {
        let config = Config::from_yaml(
            r#"
proxy:
  port: 1
  target:
    path: http://localhost:9000/v1
"#,
        )
        .unwrap();
        assert!(config.proxy.models.is_empty());
    }

    #[test]
    fn rejects_unknown_top_level_field() {
        let err = Config::from_yaml(
            r#"
proxy:
  port: 1
  target:
    path: http://localhost:9000/v1
extra: true
"#,
        )
        .unwrap_err();
        assert!(matches!(err, ConfigError::Parse(_)));
    }

    #[test]
    fn rejects_snake_case_extra_body() {
        let err = Config::from_yaml(
            r#"
proxy:
  port: 1
  target:
    path: http://localhost:9000/v1
  models:
    "a":
      extra_body: {}
"#,
        )
        .unwrap_err();
        assert!(matches!(err, ConfigError::Parse(_)));
    }

    #[test]
    fn rejects_non_absolute_target_path() {
        let err = Config::from_yaml(
            r#"
proxy:
  port: 1
  target:
    path: /relative/path
"#,
        )
        .unwrap_err();
        assert!(matches!(err, ConfigError::InvalidTargetPath(_)));
    }

    #[test]
    fn rejects_non_http_target_scheme() {
        let err = Config::from_yaml(
            r#"
proxy:
  port: 1
  target:
    path: ftp://example.com/v1
"#,
        )
        .unwrap_err();
        assert!(matches!(err, ConfigError::InvalidTargetPath(_)));
    }

    #[test]
    fn load_reports_missing_file() {
        let err = Config::load("/nonexistent/rhd_ai_proxy_config.yaml").unwrap_err();
        assert!(matches!(err, ConfigError::Io { .. }));
    }
}
