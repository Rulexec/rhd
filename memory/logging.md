# Execution Logs

When `logs` is configured, each scenario execution creates a timestamped log directory:
- Format: `<logs>/<scenarioName>-YYYY-MM-DD-HH-MM-SS/`
- Collision handling: If directory exists, appends `-2`, `-3`, etc.
- Log file: `log.txt` inside the directory
- Metadata file: `meta.json` inside the directory (structured execution data)

## meta.json Format

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

## Log Format

Written to stdout and `log.txt`:

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

## Chat Logs

When `logChats` is configured, each chat interaction creates a timestamped log directory:
- Format: `<logChats>/<chatTitle>-YYYY-MM-DD-HH-MM-SS/`
- Collision handling: If directory exists, appends `-2`, `-3`, etc.
- Log file: `log.txt` inside the directory

### Chat Log Format

```
===== Chat "<title>" (id=<id>): stream started =====
model: <model>
available tools: <tool1>, <tool2>, ...

----- messages sent to API -----
[system] <content>
[user] <content>
[assistant] <content>

===== Assistant response =====
----- reasoning -----
<reasoning content>

----- message -----
<assistant message>

finish_reason: <stop|tool_calls|...>
tokens: prompt=X, completion=Y, total=Z

===== Tool call: <tool_name> (id=<call_id>) =====
<arguments JSON>

===== Tool result: <tool_name> (id=<call_id>) =====
<result content>

===== Stream finished =====
finish_reason: <reason>
total duration: <Xms>

===== Stream error =====
error: <error message>
```

- Full message history logged on each API call (not just new messages)
- Tool calls and results logged inline with call IDs for correlation
- Stream lifecycle markers (`stream started`, `stream finished`, `stream error`) for debugging stuck streams
- Reasoning/thinking content logged separately from message content
