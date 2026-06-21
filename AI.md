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
- **Daemon mode**: Unix socket server on `rhd.sock`, accepts `RunScenario` requests
- **Client mode**: Connects to daemon, sends scenario name, receives output
- **Scenario executor**: Runs action chains sequentially with placeholder resolution
- **IPC protocol**: rkyv serialization with version-prefixed framing

## Key Design Decisions

### Scenario Execution
- Scenarios loaded from `scenarios/<name>/scenario.yaml`
- Three action types: `runCommand`, `aiChat`, `output`
- **runCommand never fails scenario** on non-zero exit (plan requirement)
- Execution context stores step results for cross-step placeholder resolution
- Placeholders: `%stepName.field%` where field is `exitCode`, `stdout`, `stderr`, `stdoutStderr`, `success`, `message`

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
name: scenario_name
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
- Unix socket at `./rhd.sock`
- Message format: 4-byte version + 4-byte length + rkyv payload
- Protocol version: 1
- Request: `IpcRequest::RunScenario { name: String }`
- Response: `IpcResponse::Success { output: String }` or `IpcResponse::Error { message: String }`

### Error Handling
- Daemon stays alive on scenario errors
- Client exits with code 0 on success, 1 on error
- All errors include context (file path, line number, step name)
- Model validation at daemon startup (exits if invalid)

## CLI Usage

```bash
# Start daemon
rhd daemon [--models-dir models] [--scenarios-dir scenarios] [--default-model name]

# Run scenario
rhd run <scenario_name>
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

## Important Conventions

1. **Placeholder resolution**: Missing values resolve to empty string, not errors
2. **runCommand behavior**: Captures exit code + stdout/stderr, never fails scenario
3. **aiChat behavior**: Resolves placeholders in systemPrompt and message before API call
4. **output behavior**: Resolves placeholders in template, returns final string
5. **Model loading**: Filename (without extension) becomes model name in HashMap
6. **Scenario loading**: Directory name is scenario identifier, `scenario.yaml` contains definition
7. **Socket cleanup**: Daemon removes stale `rhd.sock` on startup
8. **Graceful shutdown**: Daemon handles SIGTERM/SIGINT for clean shutdown

## File Structure Reference

```
packages/rhd_app/src/
├── main.rs           # CLI entry point, command dispatch
├── cli.rs            # clap argument definitions
├── daemon.rs         # Unix socket server, connection handling
├── client.rs         # Unix socket client
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
