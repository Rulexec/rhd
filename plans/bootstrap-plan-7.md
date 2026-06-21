# Phase 7: Daemon

## Target Vision
Implement Unix socket server daemon. Accept client connections, handle RunScenario requests, execute scenarios, send responses. Graceful shutdown on signals.

## Key Design Decisions
- Bind to `./rhd.sock` in current working directory
- tokio::net::UnixListener accepts connections
- Spawn task per connection for concurrent handling
- Request handler: read IpcRequest, match RunScenario, call executor, send IpcResponse
- Daemon never stops on scenario errors; sends Error response to client
- CLI parsing with clap: `rhd daemon` subcommand
- Load models before socket bind, exit if invalid
- Graceful shutdown on SIGTERM/SIGINT

## Target File Structure
```
rhd/
├── packages/
│   └── rhd_app/
│       └── src/
│           ├── main.rs
│           ├── cli.rs
│           └── daemon.rs
```

## Goal from plans/initial.md
Implement daemon mode: `rhd daemon` creates Unix socket, waits for commands. Loads models at startup, validates scenarios, executes actions, sends results back to client. Daemon stays alive on scenario errors.
