# RHD Project Knowledge Base

## Project Overview

RHD is a Rust-based automation tool for AI-assisted task execution. It uses a daemon/client architecture where a long-running daemon process executes scenarios (action chains) on behalf of client requests via Unix socket IPC.

## Architecture

### Multi-Crate Workspace Structure

```
rhd/
├── Cargo.toml (workspace root)
├── plans/            # Implementation plans
├── packages/
│   ├── rhd_util/     # Shared error types, utilities, env var substitution
│   ├── rhd_ai/       # OpenAI-compatible AI client
│   ├── rhd_app/      # Main binary (daemon + client)
│   └── rhd_test/     # E2E test runner with mock AI server
```

### Core Components

**rhd_util**: Shared error types (`RhdError`, `RhdResult<T>`), `substitute_env_vars()` for `$VAR` expansion in config strings

**rhd_ai**: 
- `ModelConfig`: AI model configuration (baseUrl, apiKey, model)
- `OpenAiClient`: HTTP client for chat completions API
- Loads models from `models/*.yaml` at startup

**rhd_app**:
- **Daemon mode**: Unix socket server on `$HOME/rhd.sock` (default), accepts `RunScenario` requests
- **Client mode**: Connects to daemon, sends scenario name, receives output
- **Scenario executor**: Runs action chains sequentially with placeholder resolution
- **IPC protocol**: rkyv serialization with version-prefixed framing

## Key Design Decisions

### Scenario Execution
- Scenarios loaded from `scenarios/<name>/scenario.yaml`
- Three action types: `runCommand`, `aiChat`, `output`
- **runCommand never fails scenario** on non-zero exit (plan requirement)
- Execution context stores step results for cross-step placeholder resolution
- Placeholders: `%stepName.field%` where field is `exitCode`, `stdout`, `stderr`, `stdoutStderr`, `success`, `message`, `cwd`

### Environment Variable Substitution

Model configs and scenario YAMLs support `$ENV_VAR` syntax in string values. At load time, all `$VAR_NAME` patterns (alphanumeric + underscore) are replaced with the corresponding environment variable value. If the variable is not set, the original `$VAR_NAME` string is kept as-is.

Example:
```yaml
baseUrl: "http://localhost:$E2E_MODEL_PORT/v1"
cmd: "$E2E_SCRIPTS_DIR/run.sh"
```

### Configuration Formats

**Model config** (`models/*.yaml`):
```yaml
baseUrl: "https://api.openai.com/v1"
apiKey: "sk-..."
model: "gpt-4"
```

**Scenario** (`scenarios/<name>/scenario.yaml`):
```yaml
description: Optional description
actions:
  - type: runCommand
    name: step_name
    cmd: command
    args: ["arg1", "arg2"]
    cwd: /optional/working/dir
  - type: aiChat
    name: ai_step
    model: model_name
    systemPrompt: "Optional system prompt"
    message: "User message with %placeholders%"
  - type: output
    name: output_step
    output: "Final output with %stepName.message%"
```

### IPC Protocol
- Unix socket at `$HOME/rhd.sock` by default (configurable via `--socket`)
- Message format: 4-byte version + 4-byte length + rkyv payload
- Protocol version: 1
- Request: `IpcRequest::RunScenario { name: String, cwd: String }`
- Response: `IpcResponse::Success { output: String }` or `IpcResponse::Error { message: String }`

### CWD Propagation
- `rhd run` captures its current working directory and sends it to the daemon via IPC
- Commands execute in the client's cwd by default (when `cwd` not explicitly set in scenario YAML)
- If `cwd` is set in scenario YAML, it takes precedence over client's cwd
- The resolved cwd for each `runCommand` step is stored in `StepResult.cwd` and accessible via `%stepName.cwd%` placeholder
- E2E tests run daemon and client in separate directories to verify cwd propagation works correctly

### Error Handling
- Daemon stays alive on scenario errors
- Client exits with code 0 on success, 1 on error
- All errors include context (file path, line number, step name)
- Model validation at daemon startup (exits if invalid)

## CLI Usage

```bash
# Start daemon
rhd daemon [--config rhd.yaml] [--models-dir models] [--scenarios-dir scenarios] [--default-model name] [--logs logs] [--socket PATH]

# Run scenario
rhd run <scenario_name> [--socket PATH]
```

By default, the socket is located at `$HOME/rhd.sock`. The `--socket` flag allows specifying a custom socket path.

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

### aiChat with MCP Tools

The `aiChat` action supports Model Context Protocol (MCP) for tool usage:

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

## Execution Logs

