# MCP Protocol Support Plan

## Goal

Add MCP (Model Context Protocol) support to RHD, enabling `aiChat` steps to use tools via external MCP servers and built-in tool implementations. Introduce `rhd_set_flag` built-in tool for cross-step conditional execution via `skip` property.

## Design Decisions

### Behavior Modes

| Mode | Condition | Behavior |
|------|-----------|----------|
| **Single-shot** | `mcp` field absent | Current behavior: send system prompt + message, receive single response |
| **Tool loop** | `mcp` field present | Loop: send request → model returns tool_calls → execute tools → feed results back → repeat until `finish_reason: "stop"` |

### MCP Server Lifecycle

- Spawn on first use (when step with given MCP config runs for first time)
- Cache at **daemon level** — reused across scenario executions for same (mcp_name, scenario_id, step_id)
- Kill only on daemon shutdown
- Key for cache: `(mcp_name, scenario_id, step_id)`

### Flags

- Bound to `aiChat` step name: `ai1.flag_example`, `ai2.flag_example` are independent
- Stored in `ExecutionContext` (accessible to all subsequent steps)
- `skip: <aiStepName>.flag_<flagName>` — if flag is `true` → skip step, if `false`/absent → execute
- Applies to all step types except `output`

### Error Handling

- MCP tool errors → returned as tool result content (model decides how to handle)
- Max iterations guard: configurable per step via `maxToolIterations` field (default: 20, set to `"inf"` for unlimited) → fail scenario with error when limit reached
- MCP server crash → return error as tool result

### Logging

- Log each tool call: name, arguments
- Log each tool result: content (full, no truncation)

## Architecture

### New Crate: `rhd_mcp_client`

```
packages/rhd_mcp_client/
├── Cargo.toml
└── src/
    ├── lib.rs           # Public API: McpClient trait, ToolDefinition, ToolResult
    ├── protocol.rs      # JSON-RPC 2.0 types (Request, Response, Error)
    ├── transport.rs     # Stdio transport (spawn process, read/write JSON lines)
    ├── client.rs        # McpClient impl: initialize, list_tools, call_tool
    └── builtin.rs       # Built-in tools registry (rhd_set_flag)
```

### MCP Config Loading

```
mcp/
├── fs/
│   └── mcp.yaml     # { cmd: "npx", args: ["-y", "@modelcontextprotocol/server-filesystem", "$AVAILABLE_ROOT"], cwd: null }
└── flags/
    └── mcp.yaml     # Built-in marker (no cmd needed) or omitted entirely
```

### Scenario Schema Extensions

```yaml
actions:
  - type: aiChat
    name: ai1
    model: gpt-4
    maxToolIterations: 20          # optional, default 20, "inf" for unlimited
    mcp:
      - name: fs                          # reference to mcp/fs/mcp.yaml
        env:
          AVAILABLE_ROOT: /home/ruliov/Trash
        args: ["--extra-arg"]             # optional override
      - name: flags                       # built-in tools (no external server)
    systemPrompt: "..."
    message: "..."

  - type: runCommand
    name: build
    cmd: make
    skip: ai1.flag_skip_build             # skip if ai1 set flag_skip_build to true
```

### Daemon Changes

- Add `McpServerCache` to daemon state
- Cache key: `(mcp_name, scenario_id, step_id)` → `McpClient` instance
- On daemon shutdown: kill all cached MCP server processes
- Pass cache reference to executor

### Executor Changes

```rust
// Pseudocode for aiChat step with MCP
async fn execute_ai_chat(step, context, mcp_cache) {
    let tools = if let Some(mcp_configs) = &step.mcp {
        // Collect tools from all MCP sources
        let mut all_tools = Vec::new();
        for mcp_config in mcp_configs {
            let client = mcp_cache.get_or_spawn(mcp_config).await?;
            all_tools.extend(client.list_tools().await?);
        }
        all_tools
    } else {
        vec![]  // single-shot mode
    };

    let max_iterations = step.max_tool_iterations.unwrap_or(20);
    let mut messages = vec![system_message, user_message];
    let mut iterations = 0;

    loop {
        if max_iterations != "inf" && iterations >= max_iterations {
            return Err("max tool call iterations exceeded");
        }

        let response = openai_client.chat(&messages, &tools).await?;
        messages.push(response.message.clone());

        match &response.message.tool_calls {
            Some(calls) if !calls.is_empty() => {
                for call in calls {
                    let result = execute_tool_call(call, &step.mcp, mcp_cache, context).await;
                    log_tool_call(call, &result);
                    messages.push(Message::tool_result(&call.id, result));
                }
                iterations += 1;
            }
            _ => break response.message.content,  // finish_reason: "stop"
        }
    }
}
```

