# SQLite Scenario ID Generation Plan

## Problem
Currently, scenario execution IDs are generated using an atomic counter in memory (`AtomicU64` in `ExecutionTracker`). When the daemon restarts, the counter resets to 1, causing duplicate IDs in the frontend's finished scenarios list.

## Solution
Create a new `rhd_db` crate to manage SQLite database for persisting the next scenario ID across daemon restarts.

## Requirements
- Create new crate `rhd_db` for database management
- Add `dbPath` configuration option (default: `rhd.db`)
- Create SQLite database with single table `meta`
- Table schema: `meta(nextScenarioId INTEGER)`
- Initialize with `nextScenarioId = 1` if database doesn't exist
- Atomically increment and retrieve ID when starting scenario execution
- Ensure ID generation is thread-safe

## Implementation Steps

### 1. Create rhd_db Crate Structure
- Create `packages/rhd_db/` directory
- Create `packages/rhd_db/Cargo.toml` with `rusqlite` dependency (bundled feature)
- Create `packages/rhd_db/src/lib.rs` with public API

### 2. Implement Database Module
- Define `ScenarioDb` struct wrapping SQLite connection
- Implement methods:
  - `new(path: &str) -> Result<Self>`: Open/create database
  - `init() -> Result<()>`: Create table if not exists, initialize counter
  - `next_id() -> Result<u64>`: Atomically get and increment ID
- Use WAL mode for better concurrency
- Handle errors with custom `DbError` type

### 3. Update Workspace Configuration
- Add `rhd_db` to workspace members in root `Cargo.toml`
- Add `rusqlite` to workspace dependencies with `bundled` feature

### 4. Update rhd_app Dependencies
- Add `rhd_db` dependency to `packages/rhd_app/Cargo.toml`

### 5. Update Configuration
- Add `dbPath: Option<String>` field to `DaemonConfig` in `packages/rhd_app/src/config.rs`
- Default value: `"rhd.db"`
- Add CLI flag `--db-path` to override config

### 6. Integrate with ExecutionTracker
- Modify `ExecutionTracker::new()` to accept `ScenarioDb` reference (Arc)
- Change `next_id` from `AtomicU64` to use database
- Update `start()` method to call `db.next_id()` instead of atomic increment

### 7. Update Daemon Initialization
- In `daemon.rs`, initialize database before creating `ExecutionTracker`
- Pass database reference to tracker
- Handle database errors gracefully (fail fast if DB can't be opened)

### 8. Update CLI
- Add `--db-path` argument to daemon command in `cli.rs`
- Pass to `DaemonConfig` during initialization

### 9. Testing
- Test ID generation across daemon restarts
- Verify concurrent scenario execution gets unique IDs
- Test with custom `dbPath` configuration
- Ensure backward compatibility (existing meta.json files still work)

## Database Schema

```sql
CREATE TABLE IF NOT EXISTS meta (
    nextScenarioId INTEGER NOT NULL
);

-- Initialize with single row
INSERT INTO meta (nextScenarioId) VALUES (1);
```

## File Changes

### New Files
- `packages/rhd_db/Cargo.toml` - Crate manifest
- `packages/rhd_db/src/lib.rs` - Database module with ScenarioDb struct

### Modified Files
- `Cargo.toml` - Add rhd_db to workspace, add rusqlite to dependencies
- `packages/rhd_app/Cargo.toml` - Add rhd_db dependency
- `packages/rhd_app/src/config.rs` - Add dbPath field
- `packages/rhd_app/src/cli.rs` - Add --db-path flag
- `packages/rhd_app/src/execution.rs` - Use database for ID generation
- `packages/rhd_app/src/daemon.rs` - Initialize database
- `packages/rhd_app/src/main.rs` - Pass db path from CLI to config

## rhd_db Public API

```rust
pub struct ScenarioDb {
    conn: rusqlite::Connection,
}

impl ScenarioDb {
    pub fn new(path: &str) -> Result<Self, DbError>;
    pub fn next_id(&self) -> Result<u64, DbError>;
}

pub enum DbError {
    SqliteError(rusqlite::Error),
    InitializationError(String),
}
```

## Migration Strategy
No migration needed for existing data. The database only stores the next ID counter. Existing meta.json files remain unchanged and continue to work. The frontend will see new unique IDs going forward.

## Risks
- Database file corruption could cause ID generation to fail
- Mitigation: Use WAL mode for better concurrency and crash recovery
- Single point of failure for ID generation
- Mitigation: Fail fast on database errors rather than continuing with broken state

## Success Criteria
- Daemon starts successfully with new database
- Scenario IDs are unique across daemon restarts
- Concurrent executions get unique IDs
- Configuration works via config file and CLI flag
- No performance degradation (database operations should be < 1ms)
- rhd_db crate is self-contained and reusable
