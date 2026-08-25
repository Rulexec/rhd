# WebSocket Reconnection with Exponential Backoff

## Overview

Add exponential backoff retry logic to WebSocket connection attempts in `rhd_chat_client` and `rhd_plugin_ai_completions`.

## Current State

- [`ChatClient::connect()`](packages/rhd_chat_client/src/client.rs:68) makes a single connection attempt with no retry
- [`run_plugin()`](plugins/rhd_plugin_ai_completions/src/plugin.rs:24) calls `ChatClient::connect()` once and fails immediately on error

## Requirements

- Initial retry after 1 second
- Second retry after ~2 seconds
- Maximum retry interval capped at 10 seconds
- Exponential backoff pattern: 1s → 2s → 4s → 8s → 10s → 10s → ...

## Approach: Use `exponential-backoff` Crate

Use the `exponential-backoff` crate (v2.1.0) which provides a simple iterator-based API for exponential backoff.

### Dependency Addition

Add to workspace `Cargo.toml`:
```toml
exponential-backoff = "2.1.0"
```

Add to `packages/rhd_chat_client/Cargo.toml`:
```toml
exponential-backoff = { workspace = true }
```

## Implementation Plan

### Phase 1: Add Retry Logic to ChatClient

**File**: `packages/rhd_chat_client/src/client.rs`

1. Add `exponential-backoff` dependency to workspace and rhd_chat_client
2. Create a new `connect_with_retry()` method using `Backoff` iterator
3. Configure backoff parameters:
   - `attempts`: `u32::MAX` (effectively infinite)
   - `min`: 1 second
   - `max`: 10 seconds
4. Add logging for each retry attempt
5. Keep the original `connect()` method for backward compatibility

**Example Implementation**:
```rust
use exponential_backoff::Backoff;

pub async fn connect_with_retry(url: &str) -> Result<Self, ClientError> {
    let attempts = u32::MAX;
    let min = Duration::from_secs(1);
    let max = Duration::from_secs(10);
    
    let mut attempt = 0;
    for duration in Backoff::new(attempts, min, max) {
        match Self::connect(url).await {
            Ok(client) => return Ok(client),
            Err(e) => {
                attempt += 1;
                match duration {
                    Some(delay) => {
                        tracing::warn!(
                            attempt = attempt,
                            delay_ms = delay.as_millis(),
                            error = %e,
                            "Connection failed, retrying"
                        );
                        tokio::time::sleep(delay).await;
                    }
                    None => return Err(e),
                }
            }
        }
    }
    
    // This should never be reached with u32::MAX attempts
    Err(ClientError::ConnectionClosed)
}
```

### Phase 2: Update Plugin to Use Retry

**File**: `plugins/rhd_plugin_ai_completions/src/plugin.rs`

1. Replace `ChatClient::connect()` call with `ChatClient::connect_with_retry()`
2. The plugin will now automatically retry connections with exponential backoff

### Phase 3: Testing

1. Add integration test that verifies reconnection behavior when server is temporarily unavailable
2. Test that the client successfully connects after server restarts within retry window

## Files to Modify

1. `Cargo.toml` - Add exponential-backoff to workspace dependencies
2. `packages/rhd_chat_client/Cargo.toml` - Add exponential-backoff dependency
3. `packages/rhd_chat_client/src/client.rs` - Add connect_with_retry() method
4. `plugins/rhd_plugin_ai_completions/src/plugin.rs` - Use connect_with_retry()
