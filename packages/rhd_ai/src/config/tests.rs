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
