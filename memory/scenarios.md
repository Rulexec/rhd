# Scenarios

## Scenario Execution

- Scenarios loaded from `scenarios/<name>/scenario.yaml`
- Three action types: `runCommand`, `aiChat`, `output`
- **runCommand never fails scenario** on non-zero exit (plan requirement)
- Execution context stores step results for cross-step placeholder resolution
- Placeholders: `%stepName.field%` where field is `exitCode`, `stdout`, `stderr`, `stdoutStderr`, `success`, `message`, `cwd`

## Scenario Format

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

## aiChat with MCP Tools

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

## Skip Conditions

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