### Tool Execution Routing

```rust
async fn execute_tool_call(call, mcp_configs, mcp_cache, context) -> String {
    // Check if built-in tool
    if call.function.name.starts_with("rhd_") {
        return execute_builtin_tool(call, context).await;
    }

    // Find which MCP server provides this tool
    for mcp_config in mcp_configs {
        let client = mcp_cache.get_or_spawn(mcp_config).await;
        if client.has_tool(&call.function.name).await {
            return client.call_tool(&call.function.name, &call.function.arguments).await;
        }
    }

    format!("Error: unknown tool '{}'", call.function.name)
}
```

### Built-in Tool: `rhd_set_flag`

```rust
// Input schema
{
    "name": "rhd_set_flag",
    "description": "Set a named flag with a boolean value",
    "inputSchema": {
        "type": "object",
        "properties": {
            "name": { "type": "string", "description": "Flag name" },
            "value": { "type": "boolean", "default": true }
        },
        "required": ["name"]
    }
}

// Execution
fn execute_rhd_set_flag(args, context, step_name) {
    let flag_name = format!("{}.flag_{}", step_name, args.name);
    let value = args.value.unwrap_or(true);
    context.set_flag(flag_name, value);
    format!("Flag '{}' set to {}", flag_name, value)
}
```

### Skip Evaluation

```rust
// Before executing any step (except output)
// skip expression format: "<aiChatStepName>.flag_<flagName>"
// e.g., "ai1.flag_skip_build" — skip if aiChat step named "ai1" set flag "skip_build" to true
if let Some(skip_expr) = &step.skip {
    // skip_expr is like "ai1.flag_skip_build"
    if context.get_flag(skip_expr) == Some(true) {
        log_skip(step.name, skip_expr);
        return Ok(StepResult::skipped());
    }
}
```

### Placeholder Resolution Extension

Extend `ExecutionContext` to support `<aiChatStepName>.flag_<flagName>`:

```rust
impl ExecutionContext {
    fn resolve_placeholder(&self, key: &str) -> Option<String> {
        // Existing: stepName.stdout, stepName.exitCode, etc.
        // New: <aiChatStepName>.flag_<flagName>
        // Flags are stored with full path key like "ai1.flag_skip_build"
        self.flags.get(key).map(|v| v.to_string())
            .or_else(|| /* existing logic */)
    }
}
```

## File Changes

### New Files

| File | Purpose |
|------|---------|
| `packages/rhd_mcp_client/Cargo.toml` | Crate manifest |
| `packages/rhd_mcp_client/src/lib.rs` | Public API |
| `packages/rhd_mcp_client/src/protocol.rs` | JSON-RPC types |
| `packages/rhd_mcp_client/src/transport.rs` | Stdio transport |
| `packages/rhd_mcp_client/src/client.rs` | MCP client impl |
| `packages/rhd_mcp_client/src/builtin.rs` | Built-in tools |
| `packages/rhd_app/src/mcp_cache.rs` | Daemon-level MCP server cache |
| `test_e2e/scenarios/mcp_test/scenario.yaml` | E2E test scenario |

### Modified Files

| File | Changes |
|------|---------|
| `Cargo.toml` | Add `rhd_mcp_client` to workspace |
| `packages/rhd_app/Cargo.toml` | Add `rhd_mcp_client` dependency |
| `packages/rhd_app/src/scenario/mod.rs` | Add `mcp`, `skip`, `maxToolIterations` fields to Action |
| `packages/rhd_app/src/scenario/loader.rs` | Load MCP configs, validate schema |
| `packages/rhd_app/src/scenario/executor.rs` | Tool call loop, skip evaluation, flag storage |
| `packages/rhd_app/src/scenario/placeholder.rs` | Extend for `<aiStep>.flag_<name>` |
| `packages/rhd_app/src/daemon.rs` | Add McpServerCache, shutdown cleanup |
| `packages/rhd_app/src/config.rs` | Add `mcpDir` config (default: `mcp`) |
| `packages/rhd_app/src/log.rs` | Add tool call/result logging |
| `packages/rhd_app/src/cli.rs` | Add `--mcp-dir` flag |
| `AI.md` | Document MCP support |

