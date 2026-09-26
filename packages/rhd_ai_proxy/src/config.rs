//! YAML configuration for the proxy.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

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
    /// Optional chat logging into a standalone SQLite database.
    #[serde(default)]
    pub logging: Option<Logging>,
}

/// Chat logging configuration (`proxy.logging`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Logging {
    /// Folder holding the logging SQLite database. Relative paths are resolved
    /// against the config file location by [`Config::load`].
    pub path: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Target {
    /// Absolute http(s) base URL to forward to, e.g. `https://example.org/raw/openrouter/v1`.
    pub path: String,
    /// Optional API key sent as `Authorization: Bearer <token>` on every forwarded request.
    ///
    /// Accepts either a raw string (used verbatim) or `{ env: "VAR_NAME" }` to resolve the
    /// token from an environment variable at startup.
    #[serde(default)]
    pub api_key: Option<ApiKey>,
}

/// Configurable API key: either a literal token or a reference to an environment variable.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ApiKey {
    /// Raw token string used verbatim.
    Literal(String),
    /// `{ env: "VAR_NAME" }` — the token is read from the named environment variable.
    FromEnv(EnvApiKey),
}

#[derive(Debug, Clone, Deserialize)]
pub struct EnvApiKey {
    pub env: String,
}

impl ApiKey {
    /// Resolves the API key into the literal token string.
    ///
    /// `Literal(s)` returns `s` directly; `FromEnv { env }` reads the variable from the
    /// process environment and fails with [`ConfigError::MissingEnvVar`] if unset.
    pub fn resolve(&self) -> Result<String, ConfigError> {
        match self {
            ApiKey::Literal(token) => Ok(token.clone()),
            ApiKey::FromEnv(source) => {
                std::env::var(&source.env).map_err(|_| ConfigError::MissingEnvVar {
                    env: source.env.clone(),
                })
            }
        }
    }
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
    #[error("environment variable {env:?} referenced by apiKey is not set")]
    MissingEnvVar { env: String },
    #[error("logging database error: {0}")]
    Logging(#[from] crate::logging::LoggingError),
}

impl Config {
    /// Parse and validate configuration from a YAML string.
    pub fn from_yaml(yaml: &str) -> Result<Self, ConfigError> {
        let config: Config = serde_yaml::from_str(yaml)?;
        config.proxy.target_url()?;
        if let Some(key) = &config.proxy.target.api_key {
            key.resolve()?;
        }
        Ok(config)
    }

    /// Read, parse, and validate configuration from a YAML file.
    ///
    /// A relative `proxy.logging.path` is rewritten in-place to an absolute path resolved
    /// against the config file's directory (so the same config file works from any cwd).
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let contents = std::fs::read_to_string(path).map_err(|source| ConfigError::Io {
            path: path.display().to_string(),
            source,
        })?;
        let mut config = Self::from_yaml(&contents)?;
        config.resolve_logging_path(path)?;
        Ok(config)
    }

    /// Rewrites a relative `logging.path` to absolute, relative to the config file location.
    fn resolve_logging_path(&mut self, config_path: &Path) -> Result<(), ConfigError> {
        let Some(logging) = &mut self.proxy.logging else {
            return Ok(());
        };
        if logging.path.is_absolute() {
            return Ok(());
        }
        let base = match config_path.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
            _ => std::env::current_dir().map_err(|source| ConfigError::Io {
                path: config_path.display().to_string(),
                source,
            })?,
        };
        logging.path = base.join(&logging.path);
        Ok(())
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

