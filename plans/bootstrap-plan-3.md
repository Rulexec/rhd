# Phase 3: Scenario Definition

## Target Vision
Implement scenario YAML schema parsing, loading, and placeholder resolution system. Load `scenarios/<name>/scenario.yaml` files, validate structure, resolve `%stepName.field%` placeholders.

## Key Design Decisions
- `Scenario` struct in `rhd_app`: defines action chain
- Action types: RunCommand, AiChat, Output
- Each action has optional `name` field for placeholder references
- serde_yaml with manual validation for error reporting with line numbers
- Placeholder pattern: `%stepName.field%` resolved against execution context
- Missing values replaced with empty string; exitCode always available; stdoutStderr empty on failure

## Target File Structure
```
rhd/
├── packages/
│   └── rhd_app/
│       └── src/
│           └── scenario/
│               ├── mod.rs
│               ├── loader.rs
│               └── placeholder.rs
```

## Goal from plans/initial.md
Support scenario definition via YAML files in `scenarios/<name>/scenario.yaml`. Scenarios are sequential action chains (runCommand, aiChat, output). Actions can have names for placeholder references. Placeholders like `%testsRun.exitCode%` resolve to step execution results.
