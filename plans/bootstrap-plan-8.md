# Phase 8: Client

## Target Vision
Implement Unix socket client for sending scenario execution requests to daemon. Connect to socket, send RunScenario request, receive and display response.

## Key Design Decisions
- Connect to `./rhd.sock` in current directory
- Send IpcRequest::RunScenario { name }
- Read IpcResponse from socket
- Handle connection errors (daemon not running)
- CLI: `rhd run <name>` subcommand
- On Success: print output, exit 0
- On Error: print error message, exit 1

## Target File Structure
```
rhd/
├── packages/
│   └── rhd_app/
│       └── src/
│           ├── main.rs
│           ├── cli.rs
│           └── client.rs
```

## Goal from plans/initial.md
Implement client mode: `rhd run <name>` connects to daemon Unix socket, sends scenario name, receives execution result. Prints output on success (exit 0) or error details on failure (exit 1).
