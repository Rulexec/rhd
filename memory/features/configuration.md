# Configuration

## Purpose
System configuration via YAML files and CLI arguments. Supports model definitions, scenario directories, credentials separation, and runtime overrides.

## Configuration Files

### `rhd.yaml` — Main Config
Loaded at daemon startup. CLI args override config values.

```yaml
scenariosDir: scenarios      # directory containing scenario folders
modelsDir: models            # directory containing model YAML files
mcpDir: mcp                  # directory containing MCP server configs
defaultModel: null           # fallback model for aiChat steps without model field
logs: null                   # directory for execution logs (null = no log files)
logChats: null               # directory for chat interaction logs (null = no logging)
logChatsRaw: false           # enable raw API request/response logging (requires logChats)
dbDir: rhd_db                # directory for SQLite databases
wsPort: null                 # WebSocket server port (null = disabled)
credentialsConfig: null      # path to credentials file (relative to rhd.yaml)
neverFail: false             # pause on AI errors instead of failing (requires wsPort)
```

### `models/<name>.yaml` — Model Definitions
Each file defines an AI model configuration. Filename = model name.

```yaml
baseUrl: "https://api.openai.com/v1"   # OpenAI-compatible endpoint
apiKey: "sk-..."                        # API key (see credentials below)
model: "gpt-4"                          # model identifier passed to API
inputTokenPrice: 5.0                    # optional: price per 1M input tokens
outputTokenPrice: 15.0                  # optional: price per 1M output tokens
priceTiers:                             # optional: tiered pricing
  - afterTokens: 250000
    inputTokenPrice: 10.0
    outputTokenPrice: 30.0
```

**Environment variables:** `$VAR` syntax in all string fields, substituted at load time.

**Model aliases:** File with `alias: target_model` makes filename an alias for target.

### `credentials.yaml` — Secrets (Optional)
Separates API keys from shareable config files.

```yaml
myApiKey: "sk-..."
anotherKey: "secret123"
```

Referenced from `rhd.yaml` via `credentialsConfig: ../credentials.yaml` (path relative to `rhd.yaml`).

### API Key Resolution
`apiKey` field supports three forms:
1. **Plain string:** `apiKey: "sk-..."` — used as-is
2. **Credential reference:** `apiKey: { cred: myApiKey }` — looks up in credentials file
3. **Environment variable:** `apiKey: "$MY_KEY"` — entire value replaced with env var

**Strict validation:** `$VAR` form requires env var to be set and non-empty at startup. Partial substitution (`"sk-$VAR"`) NOT supported for apiKey — remains literal.

## CLI Arguments

### `rhd daemon`
```
--config PATH          # config file path (default: rhd.yaml)
--models-dir PATH      # override modelsDir
--scenarios-dir PATH   # override scenariosDir
--mcp-dir PATH         # override mcpDir
--default-model NAME   # override defaultModel
--logs PATH            # override logs directory
--db-dir PATH          # override dbDir
--ws-port PORT         # override wsPort
--socket PATH          # Unix socket path (default: $HOME/rhd.sock)
```

### `rhd run <name>`
```
--socket PATH          # Unix socket path (default: $HOME/rhd.sock)
--modelAlias ALIAS=TARGET  # override model names at runtime (repeatable)
```

### `rhd reload`
```
--socket PATH          # Unix socket path (default: $HOME/rhd.sock)
```

Reloads all configuration files (scenarios, models, MCP servers, projects) without restarting the daemon. Waits for active executions to complete, then reloads configs and restarts MCP servers whose configuration has changed. Returns counts of reloaded items.

## Model Alias Resolution
Two-stage resolution:
1. **YAML aliases:** `models/medium.yaml` with `alias: gpt4` → `medium` resolves to `gpt4`
2. **CLI overrides:** `--modelAlias medium=gpt4` replaces model names in all aiChat steps

CLI applies to any model, even YAML-alias-resolved ones. Logs and meta.json show final resolved model name.

## MCP ID Field
Project MCP configurations support an optional `id` field. If not specified, the `name` field is used as the ID. The ID is used for:
- Tool namespacing: tools are exposed to AI as `{id}/{tool_name}`
- Conflict detection: attaching projects with duplicate MCP IDs is rejected

## Startup Validation
- All model configs validated at daemon startup
- Invalid YAML → daemon exits with error (file path + line number)
- Missing credentials file (when referenced) → daemon exits with error
- Missing model directories → daemon exits with error
- Socket path cleaned up on startup (stale socket removed)
