# AI Completions Ack Fix and Guardrails

## Problem Summary

The `ai_completions` plugin has a critical bug where it never records acknowledgments from other plugins, causing an infinite snowball effect:

1. Plugin sends `ai_completions:preRequest` custom event
2. Other plugins (e.g., `system_prompt`) acknowledge the event
3. The acknowledgment event is received but **never recorded** in `plugins_monitor`
4. `wait_for_acks_except()` times out after 30 seconds
5. Main loop iterates again, sees queued messages still exist
6. Triggers again → creates another event → another wait
7. Multiple unacknowledged events accumulate, all waiting forever

## Root Cause

In [`plugins/rhd_plugin_ai_completions/src/plugin.rs`](plugins/rhd_plugin_ai_completions/src/plugin.rs:77-83), after creating `plugins_monitor`, the code does not subscribe to `customEventAcknowledged` events to call `record_acknowledgment()`.

## Plan

### Task 1: Move Callback Wiring Inside PluginsMonitor

**File**: `packages/rhd_chat_client/src/plugins_monitor.rs`

Move the acknowledgment callback subscription inside `PluginsMonitor::new()` so it automatically wires up when created. This eliminates the need for each plugin to manually subscribe.

**Changes to `PluginsMonitor::new()`**:

```rust
pub async fn new(client: &ChatClient) -> Result<Self, ClientError> {
    let all_plugins = Arc::new(RwLock::new(HashMap::new()));
    let acknowledgments = Arc::new(RwLock::new(HashMap::new()));

    // ... existing plugin list subscription code ...

    // Subscribe to custom event acknowledgments
    let acknowledgments_clone = Arc::clone(&acknowledgments);
    let _ack_token = client.on_custom_event_acknowledged(move |event| {
        let acks = Arc::clone(&acknowledgments_clone);
        async move {
            let mut acks_guard = acks.write().await;
            acks_guard
                .entry(event.event_id)
                .or_insert_with(HashSet::new)
                .insert(event.acknowledging_plugin_id);
        }
    });

    Ok(Self {
        all_plugins,
        acknowledgments,
        _plugins_list_token: token,
        _ack_token,  // Store to keep subscription alive
    })
}
```

**Struct update**: Add `_ack_token: CancellationToken` field to `PluginsMonitor`.

This way, any code that creates a `PluginsMonitor` automatically gets acknowledgment recording without manual wiring.

### Task 2: Add Chat Processing Guardrails

**File**: `plugins/rhd_plugin_ai_completions/src/plugin.rs`

Add a `HashSet<i64>` to track chats currently being processed. This prevents the main loop from triggering multiple AI requests for the same chat while one is already in progress.

**Changes**:

1. Before the main loop, create a shared set:
   ```rust
   let processing_chats = Arc::new(RwLock::new(HashSet::new()));
   ```

2. In the main loop, before spawning the AI request task:
   - Check if `chat_id` is already in `processing_chats`
   - If yes, skip this chat
   - If no, add it to the set

3. In the spawned task, remove `chat_id` from the set when done (success or failure):
   ```rust
   let processing_chats_clone = Arc::clone(&processing_chats);
   tokio::spawn(async move {
       let result = ai_request::handle_ai_request(...).await;
       processing_chats_clone.write().await.remove(&chat_id);
       if let Err(e) = result {
           tracing::error!("AI request failed for chat {}: {}", chat_id, e);
       }
   });
   ```

### Task 3: Add `--clear-pending-acks` CLI Argument to rhd_chat_server

**Files**:
- `packages/rhd_chat_server/src/config.rs`
- `packages/rhd_chat_server/src/main.rs`
- `packages/rhd_db/src/chat_db/custom_events.rs`

**Changes**:

1. **config.rs**: Add new CLI argument:
   ```rust
   /// Clear all pending custom event acknowledgments before starting server
   #[arg(long)]
   pub clear_pending_acks: bool,
   ```

2. **custom_events.rs**: Add function to clear all custom events:
   ```rust
   /// Delete all custom events and their acknowledgments.
   pub fn clear_all_custom_events(conn: &Mutex<Connection>) -> DbResult<()> {
       let conn_guard = conn.lock().map_err(|e| DbError::InitializationError(e.to_string()))?;
       conn_guard.execute("DELETE FROM custom_events", [])?;
       Ok(())
   }
   ```

3. **main.rs**: Check the flag before starting the server, but continue to start normally after clearing:
   ```rust
   // Initialize database
   let db = Arc::new(
       ChatDb::new(&config.db_path).map_err(|e| ServerError::Database(e.to_string()))?
   );

   // Clear pending acks if requested
   if config.clear_pending_acks {
       db.clear_all_custom_events().map_err(|e| ServerError::Database(e.to_string()))?;
       info!("Cleared all pending custom event acknowledgments");
   }

   // Start server (continues normally)
   server::run_with_db(config, db).await?;
   ```

   Note: Need to expose `clear_all_custom_events` through `ChatDb` public API and refactor `server::run()` to accept a pre-initialized database.

## Implementation Order

1. Task 1 (callback wiring in PluginsMonitor) — fixes the root cause
2. Task 2 (guardrails) — prevents snowball effect even if acks are slow
3. Task 3 (CLI argument) — utility for cleaning up corrupted state

## Testing Considerations

- Task 1: Verify that `system_prompt` acknowledgment is recorded and `wait_for_acks_except()` returns successfully
- Task 2: Verify that triggering AI completion for a chat already being processed is skipped
- Task 3: Run `rhd_chat_server --clear-pending-acks --db-path ./rhd_db` and verify database is cleaned, then server starts normally
