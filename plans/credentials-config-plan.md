# Credentials Config Plan

## Goal

Add optional `credentials.yaml` support to separate API keys from model configs, enabling safe sharing of `rhd.yaml` and scenario files without leaking secrets. API keys support three forms: plain strings, credential references, and full environment variable substitution with strict validation.

## Architecture

### New Config Type: `credentials.yaml`

Optional file containing key-value pairs of credential names to secret values:

```yaml
myApiKey: "sk-..."
anotherKey: "secret123"
```

Referenced from `rhd.yaml` via `credentialsConfig` field:

```yaml
credentialsConfig: ../credentials.yaml
```

Path resolved relative to `rhd.yaml` location. If `credentialsConfig` is specified and file does not exist, daemon exits with error. If `credentialsConfig` is not specified, no error occurs (credentials are optional).

### API Key Resolution

`apiKey` field in model configs supports three forms:

1. **Plain string**: `apiKey: "sk-..."` — used as-is (existing behavior)
2. **Credential reference**: 
   ```yaml
   apiKey:
     cred: myApiKey
   ```
   Looks up `myApiKey` in loaded credentials map
3. **Environment variable (full replacement only)**: `apiKey: "$MY_KEY"` — entire value replaced with env var

**Partial env var substitution NOT supported for apiKey**: `apiKey: "sk-$MY_KEY"` remains literal string `"sk-$MY_KEY"` (no substitution). This differs from other string fields which use `substitute_env_vars()`.

**Strict validation**: If apiKey is `"$VAR_NAME"` form and env var is empty/unset, daemon exits with error at startup. No fallback to literal string.

### Data Flow

```
rhd.yaml (credentialsConfig: ../credentials.yaml)
    ↓
load_credentials() → HashMap<String, String>
    ↓
models/*.yaml (apiKey: string | {cred: name} | "$ENV_VAR")
    ↓
resolve_api_key() → final apiKey string
    ↓
ModelConfig.api_key (String)
```

## Implementation Steps

### 1. Add `credentials_config` field to `DaemonConfig`

**File**: [`packages/rhd_app/src/config.rs`](packages/rhd_app/src/config.rs)

- Add `credentials_config: Option<PathBuf>` field with `#[serde(default)]`
- Path resolved relative to config file directory in `load_config()`

### 2. Create credentials loader

**File**: [`packages/rhd_app/src/credentials.rs`](packages/rhd_app/src/credentials.rs) (new)

- `load_credentials(path: &Path) -> Result<HashMap<String, String>, CredentialsError>`
- Parse YAML as `HashMap<String, String>`
- Error types: file read error, YAML parse error
- Called from `run_daemon_command()` after config load

### 3. Change `ModelConfig.api_key` type

**File**: [`packages/rhd_ai/src/config.rs`](packages/rhd_ai/src/config.rs)

- Create enum `ApiKeySource`:
  ```rust
  #[derive(Debug, Deserialize, Clone)]
  #[serde(untagged)]
  pub enum ApiKeySource {
      Plain(String),
      Cred { cred: String },
  }
  ```
- Change `api_key: String` to `api_key: ApiKeySource`
- Remove `substitute_env_vars()` call on `api_key` in `load_models()` (handled later)

### 4. Add API key resolution function

**File**: [`packages/rhd_ai/src/config.rs`](packages/rhd_ai/src/config.rs)

- `resolve_api_key(source: &ApiKeySource, credentials: &HashMap<String, String>) -> Result<String, ApiKeyError>`
- Logic:
  - `Plain(s)` where `s.starts_with('$')` and contains no other chars after `$` → env var lookup
    - If env var unset/empty → error
    - Else → return env var value
  - `Plain(s)` otherwise → return `s` as-is (no substitution)
  - `Cred { cred }` → lookup in credentials map
    - If missing → error
    - Else → return value

### 5. Update `load_models()` signature

**File**: [`packages/rhd_ai/src/config.rs`](packages/rhd_ai/src/config.rs)

- Add `credentials: &HashMap<String, String>` parameter
- After parsing each model, call `resolve_api_key()` to get final `api_key: String`
- Return `Result<HashMap<String, ModelConfig>, ConfigError>` (error type extended)

### 6. Update daemon startup

