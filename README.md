# rhd

Rust-based automation tool for AI-assisted task execution. Daemon/client architecture: long-running daemon executes scenarios (action chains) via Unix socket IPC.

## Architecture

- **Daemon**: Unix socket server on `$HOME/rhd.sock` (default), accepts `RunScenario` requests
- **Client**: Connects to daemon, sends scenario name, receives output
- **Scenario executor**: Runs action chains sequentially with placeholder resolution
- **IPC protocol**: rkyv serialization with version-prefixed framing

## Install

```bash
cargo build --release
```

Binary: `target/release/rhd`

### Global wrapper script

Save as `~/.local/bin/rhd` (or any directory in `$PATH`):

```bash
#!/bin/bash

cargo build --release --manifest-path ~/projects/rhd/Cargo.toml > /dev/null 2>&1
if [ $? -ne 0 ]; then
    echo "Build failed" >&2
    exit 1
fi

exec ~/projects/rhd/target/release/rhd "$@"
```

Make executable:

```bash
chmod +x ~/.local/bin/rhd
```

## CLI

### Start daemon

```bash
rhd daemon [OPTIONS]
```

| Flag | Default | Description |
|---|---|---|
| `--config PATH` | `rhd.yaml` | Path to configuration file |
| `--models-dir PATH` | `models` | Directory with model YAML files (overrides config) |
| `--scenarios-dir PATH` | `scenarios` | Directory with scenario folders (overrides config) |
| `--mcp-dir PATH` | `mcp` | Directory with MCP server configurations (overrides config) |
| `--default-model NAME` | — | Fallback model for `aiChat` steps without `model` field (overrides config) |
| `--logs PATH` | — | Directory for execution logs (overrides config) |
| `--socket PATH` | `$HOME/rhd.sock` | Unix socket path |
| `--ws-port PORT` | — | WebSocket server port for Web UI (optional) |
| `--db-dir DIR` | `rhd_db` | Directory for SQLite database (overrides config) |

## Configuration File

The daemon can be configured via a YAML file (default: `rhd.yaml` in current directory). CLI arguments override config file values.

**Config file format** (`rhd.yaml`):
```yaml
modelsDir: models
scenariosDir: scenarios
mcpDir: mcp
defaultModel: null
logs: null
wsPort: null
dbDir: rhd_db
credentialsConfig: null
```

| Field | Default | Description |
|---|---|---|
| `modelsDir` | `models` | Directory containing model YAML files |
| `scenariosDir` | `scenarios` | Directory containing scenario folders |
| `mcpDir` | `mcp` | Directory containing MCP server configurations |
| `defaultModel` | `null` | Fallback model for `aiChat` steps without `model` field |
| `logs` | `null` | Directory for execution logs (no logging if null) |
| `wsPort` | `null` | WebSocket server port (disabled if null) |
| `dbDir` | `rhd_db` | Directory for SQLite database |
| `credentialsConfig` | `null` | Path to credentials file (relative to config file) |

## Execution Logs

When `logs` is configured, each scenario execution creates a timestamped log directory:
- Format: `<logs>/<scenarioName>-YYYY-MM-DD-HH-MM-SS/`
- Collision handling: If directory exists, appends `-2`, `-3`, etc.
- Log file: `log.txt` inside the directory

### Run scenario

```bash
rhd run <scenario_name> [--socket PATH] [--modelAlias ALIAS=TARGET]
```

| Flag | Description |
|---|---|
| `--socket PATH` | Unix socket path (default: `$HOME/rhd.sock`) |
| `--modelAlias ALIAS=TARGET` | Override model alias at runtime (can be repeated) |

Client captures current working directory and sends it to daemon. Commands execute in client's cwd unless scenario overrides with `cwd` field.

**Model alias override example:**
```bash
rhd run example --modelAlias medium=gpt4 --modelAlias small=qwen3
```

## Models

Model configs: `models/<name>.yaml`. Filename (without extension) becomes model name.

