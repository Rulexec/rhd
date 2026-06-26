# RHD Project Knowledge Base

## Project Overview

RHD is a Rust-based automation tool for AI-assisted task execution. It uses a daemon/client architecture where a long-running daemon process executes scenarios (action chains) on behalf of client requests via Unix socket IPC.

## Architecture

### Multi-Crate Workspace Structure

```
rhd/
├── Cargo.toml (workspace root)
├── plans/            # Implementation plans
├── frontend/         # Svelte web UI
├── packages/
│   ├── rhd_util/     # Shared error types, utilities, env var substitution
│   ├── rhd_ai/       # OpenAI-compatible AI client
│   ├── rhd_api/      # Shared IPC types, protocol definitions, execution tracking types
│   ├── rhd_db/       # SQLite database for scenario ID persistence
│   ├── rhd_mcp_client/ # MCP protocol client for tool usage
│   ├── rhd_app/      # Main binary (daemon + client)
│   └── rhd_test/     # E2E test runner with mock AI server
```

### Core Components

**rhd_util**: Shared error types (`RhdError`, `RhdResult<T>`), `substitute_env_vars()` for `$VAR` expansion in config strings

**rhd_ai**:
- `ModelConfig`: AI model configuration (baseUrl, apiKey, model, optional token pricing)
- `OpenAiClient`: HTTP client for chat completions API
- Loads models from `models/*.yaml` at startup
- Parses token usage from API responses

**rhd_api**:
- Shared types for execution tracking, WebSocket protocol, and token pricing
- `ExecutionEvent`, `StepTiming`, `LogSection`, `TokenUsage`, `ScenarioMeta`
- `WsRequest`, `WsResponse`, `WsEvent`, `ErrorCode` for WebSocket protocol
- `TokenPriceTier` and `calculate_cost()` for token pricing

**rhd_db**:
- `ScenarioDb`: SQLite database wrapper for persisting scenario execution IDs
- Uses WAL mode for better concurrency and crash recovery
- Thread-safe via `Mutex<Connection>`
- `next_id()`: Atomically retrieves and increments the next scenario ID
- Database file: `<dbDir>/meta.db` (default: `rhd_db/meta.db`)

**rhd_mcp_client**:
- `McpConfig`: MCP server configuration (cmd, args, cwd, env)
- `McpClient`: MCP client implementation with stdio transport
- `ToolDefinition`, `ToolResult`: Tool types for MCP protocol
- `McpClientTrait`: Trait for MCP client implementations
- Built-in tools support (e.g., `rhd_set_flag`)

**rhd_app**:
- **Daemon mode**: Unix socket server on `$HOME/rhd.sock` (default), accepts `RunScenario` requests
- **WebSocket server**: Optional TCP listener on `127.0.0.1:{ws_port}` for Web UI integration
- **Client mode**: Connects to daemon, sends scenario name, receives output
- **Scenario executor**: Runs action chains sequentially with placeholder resolution
- **Execution tracking**: Tracks step timings, token usage, log sections
- **IPC protocol**: rkyv serialization with version-prefixed framing (Unix socket)
- **WebSocket protocol**: JSON over WebSocket (TCP)
- **MCP integration**: Loads MCP configs, caches server instances, handles tool calls

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
apiKey: "sk-..."          # Plain string, credential reference, or env var
model: "gpt-4"
inputTokenPrice: 5.0      # Optional: price per 1M tokens
outputTokenPrice: 15.0    # Optional: price per 1M tokens
priceTiers:               # Optional: tiered pricing
  - afterTokens: 250000
    inputTokenPrice: 10.0
    outputTokenPrice: 30.0
