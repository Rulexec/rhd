# Phase 6: Action Execution

## Target Vision
Implement scenario action execution engine. Execute runCommand, aiChat, output actions sequentially. Build execution context map for placeholder resolution.

## Key Design Decisions
- `runCommand`: tokio::process::Command, capture exit code + stdout/stderr combined, never fail scenario on non-zero exit
- `aiChat`: resolve model from config, resolve placeholders in systemPrompt/message, call OpenAI client, store response as `message` field, on failure store error and mark step failed
- `output`: resolve placeholders in template, return final string (no context storage)
- Executor: load scenario, validate, execute actions sequentially, build context map (step_name -> { exitCode, stdoutStderr, message }), return final output or error

## Target File Structure
```
rhd/
├── packages/
│   └── rhd_app/
│       └── src/
│           └── scenario/
│               ├── mod.rs
│               ├── loader.rs
│               ├── executor.rs
│               └── placeholder.rs
```

## Goal from plans/initial.md
Execute scenario action chains sequentially. runCommand captures exit code and output. aiChat calls AI with placeholders resolved. output produces final result. Execution context stores step results for placeholder resolution across steps.
