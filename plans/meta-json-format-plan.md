# Meta.json Format Enhancement Plan

## Goal

Enhance meta.json format to include:
1. Step `type` field for every step (`runCommand`, `aiChat`, `output`)
2. `exitCode` property for `runCommand` steps
3. `model` property for `aiChat` steps

## Current State

**StepTiming** (rhd_api/src/lib.rs:53-65):
```rust
pub struct StepTiming {
    pub name: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub duration_ms: u64,
    pub tokens: Option<TokenUsage>,
    pub cost: Option<f64>,
    pub sections: Vec<LogSection>,
}
```

**ExecutionState** (rhd_app/src/execution.rs:20-27):
- Tracks step timings, tokens, sections
- No step type, exit code, or model tracking

## Target meta.json Format

```json
{
  "scenario": "my_scenario",
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
      "sections": [...]
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

## Implementation Steps

### 1. Update StepTiming struct (rhd_api/src/lib.rs)

Add new fields:
- `step_type: StepType` (enum: RunCommand, AiChat, Output)
- `exit_code: Option<i32>` (for runCommand steps)
- `model: Option<String>` (for aiChat steps)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StepType {
    RunCommand,
    AiChat,
    Output,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepTiming {
    pub name: String,
    pub step_type: StepType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens: Option<TokenUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<f64>,
    pub sections: Vec<LogSection>,
}
```

### 2. Update ExecutionState (rhd_app/src/execution.rs)

Add tracking fields:
- `current_step_type: Option<StepType>`
- `current_step_exit_code: Option<i32>`
- `current_step_model: Option<String>`

Add methods to ExecutionHandle:
- `set_step_type(&self, step_type: StepType)`
- `set_step_exit_code(&self, exit_code: i32)`
- `set_step_model(&self, model: String)`

Update `finalize_current_step` to include new fields in StepTiming.

### 3. Update executor.rs

Pass step type when calling `step_started`:
- For RunCommand: `handle.set_step_type(StepType::RunCommand)`
- For AiChat: `handle.set_step_type(StepType::AiChat)`
- For Output: `handle.set_step_type(StepType::Output)`

### 4. Update run_command.rs

After command execution, report exit code:
```rust
if let Some(h) = &handle {
    h.set_step_exit_code(exit_code);
}
```

### 5. Update ai_chat.rs

After determining model name, report it:
```rust
if let Some(h) = &handle {
    h.set_step_model(model_name.clone());
}
```

### 6. Update AI.md documentation

Update meta.json format section to reflect new fields.

### 7. Update E2E tests (rhd_test/src/standard_test.rs)

Add validation for new fields:
- Check `step_type` is present for all steps
- Check `exit_code` is present for runCommand steps
- Check `model` is present for aiChat steps

## Files to Modify

1. `packages/rhd_api/src/lib.rs` - Add StepType enum, update StepTiming
2. `packages/rhd_app/src/execution.rs` - Track step type, exit code, model
3. `packages/rhd_app/src/scenario/executor.rs` - Set step type
4. `packages/rhd_app/src/scenario/run_command.rs` - Report exit code
5. `packages/rhd_app/src/scenario/ai_chat.rs` - Report model
6. `packages/rhd_test/src/standard_test.rs` - Validate new fields
7. `AI.md` - Update documentation

## Success Criteria

- meta.json includes `type` field for every step
- `runCommand` steps include `exitCode` field
- `aiChat` steps include `model` field
- E2E tests pass with new validation
- Documentation updated
