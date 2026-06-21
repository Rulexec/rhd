# Phase 9: Integration

## Target Vision
Wire up CLI argument parsing, perform end-to-end testing, ensure comprehensive error reporting. Validate complete daemon-client workflow with all action types.

## Key Design Decisions
- clap argument parsing with subcommands: daemon, run
- Validate CLI arguments before execution
- End-to-end test scenario with all action types (runCommand, aiChat, output)
- Error reporting includes context: file path, line number, step name
- Daemon stays alive on scenario errors; client receives Error response
- Client exit codes: 0 on success, 1 on error

## Target File Structure
```
rhd/
├── packages/
│   └── rhd_app/
│       └── src/
│           ├── main.rs
│           └── cli.rs
```

## Goal from plans/initial.md
Complete end-to-end integration. CLI validates arguments, daemon loads models and scenarios, executes action chains, client receives results. Error cases handled: missing model, invalid YAML, AI failure, daemon not running. All errors include context for debugging.
