# Phase 4: IPC Protocol

## Target Vision
Implement inter-process communication protocol between daemon and client using Unix sockets. Define message types with rkyv serialization and length-prefixed framing.

## Key Design Decisions
- `IpcRequest` enum: RunScenario { name: String }
- `IpcResponse` enum: Success { output: String }, Error { message: String }
- rkyv serialization with manual version field (4-byte version + 4-byte length + payload)
- Length-prefixed messages: 4-byte big-endian length + rkyv payload
- Helper functions: write_message, read_message handle partial reads/writes

## Target File Structure
```
rhd/
├── packages/
│   └── rhd_app/
│       └── src/
│           └── ipc/
│               ├── mod.rs
│               └── protocol.rs
```

## Goal from plans/initial.md
Enable daemon-client communication via Unix socket. Client sends RunScenario request, daemon executes scenario, sends response back. Protocol uses rkyv for efficient binary serialization with version field for future compatibility.
