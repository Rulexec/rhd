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

- Step name prefix on all headers except `output` step and scenario-level headers
- `[STDOUT]`/`[STDERR]` prefixes preserve line-by-line interleaving
- AI request splits system prompt and message with sub-headers
- Tool calls/results logged individually in tool loop

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
      "durationMs": 10000,
      "sections": [
        { "kind": "runningCommand", "startLine": 5, "endLine": 6 },
        { "kind": "commandOutput", "startLine": 8, "endLine": 12 },
        { "kind": "exitCode", "startLine": 14, "endLine": 15 }
      ]
    },
    {
      "name": "ai_review",
      "type": "aiChat",
      "model": "gpt-4",
      "started": "2026-06-26T15:00:10Z",
      "finished": "2026-06-26T15:01:25Z",
      "durationMs": 75000,
      "tokens": { "prompt": 1500, "completion": 800, "total": 2300 },
      "cost": 0.0235,
      "sections": [
        { "kind": "aiRequest", "startLine": 17, "endLine": 25 },
        { "kind": "aiResponse", "startLine": 27, "endLine": 35 }
      ]
    }
  ]
}
```

### Status Values
- `executing` — currently running
- `success` — completed normally
- `error` — failed with error
- `aborted` — user aborted

### Step Types
- `runCommand` — includes `exitCode` field
- `aiChat` — includes `model` field, optional `tokens`/`cost`
- `output` — no extra fields

### Log Sections
Each step records line ranges for all log sections (from `=====` and `-----` delimiters). Section kinds:
- `runningCommand`, `commandOutput`, `exitCode`, `commandSpawnError`
- `aiRequest`, `systemPrompt`, `message`, `aiResponse`, `aiRequestFailed`
- `toolCall`, `toolResult`
- `outputStep`, `skipped`

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
- Server pushes events as they happen via broadcast channel
- Multiple subscribers supported

### Finished Scenarios Query
- Client sends `getFinishedScenarios` with optional `lastId` parameter
- Server reads `meta.json` files from logs directory
- Returns only scenarios with `id > lastId` (incremental fetch)
- Sorted by start time (newest first)

## Notifications

### Desktop Notifications (Daemon)
- macOS: `terminal-notifier` (preferred, supports click actions) or `osascript` fallback
- Triggered when scenario pauses (if `neverFail` enabled and no frontend alive)
- Debug command: `rhd dev daemon-notification`

### Browser Notifications (Frontend)
- Request permission on app load
- Shown when `scenarioPaused` event received
- Click notification focuses browser tab
- Debug: `rhd dev frontend-notification` sends `devNotification` WebSocket request

## Token Tracking
- Token usage captured from OpenAI-compatible API responses
- Accumulated per scenario execution
- Cost calculated from model config pricing (flat or tiered)
- Stored in `meta.json` and broadcast via WebSocket events

## Key Files
- Log sink: `packages/rhd_app/src/log.rs`
- Execution tracker: `packages/rhd_app/src/execution.rs`
- Meta types: `packages/rhd_api/src/lib.rs`
- WebSocket events: `packages/rhd_app/src/ws.rs`
- Notifications: `packages/rhd_app/src/notifications.rs`
- Frontend notifications: `frontend/src/lib/notifications.ts`