When `logs` is configured, each scenario execution creates a timestamped log directory:
- Format: `<logs>/<scenarioName>-YYYY-MM-DD-HH-MM-SS/`
- Collision handling: If directory exists, appends `-2`, `-3`, etc.
- Log file: `log.txt` inside the directory

**Log format** (written to stdout and `log.txt`):
```
===== <scenarioName>: executing scenario =====

===== <stepName>: running command =====
<command> <args>

----- <stepName>: command exit code -----
<code>

----- <stepName>: command output -----
[STDOUT] stdout line
[STDERR] stderr line

===== <stepName>: AI request =====
model: <model>
available tools: <tool1>, <tool2>, ...   (only when MCP tools configured)
----- system prompt -----
<prompt>
----- message -----
<message>

===== <stepName>: AI response =====
<response>

----- <stepName>: tool call -----
<toolName>(<arguments>)

----- <stepName>: tool result -----
<result>

===== <stepName>: skipped =====
<skip expression>

===== <stepName>: output step =====
<resolved output>
```

## Development Practices

### Planning
- Implementation plans saved to `plans/` folder as markdown files
- Plan naming: `<feature>-plan.md` or `<feature>-plan-<n>.md` for iterations
- Plans should include: goal, architecture, implementation steps, file changes, risks, success criteria

### Code Organization
- All crates prefixed with `rhd_`
- Shared dependencies managed in workspace root `Cargo.toml`
- Strict YAML parsing with `deny_unknown_fields`
- Field names use camelCase in YAML, snake_case in Rust structs (via `#[serde(rename_all = "camelCase")]`)

### Testing
- E2E tests via `rhd_test` crate: `cargo run -p rhd_test [-- --seed <N> --repetitions <N>]`
- `rhd_test` accepts `--seed` (default 42) for deterministic random generation and `--repetitions` (default 10) to run tests in loop
- Each iteration uses seed `base_seed + i`, prints iteration seed for reproducibility on failure
- `rhd_test` starts a mock OpenAI-compatible HTTP server (axum, reused across iterations), spawns daemon per iteration, runs scenario, validates AI request payloads and output
- Test scenarios in `test_e2e/scenarios/<name>/scenario.yaml`
- Test models in `test_e2e/models/*.yaml`

### Build & Validation
- `cargo build` for compilation
- `cargo test` for unit tests
- Daemon validates models and scenarios at startup

### Committing
- Commit messages should be short and descriptive, inferred from the work completed
- Format: lowercase, no period, concise summary of changes
- Examples: "add seeded rng for e2e tests", "fix placeholder resolution bug", "update daemon shutdown logic"
- Always use `git add -A` to stage all changes before committing

## Important Conventions

1. **Placeholder resolution**: Missing values resolve to empty string, not errors
2. **runCommand behavior**: Captures exit code + stdout/stderr, never fails scenario
3. **aiChat behavior**: Resolves placeholders in systemPrompt and message before API call
4. **output behavior**: Resolves placeholders in template, returns final string
5. **Model loading**: Filename (without extension) becomes model name in HashMap
6. **Scenario loading**: Directory name is scenario identifier (used as HashMap key), `scenario.yaml` contains definition
7. **Socket cleanup**: Daemon removes stale socket file on startup
8. **Graceful shutdown**: Daemon handles SIGTERM/SIGINT for clean shutdown
9. **CWD propagation**: `rhd run` sends its cwd to daemon; commands execute in client's cwd unless overridden in scenario
10. **Socket path**: Default socket location is `$HOME/rhd.sock`; both daemon and client accept `--socket` flag for custom location

## File Structure Reference

```
packages/rhd_app/src/
├── main.rs           # CLI entry point, command dispatch
├── cli.rs            # clap argument definitions
├── config.rs         # DaemonConfig YAML loading
├── daemon.rs         # Unix socket server, connection handling
├── client.rs         # Unix socket client
├── log.rs            # LogSink, execution logging
├── ipc/
│   ├── mod.rs
│   └── protocol.rs   # rkyv message types, read/write helpers
└── scenario/
    ├── mod.rs        # Action/Scenario structs
    ├── loader.rs     # YAML loading, validation
    ├── executor.rs   # Action execution engine
    └── placeholder.rs # Placeholder resolution, ExecutionContext

packages/rhd_ai/src/
├── lib.rs
├── config.rs         # ModelConfig, load_models()
└── client.rs         # OpenAiClient, AiError

packages/rhd_util/src/
└── lib.rs            # RhdError, RhdResult, substitute_env_vars()

packages/rhd_test/src/
└── main.rs           # E2E test runner: mock AI server, daemon spawn, validation
```