| Field | Required | Description |
|---|---|---|
| `baseUrl` | yes | OpenAI-compatible API endpoint. Supports `$ENV_VAR`. |
| `apiKey` | yes | API key. Plain string, `$ENV_VAR`, or `{ cred: keyName }`. |
| `model` | yes | Model identifier passed to API. Supports `$ENV_VAR`. |
| `inputTokenPrice` | no | Price per 1M input tokens |
| `outputTokenPrice` | no | Price per 1M output tokens |
| `priceTiers` | no | Tiered pricing (see below) |

Example `models/deepseek.yaml`:

```yaml
baseUrl: "https://api.deepseek.com/v1"
apiKey: "$DEEPSEEK_API_KEY"
model: "deepseek-chat"
```

### Token pricing

Optional fields for cost tracking:

```yaml
inputTokenPrice: 5.0      # Price per 1M input tokens
outputTokenPrice: 15.0    # Price per 1M output tokens
priceTiers:               # Optional tiered pricing
  - afterTokens: 250000   # Apply after this many tokens
    inputTokenPrice: 10.0
    outputTokenPrice: 30.0
```

### Model alias

A model config can reference another model:

```yaml
# models/medium.yaml
alias: gpt4
```

Filename becomes alias name. Resolves to target model at runtime. Aliases can chain.

## Credentials

Separate API keys from config for safe sharing:

**`credentials.yaml`:**
```yaml
openaiKey: "sk-..."
anthropicKey: "sk-ant-..."
```

**`rhd.yaml`:**
```yaml
credentialsConfig: ./credentials.yaml
```

**`models/gpt4.yaml`:**
```yaml
baseUrl: "https://api.openai.com/v1"
apiKey:
  cred: openaiKey
model: "gpt-4"
```

Add `credentials.yaml` to `.gitignore`.

## WebSocket Server

Optional WebSocket server for Web UI integration. Enable via `--ws-port` or `wsPort` config.

Default disabled. Used by frontend for real-time scenario monitoring.

## Scenarios

Scenarios: `scenarios/<name>/scenario.yaml`. Directory name is scenario identifier.

### Top-level fields

| Field | Required | Description |
|---|---|---|
| `description` | no | Free text description |
| `actions` | yes | Ordered list of actions |

### Action types

#### `runCommand`

Execute shell command. **Never fails scenario** on non-zero exit.

| Field | Required | Description |
|---|---|---|
| `type` | yes | `runCommand` |
| `name` | no | Step name for placeholder references |
| `cmd` | yes | Executable path or command. Supports `$ENV_VAR`. |
| `args` | no | Argument list. Default `[]`. Each arg supports `$ENV_VAR`. |
| `cwd` | no | Working directory. Default: client's cwd. Supports `$ENV_VAR`. |

#### `aiChat`

Send message to AI model, receive response. Supports tool calling via MCP (Model Context Protocol).

