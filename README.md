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
| `--default-model NAME` | — | Fallback model for `aiChat` steps without `model` field (overrides config) |
| `--logs PATH` | — | Directory for execution logs (overrides config) |
| `--socket PATH` | `$HOME/rhd.sock` | Unix socket path |

## Configuration File

The daemon can be configured via a YAML file (default: `rhd.yaml` in current directory). CLI arguments override config file values.

**Config file format** (`rhd.yaml`):
```yaml
modelsDir: models
scenariosDir: scenarios
defaultModel: null
logs: null
```

- `modelsDir`: Directory containing model YAML files (default: `models`)
- `scenariosDir`: Directory containing scenario folders (default: `scenarios`)
- `defaultModel`: Fallback model for `aiChat` steps without `model` field (default: `null`)
- `logs`: Directory for execution logs (default: `null`, no logging)

## Execution Logs

When `logs` is configured, each scenario execution creates a timestamped log directory:
- Format: `<logs>/<scenarioName>-YYYY-MM-DD-HH-MM-SS/`
- Collision handling: If directory exists, appends `-2`, `-3`, etc.
- Log file: `log.txt` inside the directory

**Log format** (written to stdout and `log.txt`):
```
===== executing scenario =====
<scenarioName>

<stepName>: ===== running command =====
<command> <args>

<stepName>: ===== command output =====
[STDOUT] stdout line
[STDERR] stderr line

<stepName>: ===== command exit code =====
<code>

<stepName>: ===== AI request =====
model: <model>
----- system prompt -----
<prompt>
----- message -----
<message>

<stepName>: ===== AI response =====
<response>

===== output step =====
<resolved output>
```

### Run scenario

```bash
rhd run <scenario_name> [--socket PATH]
```

Client captures current working directory and sends it to daemon. Commands execute in client's cwd unless scenario overrides with `cwd` field.

## Models

Model configs: `models/<name>.yaml`. Filename (without extension) becomes model name.

| Field | Required | Description |
|---|---|---|
| `baseUrl` | yes | OpenAI-compatible API endpoint. Supports `$ENV_VAR`. |
| `apiKey` | yes | API key. Supports `$ENV_VAR`. |
| `model` | yes | Model identifier passed to API. Supports `$ENV_VAR`. |

Example `models/deepseek.yaml`:

```yaml
baseUrl: "https://api.deepseek.com/v1"
apiKey: "$DEEPSEEK_API_KEY"
model: "deepseek-chat"
```

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

Send message to AI model, receive response.

| Field | Required | Description |
|---|---|---|
| `type` | yes | `aiChat` |
| `name` | no | Step name for placeholder references |
| `model` | no | Model name (from `models/*.yaml`). Falls back to `--default-model`. Supports `$ENV_VAR`. |
| `systemPrompt` | no | System prompt. Supports `$ENV_VAR` and `%placeholder%`. |
| `message` | yes | User message. Supports `$ENV_VAR` and `%placeholder%`. |

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

Missing placeholders resolve to empty string (no error).

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