```

**API Key Forms**:
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
- Response: `IpcResponse::Success { output: String }`, `IpcResponse::Error { message: String }`, or `IpcResponse::Aborted`

### WebSocket Protocol
- Optional TCP listener on `127.0.0.1:{ws_port}` (configurable via `--ws-port` or `wsPort` in config)
- JSON over WebSocket for web-friendly integration
- **Client → Server requests**:
  - `runScenario`: Execute a scenario
  - `subscribe`: Subscribe to execution events (returns list of currently active executions)
  - `getFinishedScenarios`: Get list of finished scenarios from meta.json. Accepts optional `lastId` parameter to fetch only scenarios with id > lastId (for incremental updates)
  - `abortScenario`: Abort an active scenario execution by execution ID
- **Server → Client responses**: Request responses with success/error status
- **Server → Client events**: Real-time execution events (scenarioStarted, stepStarted, scenarioFinished)
  - `scenarioFinished` event data uses same `ScenarioMeta` format as `getFinishedScenarios` response items
- Multiple subscribers supported via broadcast channel

### CWD Propagation
- `rhd run` captures its current working directory and sends it to the daemon via IPC
- Commands execute in the client's cwd by default (when `cwd` not explicitly set in scenario YAML)
- If `cwd` is set in scenario YAML, it takes precedence over client's cwd
- The resolved cwd for each `runCommand` step is stored in `StepResult.cwd` and accessible via `%stepName.cwd%` placeholder
- E2E tests run daemon and client in separate directories to verify cwd propagation works correctly

### Error Handling
- Daemon stays alive on scenario errors
- Client exits with code 0 on success, 1 on error, 2 on abort
- All errors include context (file path, line number, step name)
- Model validation at daemon startup (exits if invalid)

### Scenario ID Persistence
- Scenario execution IDs are persisted in SQLite database to survive daemon restarts
- Database location: `<dbDir>/meta.db` (default: `rhd_db/meta.db`)
- Single table `meta` with column `nextScenarioId` (INTEGER)
- IDs are atomically incremented using SQLite transactions
- WAL mode enabled for better concurrency and crash recovery
- Database directory is created automatically if it doesn't exist
- Fails fast if database cannot be opened or accessed

## CLI Usage

```bash
# Start daemon
rhd daemon [--config rhd.yaml] [--models-dir models] [--scenarios-dir scenarios] [--mcp-dir mcp] [--default-model name] [--logs logs] [--socket PATH] [--ws-port PORT] [--db-dir DIR]

# Run scenario
rhd run <scenario_name> [--socket PATH]
```

By default, the socket is located at `$HOME/rhd.sock`. The `--socket` flag allows specifying a custom socket path.
The `--ws-port` flag enables WebSocket server on the specified port (optional).
The `--db-dir` flag specifies the directory for the SQLite database (default: `rhd_db`).

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

### Credentials Configuration

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
- Metadata file: `meta.json` inside the directory (structured execution data)

### meta.json Format

```json
{
  "id": 1,
  "scenario": "my_scenario",
  "status": "success",
  "started": "2026-06-26T15:00:00Z",
  "finished": "2026-06-26T15:01:30Z",
  "durationMs": 90000,
  "tokens": {
    "prompt": 1500,
    "completion": 800,
    "total": 2300
  },
  "cost": 0.0235,
  "steps": [
    {
      "name": "build",
      "type": "runCommand",
      "exitCode": 0,
      "started": "2026-06-26T15:00:00Z",
      "finished": "2026-06-26T15:00:10Z",
      "durationMs": 10000,
      "sections": [
        { "kind": "runningCommand", "startLine": 5, "endLine": 6 },
        { "kind": "exitCode", "startLine": 8, "endLine": 9 },
        { "kind": "commandOutput", "startLine": 11, "endLine": 15 }
      ]
    },
    {
      "name": "ai_step",
      "type": "aiChat",
      "model": "gpt-4",
      "started": "2026-06-26T15:00:10Z",
      "finished": "2026-06-26T15:00:20Z",
      "durationMs": 10000,
      "tokens": { "prompt": 500, "completion": 200, "total": 700 },
      "cost": 0.005,
      "sections": [...]
    },
    {
      "name": "output_step",
      "type": "output",
      "started": "2026-06-26T15:00:20Z",
      "finished": "2026-06-26T15:00:20Z",
      "durationMs": 0,
      "sections": [...]
    }
  ]
}
```

- `id` field contains the execution ID (unique per scenario execution)
- `status` field contains execution status: `executing`, `success`, `error`, or `aborted`
- Every step includes `type` field: `runCommand`, `aiChat`, or `output`
- `runCommand` steps include `exitCode` field
- `aiChat` steps include `model` field
- `tokens` and `cost` fields omitted at scenario level if no AI steps
- Per-step `tokens` and `cost` omitted for non-AI steps
- `sections` array contains line ranges for all delimited blocks within the step
- All timestamps are ISO 8601 (local timezone for log directory names, UTC for meta.json fields)

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

===== ABORTED =====
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

1. **Placeholder resolution**: Missing values resolve to empty string, not errors. Exception: flag placeholders (`%step.flag_name%`) resolve to "false" when flag not set
2. **runCommand behavior**: Captures exit code + stdout/stderr, never fails scenario
3. **aiChat behavior**: Resolves placeholders in systemPrompt and message before API call
4. **output behavior**: Resolves placeholders in template, returns final string
5. **Model loading**: Filename (without extension) becomes model name in HashMap
6. **Scenario loading**: Directory name is scenario identifier (used as HashMap key), `scenario.yaml` contains definition
7. **Socket cleanup**: Daemon removes stale socket file on startup
8. **Graceful shutdown**: Daemon handles SIGTERM/SIGINT for clean shutdown
9. **CWD propagation**: `rhd run` sends its cwd to daemon; commands execute in client's cwd unless overridden in scenario
10. **Socket path**: Default socket location is `$HOME/rhd.sock`; both daemon and client accept `--socket` flag for custom location

## Frontend

Svelte-based web UI in `frontend/` directory for monitoring scenario execution.

### Setup
- Requires Node.js v24.13.0 (specified in `.nvmrc`)
- Start with `nvm use && npm run start`
- Connects to daemon WebSocket server (default port 9876, configurable via `VITE_WS_PORT` env var)

### Features
- **Scenarios tab**:
  - Shows active scenarios with real-time updates (current step, elapsed time, token counts)
  - Abort button to stop active scenario execution
  - Shows finished scenarios list (sorted by date, newest first) with status badges
  - Status badges: executing (blue), success (green), error (red), aborted (orange)
  - Caches finished scenarios; uses `lastId` parameter for incremental fetching
- **Chats tab**: Placeholder (not implemented yet)

### Architecture
- WebSocket connection with auto-reconnect
- Svelte stores for state management (`activeScenarios`, `finishedScenarios`, `lastKnownId`, `wsConnected`)
- CSS modules + utility classes (Tailwind-like approach)
- Components: `TabNav`, `ScenariosTab`, `ChatsTab`, `ActiveScenario`, `FinishedScenario`

## File Structure Reference

```
frontend/
├── .nvmrc              # Node.js version (v24.13.0)
├── package.json        # Dependencies and scripts
├── vite.config.js      # Vite configuration
├── index.html          # Entry HTML
├── svelte.config.js    # Svelte configuration
└── src/
    ├── main.js         # App entry point
    ├── App.svelte      # Root component
    ├── lib/
    │   ├── ws.js       # WebSocket connection service
    │   ├── stores.js   # Svelte stores for state
    │   └── utils.js    # Helper functions
    ├── components/
    │   ├── TabNav.svelte
    │   ├── ScenariosTab.svelte
    │   ├── ChatsTab.svelte
    │   ├── ActiveScenario.svelte
    │   └── FinishedScenario.svelte
    └── styles/
        ├── global.css
        ├── utilities.css
        └── components/

