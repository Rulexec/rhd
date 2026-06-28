# Configuration

## Environment Variable Substitution

Model configs and scenario YAMLs support `$ENV_VAR` syntax in string values. At load time, all `$VAR_NAME` patterns (alphanumeric + underscore) are replaced with the corresponding environment variable value. If the variable is not set, the original `$VAR_NAME` string is kept as-is.

Example:
```yaml
baseUrl: "http://localhost:$E2E_MODEL_PORT/v1"
cmd: "$E2E_SCRIPTS_DIR/run.sh"
```

## Model Config (`models/*.yaml`)

```yaml
baseUrl: "https://api.openai.com/v1"
apiKey: "sk-..."          # Plain string, credential reference, or env var
model: "gpt-4"
inputTokenPrice: 5.0      # Optional: price per 1M tokens
outputTokenPrice: 15.0    # Optional: price per 1M tokens
priceTiers:               # Optional: tiered pricing
  - afterTokens: 250000
    inputTokenPrice: 10.0
    outputTokenPrice: 30.0
```

**Important**: The filename (without extension) is used as the model identifier in logs, meta.json, and scenario references. The `model` field is only used for API calls. For example, `models/gpt4.yaml` with `model: "gpt-4"` will be referenced as `gpt4` in scenarios and logs, while `gpt-4` is sent to the API. For aliases, the resolved target name is used in logs (e.g., `small.yaml` with `alias: other_model` logs as `other_model`).

## Model Alias

A model config file can contain only an `alias` field to reference another model:
```yaml
alias: gpt4
```
This creates an alias named after the filename (e.g., `medium.yaml` with `alias: gpt4` creates a `medium` alias that resolves to the `gpt4` model). In logs and meta.json, the resolved target name is shown (e.g., `medium` alias logs as `gpt4`). Aliases can chain (alias pointing to another alias), but circular references are not allowed.

## CLI Model Alias Override

The `rhd run` command supports `--modelAlias ALIAS=TARGET` to override model names at runtime:
```bash
rhd run example --modelAlias medium=gpt4 --modelAlias small=qwen3
```
This replaces all occurrences of `medium` with `gpt4` and `small` with `qwen3` in aiChat steps. CLI aliases apply after YAML alias resolution, so they can override both direct model names and YAML-resolved aliases.

## API Key Forms

The `apiKey` field supports three forms:
1. **Plain string**: `apiKey: "sk-..."` - used as-is
2. **Credential reference**:
   ```yaml
   apiKey:
     cred: myApiKey
   ```
   References a key in the credentials file (see Credentials Configuration below)
3. **Environment variable** (full replacement only): `apiKey: "$MY_API_KEY"` - entire value replaced with env var

**Note**: Partial environment variable substitution (e.g., `apiKey: "sk-$MY_KEY"`) is NOT supported for apiKey. The string must be exactly `"$VAR_NAME"` to trigger env var lookup. If the environment variable is not set or empty, the daemon will fail to start.

## CLI Usage

```bash
# Start daemon
rhd daemon [--config rhd.yaml] [--models-dir models] [--scenarios-dir scenarios] [--mcp-dir mcp] [--default-model name] [--logs logs] [--socket PATH] [--ws-port PORT] [--db-dir DIR]

# Run scenario
rhd run <scenario_name> [--socket PATH] [--modelAlias ALIAS=TARGET]
```

By default, the socket is located at `$HOME/rhd.sock`. The `--socket` flag allows specifying a custom socket path.
The `--ws-port` flag enables WebSocket server on the specified port (optional).
The `--db-dir` flag specifies the directory for the SQLite database (default: `rhd_db`).

## Configuration File (`rhd.yaml`)

The daemon can be configured via a YAML file (default: `rhd.yaml` in current directory). CLI arguments override config file values.

```yaml
modelsDir: models
scenariosDir: scenarios
mcpDir: mcp
defaultModel: null
logs: null
wsPort: null
dbDir: rhd_db
credentialsConfig: null   # Optional: path to credentials file
```

- `modelsDir`: Directory containing model YAML files (default: `models`)
- `scenariosDir`: Directory containing scenario folders (default: `scenarios`)
- `mcpDir`: Directory containing MCP server configurations (default: `mcp`)
- `defaultModel`: Fallback model for `aiChat` steps without `model` field (default: `null`)
- `logs`: Directory for execution logs (default: `null`, no logging)
- `wsPort`: WebSocket server port (default: `null`, disabled)
- `dbDir`: Directory for SQLite database (default: `rhd_db`, creates `meta.db` inside)
- `credentialsConfig`: Path to credentials file (default: `null`, optional). Path is resolved relative to the config file location.

## Credentials Configuration

The credentials feature allows separating sensitive API keys from model configurations, enabling safe sharing of `rhd.yaml` and scenario files without leaking secrets.

**Credentials file format** (`credentials.yaml`):
```yaml
myApiKey: "sk-..."
anotherKey: "secret123"
```

**Usage in rhd.yaml**:
```yaml
credentialsConfig: ../credentials.yaml
modelsDir: models
scenariosDir: scenarios
```

**Behavior**:
- If `credentialsConfig` is specified and the file does not exist, the daemon exits with an error
- If `credentialsConfig` is not specified, no error occurs (credentials are optional)
- Path is resolved relative to the `rhd.yaml` file location
- Credentials can be referenced in model configs using `apiKey: { cred: keyName }`

**Example**: Secure configuration sharing
```
project/
├── rhd.yaml              # Can be committed to git
├── credentials.yaml      # Add to .gitignore
├── models/
│   └── gpt4.yaml         # References credentials
└── scenarios/
    └── my_scenario/
        └── scenario.yaml
```

In `rhd.yaml`:
```yaml
credentialsConfig: ./credentials.yaml
```

In `models/gpt4.yaml`:
```yaml
baseUrl: "https://api.openai.com/v1"
apiKey:
  cred: openaiKey
model: "gpt-4"
```

In `credentials.yaml` (not committed):
```yaml
openaiKey: "sk-actual-api-key-here"
```