## Implementation Steps

### Phase 1: Core MCP Client

1. Create `rhd_mcp_client` crate with Cargo.toml
2. Implement JSON-RPC 2.0 protocol types (Request, Response, Error)
3. Implement stdio transport (spawn process, line-delimited JSON)
4. Implement MCP client: `initialize()`, `tools/list`, `tools/call`
5. Add unit tests for protocol serialization

### Phase 2: Built-in Tools

6. Implement built-in tools registry in `rhd_mcp_client/src/builtin.rs`
7. Implement `rhd_set_flag` tool
8. Add `ToolDefinition` and `ToolResult` types to lib.rs

### Phase 3: Schema Extensions

9. Add `mcp` field to `AiChatAction` struct
10. Add `skip` field to all action types (except output)
11. Add `maxToolIterations` field to `AiChatAction` struct
12. Implement MCP config loading from `mcp/<name>/mcp.yaml`
13. Implement inline config override (args, env)
14. Add env var substitution to MCP args

### Phase 4: Daemon Integration

15. Create `McpServerCache` struct in `rhd_app/src/mcp_cache.rs`
16. Add cache to daemon state
17. Implement `get_or_spawn()` keyed by `(mcp_name, scenario_id, step_id)`
18. Add shutdown hook to kill all cached servers

### Phase 5: Executor Changes

19. Implement tool call loop in `execute_ai_chat()`
20. Implement tool routing (built-in vs external MCP)
21. Add max iterations guard (configurable, default 20)
22. Implement flag storage in `ExecutionContext`
23. Implement skip evaluation before each step
24. Extend placeholder resolution for `<aiStep>.flag_<name>`

### Phase 6: Logging

25. Add tool call logging (name, arguments)
26. Add tool result logging (content)
27. Add skip logging

### Phase 7: Testing

28. Write E2E test for built-in `rhd_set_flag` tool
29. Write E2E test for skip functionality
30. Write E2E test for external MCP server (mock)
31. Update AI.md with MCP documentation

## Log Format Extensions

```
===== <stepName>: AI request =====
model: <model>
----- system prompt -----
<prompt>
----- message -----
<message>
----- tools -----
<tool1_name>
<tool2_name>

===== <stepName>: AI response =====
<response>

----- <stepName>: tool call -----
name: <tool_name>
arguments: <json>

----- <stepName>: tool result -----
<content>

(repeat tool call/result pairs as needed)

===== <stepName>: AI final response =====
<final response after all tool calls resolved>
```

## Risks

1. **MCP server hangs**: Stdio reads may block indefinitely → add timeout to transport
2. **Tool name collisions**: Multiple MCP servers may expose same tool name → first match wins, log warning
3. **Flag naming conflicts**: Two aiChat steps with same name → flags merge (user responsibility to use unique names)
4. **Infinite tool loops**: Model keeps calling tools → max iterations guard (configurable, default 20)
5. **MCP server resource leaks**: Daemon crash leaves orphan processes → use process groups (`setsid()` on Unix) so all child processes are terminated when daemon kills the MCP server process group. On Unix, spawn MCP server in its own process group via `Command::pre_exec(|| setsid())`, then kill entire group with `kill(-pid, SIGKILL)`. This ensures any grandchildren spawned by the MCP server are also cleaned up.

## Success Criteria

- [ ] `aiChat` without `mcp` works exactly as before (backward compatible)
- [ ] `aiChat` with `mcp: [{ name: 'flags' }]` can call `rhd_set_flag` tool
- [ ] Flags set by `rhd_set_flag` are accessible via `<aiStepName>.flag_<name>` placeholder
- [ ] `skip: <aiStepName>.flag_<name>` skips step when flag is true
- [ ] External MCP servers can be spawned and called
- [ ] MCP servers cached at daemon level, reused across scenarios
- [ ] Tool calls and results logged in execution logs
- [ ] Max iterations guard prevents infinite loops (configurable per step)
- [ ] E2E tests pass for all new functionality