packages/rhd_app/src/
├── main.rs           # CLI entry point, command dispatch
├── cli.rs            # clap argument definitions
├── config.rs         # DaemonConfig YAML loading
├── daemon.rs         # Unix socket server, WebSocket server, connection handling
├── client.rs         # Unix socket client
├── execution.rs      # ExecutionTracker, ExecutionHandle, execution tracking
├── ws.rs             # WebSocket server, JSON protocol handlers
├── log.rs            # LogSink, execution logging, meta.json writing/reading
├── mcp_cache.rs      # MCP server instance caching
├── mcp_loader.rs     # MCP config loading from mcp/<name>/mcp.yaml
├── ipc/
│   ├── mod.rs
│   └── protocol.rs   # rkyv message types, read/write helpers
└── scenario/
    ├── mod.rs        # Action/Scenario structs
    ├── loader.rs     # YAML loading, validation
    ├── executor.rs   # Action execution engine
    ├── error.rs      # ExecuteError, ExecuteOutput types
    ├── ai_chat.rs    # AI chat execution with token tracking
    ├── run_command.rs # Command execution with section tracking
    └── placeholder.rs # Placeholder resolution, ExecutionContext

packages/rhd_ai/src/
├── lib.rs
├── config.rs         # ModelConfig with optional token pricing, load_models()
└── client.rs         # OpenAiClient, AiError, token usage parsing

packages/rhd_api/src/
└── lib.rs            # Shared types: ExecutionEvent, StepTiming, LogSection, TokenUsage, ScenarioMeta, WsRequest, WsResponse, WsEvent, ErrorCode, TokenPriceTier

packages/rhd_util/src/
└── lib.rs            # RhdError, RhdResult, substitute_env_vars()

packages/rhd_db/src/
└── lib.rs            # ScenarioDb, DbError, SQLite wrapper for ID persistence

packages/rhd_mcp_client/src/
├── lib.rs            # McpConfig, ToolDefinition, ToolResult, McpClientTrait
├── client.rs         # MCP client implementation
├── protocol.rs       # MCP JSON-RPC protocol types
├── transport.rs      # stdio transport for MCP servers
└── builtin.rs        # Built-in tools (rhd_set_flag)

packages/rhd_test/src/
├── main.rs           # E2E test runner: mock AI server, daemon spawn, validation
├── mock_server.rs    # Mock OpenAI-compatible server with token usage
├── standard_test.rs  # Standard test with meta.json validation
└── mcp_test.rs       # MCP tool test
```