| Field | Required | Description |
|---|---|---|
| `type` | yes | `aiChat` |
| `name` | no | Step name for placeholder references |
| `model` | no | Model name (from `models/*.yaml`). Falls back to `--default-model`. Supports `$ENV_VAR`. |
| `systemPrompt` | no | System prompt. Supports `$ENV_VAR` and `%placeholder%`. |
| `message` | yes | User message. Supports `$ENV_VAR` and `%placeholder%`. |
| `mcp` | no | List of MCP server references for tool calling. See [MCP Tools](#mcp-tools). |
| `maxToolIterations` | no | Max tool call loop iterations. Default `20`, set `"inf"` for unlimited. |
| `skip` | no | Skip condition based on flag. See [Skip Conditions](#skip-conditions). |

#### `output`

Return final text. Resolves placeholders in template.

| Field | Required | Description |
|---|---|---|
| `type` | yes | `output` |
| `name` | no | Step name |
| `output` | yes | Text template with `%placeholder%` references |

### Placeholders

Reference previous step results: `%stepName.field%`.

| Field | Source | Description |
|---|---|---|
| `%step.exitCode%` | `runCommand` | Exit code (integer) |
| `%step.stdout%` | `runCommand` | Standard output |
| `%step.stderr%` | `runCommand` | Standard error |
| `%step.stdoutStderr%` | `runCommand` | Concatenated stdout+stderr on success; empty on failure |
| `%step.success%` | `runCommand` | `true` if exit code 0, `false` otherwise |
| `%step.cwd%` | `runCommand` | Resolved working directory |
| `%step.message%` | `aiChat` | Assistant reply text |
| `%step.flag_name%` | `aiChat` | Flag value set by `rhd_set_flag` tool (`true` or `false`) |

Missing placeholders resolve to empty string (no error).

### MCP Tools

The `aiChat` action supports Model Context Protocol (MCP) for tool usage. When `mcp` field is present, the model can call tools in a loop until it responds without tool calls.

```yaml
- type: aiChat
  name: ai_step
  model: model_name
  mcp:
    - name: fs                    # Reference to mcp/fs/mcp.yaml
      args: ["--extra-arg"]       # Optional override
      env:
        AVAILABLE_ROOT: /tmp      # Optional env vars
    - name: flags                 # Built-in tools
  maxToolIterations: 20           # Optional, default 20, "inf" for unlimited
  systemPrompt: "Optional system prompt"
  message: "User message"
```

**Behavior**:
- Without `mcp` field: Single-shot mode (current behavior)
- With `mcp` field: Tool loop mode - model can call tools, results fed back, loop until `finish_reason: "stop"`
- Max iterations guard prevents infinite loops (configurable per step)

**Built-in Tools**:
- `rhd_set_flag`: Sets a flag that can be used for conditional step execution
  ```json
  {"name": "flag_name", "value": true}
  ```

**MCP Configuration** (`mcp/<name>/mcp.yaml`):
```yaml
cmd: npx
args: ["-y", "@modelcontextprotocol/server-filesystem", "$AVAILABLE_ROOT"]
cwd: null
```

**MCP Server Lifecycle**:
- Spawned on first use per (mcp_name, scenario_id, step_id)
- Cached at daemon level, reused across scenario executions
- Killed only on daemon shutdown

### Skip Conditions

Steps can be conditionally skipped based on flags set by `rhd_set_flag`:

```yaml
- type: runCommand
  name: build
  cmd: make
  skip: ai_step.flag_skip_build    # Skip if flag is true
```

**Flag Format**: `<aiChatStepName>.flag_<flagName>`
- Flags are stored in `ExecutionContext`
- Accessible to all subsequent steps
- If flag is `true` → skip step, if `false`/absent → execute
- Applies to all step types except `output`

### Environment variable substitution

All string fields support `$VAR_NAME` syntax (alphanumeric + underscore). Replaced at load time with environment variable value. If variable not set, original `$VAR_NAME` string kept as-is.

## Example scenario

```yaml
description: Just a test
actions:
  - type: runCommand
    name: cmd
    cmd: ./echo.sh
    args:
      - '21'
  - type: aiChat
    name: ai
    model: deepseek_v4_flash
    systemPrompt: >-
      You will receive number. Multiply it by 2 and respond only with result,
      append short funny description to this number.

      For example: "4 — looks definitely random!"
    message: "%cmd.stdout%"
  - type: aiChat
    name: ai2
    model: deepseek_v4_flash
    systemPrompt: >-
      You will receive number and short description of it. Translate it in Russian,
      then add one paragraph of text about it.
    message: "%ai.message%"
  - type: output
    name: out
    output: "Result: %ai2.message%"
```

Run:

```bash
rhd run example
```

## Behavior notes

- **runCommand never fails scenario**: Non-zero exit codes captured in `%step.exitCode%` and `%step.success%`, but execution continues.
- **Missing placeholders**: Resolve to empty string, not errors.
- **CWD propagation**: `rhd run` sends its cwd to daemon. Commands execute in client's cwd unless scenario `cwd` field overrides.
- **Socket cleanup**: Daemon removes stale socket file on startup.
- **Graceful shutdown**: Daemon handles SIGTERM/SIGINT for clean shutdown.
- **Model validation**: Daemon validates all models at startup, exits if invalid.