    /// Resolved API key token, if configured. Reads the environment variable on every call
    /// when the key is `{ env: "..." }`; returns `MissingEnvVar` when the variable is unset.
    pub fn api_key(&self) -> Result<Option<String>, ConfigError> {
        self.target
            .api_key
            .as_ref()
            .map(|key| key.resolve())
            .transpose()
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
        assert!(config.proxy.target.api_key.is_none());
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

    #[test]
    fn parses_literal_api_key() {
        let config = Config::from_yaml(
            r#"
proxy:
  port: 1
  target:
    path: http://localhost:9000/v1
    apiKey: sk-literal-1234
"#,
        )
        .unwrap();
        assert_eq!(config.proxy.api_key().unwrap().as_deref(), Some("sk-literal-1234"));
    }

    #[test]
    fn parses_env_api_key_when_var_is_set() {
        std::env::set_var("RHD_AI_PROXY_TEST_KEY", "sk-from-env-5678");
        let config = Config::from_yaml(
            r#"
proxy:
  port: 1
  target:
    path: http://localhost:9000/v1
    apiKey: { env: RHD_AI_PROXY_TEST_KEY }
"#,
        )
        .unwrap();
        assert_eq!(config.proxy.api_key().unwrap().as_deref(), Some("sk-from-env-5678"));
        std::env::remove_var("RHD_AI_PROXY_TEST_KEY");
    }

    #[test]
    fn rejects_env_api_key_when_var_is_unset() {
        std::env::remove_var("RHD_AI_PROXY_DEFINITELY_UNSET");
        let err = Config::from_yaml(
            r#"
proxy:
  port: 1
  target:
    path: http://localhost:9000/v1
    apiKey: { env: RHD_AI_PROXY_DEFINITELY_UNSET }
"#,
        )
        .unwrap_err();
        assert!(matches!(err, ConfigError::MissingEnvVar { .. }));
    }

    #[test]
    fn api_key_defaults_to_none_when_omitted() {
        let config = Config::from_yaml(
            r#"
proxy:
  port: 1
  target:
    path: http://localhost:9000/v1
"#,
        )
        .unwrap();
        assert!(config.proxy.api_key().unwrap().is_none());
    }

    #[test]
    fn parses_logging_section() {
        let config = Config::from_yaml(
            r#"
proxy:
  port: 1
  target:
    path: http://localhost:9000/v1
  logging:
    path: ./proxy-logs
"#,
        )
        .unwrap();
        let logging = config.proxy.logging.expect("logging should parse");
        assert_eq!(logging.path, PathBuf::from("./proxy-logs"));
    }

    #[test]
    fn logging_defaults_to_none() {
        let config = Config::from_yaml(
            r#"
proxy:
  port: 1
  target:
    path: http://localhost:9000/v1
"#,
        )
        .unwrap();
        assert!(config.proxy.logging.is_none());
    }

    #[test]
    fn rejects_unknown_logging_field() {
        let err = Config::from_yaml(
            r#"
proxy:
  port: 1
  target:
    path: http://localhost:9000/v1
  logging:
    path: ./logs
    extra: true
"#,
        )
        .unwrap_err();
        assert!(matches!(err, ConfigError::Parse(_)));
    }

    fn write_temp_config(name: &str, contents: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "rhd_ai_proxy_config_test_{name}_{}.yaml",
            std::process::id()
        ));
        std::fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn load_resolves_relative_logging_path_against_config_file() {
        let config_path = write_temp_config(
            "relative",
            r#"
proxy:
  port: 1
  target:
    path: http://localhost:9000/v1
  logging:
    path: ./proxy-logs
"#,
        );
        let config = Config::load(&config_path).unwrap();
        let logging = config.proxy.logging.expect("logging should parse");
        assert_eq!(
            logging.path,
            config_path.parent().unwrap().join("proxy-logs")
        );
        let _ = std::fs::remove_file(&config_path);
    }

    #[test]
    fn load_keeps_absolute_logging_path() {
        let config_path = write_temp_config(
            "absolute",
            r#"
proxy:
  port: 1
  target:
    path: http://localhost:9000/v1
  logging:
    path: /var/log/ai-proxy
"#,
        );
        let config = Config::load(&config_path).unwrap();
        let logging = config.proxy.logging.expect("logging should parse");
        assert_eq!(logging.path, PathBuf::from("/var/log/ai-proxy"));
        let _ = std::fs::remove_file(&config_path);
    }
}
