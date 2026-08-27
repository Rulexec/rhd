# Logging & Monitoring

## Purpose
Execution logging to files and stdout, structured metadata for completed scenarios, real-time monitoring via WebSocket events, and desktop/browser notifications for scenario state changes.

## Execution Logs

### Log Files
When `logs` directory configured in `rhd.yaml`, each scenario run creates:
- Directory: `<logs>/<scenarioName>-<YYYY-MM-DD-HH-MM-SS>[-N]/`
- File: `log.txt` inside directory
- Collision suffix `-2`, `-3`, etc. if directory exists

### Log Format
Written to both stdout (always) and `log.txt` (when logs configured):

```
===== <scenarioName>: executing scenario =====

===== <stepName>: running command =====
<command> <args>

===== <stepName>: command output =====
[STDOUT] stdout line
[STDERR] stderr line

===== <stepName>: command exit code =====
<code>

===== <stepName>: AI request =====
model: <model>
----- system prompt -----
<prompt>
----- message -----
<message>
----- tools -----
<tool1>, <tool2>

===== <stepName>: AI response =====
<response>

===== <stepName>: tool call =====
name: <tool_name>
arguments: <json>

===== <stepName>: tool result =====
<content>

===== <stepName>: output step =====
<resolved output>

===== ABORTED =====
```

## Meta.json

### Location
Written to each log directory at scenario completion: `<logDir>/meta.json`

### Format
```json
{
  "id": 42,
  "scenario": "my_scenario",
  "status": "success",
  "started": "2026-06-26T15:00:00Z",
  "finished": "2026-06-26T15:01:30Z",
  "durationMs": 90000,
  "tokens": { "prompt": 1500, "completion": 800, "total": 2300 },
  "cost": 0.0235,
  "steps": [
    {
      "name": "build",
      "type": "runCommand",
      "exitCode": 0,
      "started": "2026-06-26T15:00:00Z",
      "finished": "2026-06-26T15:00:10Z",
      "durationMs": 10000
    },
    {
      "name": "ai_review",
      "type": "aiChat",
      "model": "gpt-4",
      "started": "2026-06-26T15:00:10Z",
      "finished": "2026-06-26T15:01:25Z",
      "durationMs": 75000,
      "tokens": { "prompt": 1500, "completion": 800, "total": 2300 },
      "cost": 0.0235
    }
  ]
}
```

### Status Values
- `executing` — currently running
- `success` — completed normally
- `error` — failed with error
- `aborted` — user aborted

## Chat Logs

When `logChats` directory configured in `rhd.yaml`, each chat interaction creates:
- Directory: `<logChats>/<chatTitle>-<YYYY-MM-DD-HH-MM-SS>[-N]/`
- File: `log.txt` inside directory
- Collision suffix `-2`, `-3`, etc. if directory exists

### Chat Log Format
Written to `log.txt` (when logChats configured):

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

## WebSocket Events

### Scenario Events
- `scenarioStarted` — execution started (id, name, startedAt)
- `stepStarted` — step began (executionId, stepName, startedAt)
- `scenarioFinished` — execution completed (full ScenarioMeta)
- `scenarioPaused` — execution paused on AI error (executionId, error, stepName, availableModels)
- `scenarioResumed` — execution resumed after retry (executionId)

### Subscription
- Client sends `subscribe` request
- Server responds with snapshot of current active executions
- Server pushes events as they happen
- Multiple subscribers supported

### Finished Scenarios Query
- Client sends `getFinishedScenarios` with optional `lastId` parameter
- Server returns only scenarios with `id > lastId` (incremental fetch)
- Sorted by start time (newest first)

## Notifications

### Desktop Notifications (Daemon)
- macOS: `terminal-notifier` (preferred, supports click actions) or `osascript` fallback
- Triggered when scenario pauses (if `neverFail` enabled and no WebSocket client alive)
- Debug command: `rhd dev daemon-notification`


## Token Tracking
- Token usage captured from OpenAI-compatible API responses
- Accumulated per scenario execution
- Cost calculated from model config pricing (flat or tiered)
- Stored in `meta.json` and broadcast via WebSocket events
