# Global Socket Location Plan

## Goal

Change default socket location from current directory (`./rhd.sock`) to global location (`$HOME/rhd.sock`), allowing `rhd run` to work from any directory without explicitly specifying socket path.

## Current State

- Both `daemon` and `run` commands have `--socket` flag with `default_value = "rhd.sock"`
- Socket created in current working directory
- E2E tests explicitly pass `--socket` with absolute path in temp directory (will continue to work)

## Implementation

### 1. Modify CLI Arguments (`packages/rhd_app/src/cli.rs`)

Change socket field from `PathBuf` with static default to `Option<PathBuf>`:

```rust
#[derive(Parser, Debug)]
pub struct RunArgs {
    pub name: String,
    #[arg(long)]
    pub socket: Option<PathBuf>,
}

#[derive(Parser, Debug)]
pub struct DaemonArgs {
    // ... other fields ...
    #[arg(long)]
    pub socket: Option<PathBuf>,
}
```

Add helper function to compute default socket path:

```rust
pub fn default_socket_path() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("rhd.sock")
}
```

### 2. Update Main (`packages/rhd_app/src/main.rs`)

Compute actual socket path before passing to daemon/client:

```rust
Command::Daemon(args) => {
    let socket_path = args.socket.clone().unwrap_or_else(|| cli::default_socket_path());
    // pass socket_path to run_daemon_command
}

Command::Run(args) => {
    let socket_path = args.socket.clone().unwrap_or_else(|| cli::default_socket_path());
    // pass socket_path to client::run_scenario
}
```

### 3. Update Documentation (`AI.md`)

Update sections:
- **IPC Protocol**: Change "Unix socket at `./rhd.sock`" to "Unix socket at `$HOME/rhd.sock` (configurable via `--socket`)"
- **CLI Usage**: Update examples to show default behavior
- **Important Conventions**: Update point 7 about socket cleanup and point 10 about socket path

### 4. Testing

- Run e2e tests: `cargo run -p rhd_test`
  - Tests explicitly pass `--socket` flag, so should continue to work
- Manual verification:
  - Start daemon without `--socket` flag
  - Verify socket created at `$HOME/rhd.sock`
  - Run client from different directory without `--socket` flag
  - Verify successful connection

### 5. Commit

Commit message: "use global socket path at $HOME/rhd.sock by default"

## Files to Modify

1. `packages/rhd_app/src/cli.rs` - Change socket to Option, add default_socket_path()
2. `packages/rhd_app/src/main.rs` - Compute socket path before use
3. `AI.md` - Update documentation

## Risks

- If `$HOME` not set, falls back to current directory (maintains backward compatibility)
- Existing scripts using relative socket paths need to explicitly pass `--socket rhd.sock`
- E2E tests unaffected (explicitly pass `--socket`)

## Success Criteria

- `rhd daemon` creates socket at `$HOME/rhd.sock` by default
- `rhd run <scenario>` connects to `$HOME/rhd.sock` by default
- `--socket` flag still works for custom locations
- E2E tests pass without modification
- AI.md reflects new behavior