**File**: [`packages/rhd_app/src/main.rs`](packages/rhd_app/src/main.rs)

- In `run_daemon_command()`:
  - Load credentials if `credentials_config` is `Some`
  - Pass credentials to `load_models()`
  - If credentials file specified but not found → exit with error

### 7. Update `ModelConfig` struct

**File**: [`packages/rhd_ai/src/config.rs`](packages/rhd_ai/src/config.rs)

- Keep `api_key: String` in final struct (resolved value)
- Use intermediate struct for deserialization with `ApiKeySource`
- Or deserialize directly to `ApiKeySource` then resolve

### 8. Add error types

**File**: [`packages/rhd_ai/src/config.rs`](packages/rhd_ai/src/config.rs)

- Extend `ConfigError`:
  ```rust
  #[error("api key references credential '{cred}' but not found in credentials file")]
  CredentialNotFound { cred: String },
  
  #[error("api key environment variable '{var}' is not set or empty")]
  EnvVarNotSet { var: String },
  
  #[error("credentials file not found: {path}")]
  CredentialsFileNotFound { path: PathBuf },
  ```

### 9. Update tests

**Files**: 
- [`packages/rhd_ai/src/config.rs`](packages/rhd_ai/src/config.rs) (unit tests)
- [`test_e2e/`](test_e2e/) (E2E test with credentials)

- Unit tests for `resolve_api_key()`:
  - Plain string → returns as-is
  - `"$VAR"` with set env var → returns value
  - `"$VAR"` with unset env var → error
  - `"sk-$VAR"` → returns literal `"sk-$VAR"` (no substitution)
  - `Cred { cred }` with existing key → returns value
  - `Cred { cred }` with missing key → error
- Unit tests for backward compatibility:
  - Existing model config with plain string `apiKey: "sk-..."` parses correctly
  - Model config with env var `apiKey: "$MY_KEY"` parses correctly
  - Verify `ApiKeySource` deserialization handles all three forms
- E2E test: create `credentials.yaml`, reference from model config, verify API key resolved correctly

### 10. Update documentation

**File**: [`AI.md`](AI.md)

- Document `credentialsConfig` field in `rhd.yaml` format
- Document three `apiKey` forms
- Document strict env var validation for apiKey
- Add example showing secure config sharing pattern

## File Changes Summary

| File | Change |
|------|--------|
| [`packages/rhd_app/src/config.rs`](packages/rhd_app/src/config.rs) | Add `credentials_config: Option<PathBuf>` field |
| [`packages/rhd_app/src/credentials.rs`](packages/rhd_app/src/credentials.rs) | New file: credentials loader |
| [`packages/rhd_app/src/main.rs`](packages/rhd_app/src/main.rs) | Load credentials, pass to `load_models()` |
| [`packages/rhd_ai/src/config.rs`](packages/rhd_ai/src/config.rs) | `ApiKeySource` enum, `resolve_api_key()`, update `load_models()` signature |
| [`AI.md`](AI.md) | Document credentials feature |
| [`test_e2e/`](test_e2e/) | Add E2E test for credentials |

## Risks

1. **Breaking change**: `apiKey` type change from `String` to `ApiKeySource` requires updating all existing model configs. Mitigation: `#[serde(untagged)]` allows plain strings to deserialize as `ApiKeySource::Plain`. Add unit tests to verify backward compatibility: existing model configs with plain string `apiKey` must parse correctly.

2. **Path resolution**: `credentialsConfig` path must be resolved relative to `rhd.yaml`, not CWD. Mitigation: resolve in `load_config()` using config file's parent directory.

3. **Env var validation strictness**: Existing configs using `"$VAR"` in `apiKey` will break if var unset. Mitigation: document breaking change; this is intentional for security.

## Success Criteria

- Daemon loads `credentials.yaml` if `credentialsConfig` specified in `rhd.yaml`
- Model configs support three `apiKey` forms: plain string, `{cred: name}`, `"$ENV_VAR"`
- Partial env var substitution (`"sk-$VAR"`) does NOT occur for apiKey
- Daemon exits with error if `"$ENV_VAR"` used and var is unset/empty
- Daemon exits with error if credential reference not found
- Existing model configs with plain string `apiKey` continue to work
- E2E test validates credentials resolution
- Documentation updated with examples
