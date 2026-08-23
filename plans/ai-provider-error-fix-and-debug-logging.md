# AI Provider Error Fix and Debug Logging

## Problem Analysis

### Error Message
```
Role must be in ["user", "assistant", "system", "function", "plugin", "tool"] 
and the role in last message must be in ["user", "function", "tool"]
```

### Chat State
The chat has 2 messages:
1. User message: "Hello, how are you?" (id=2)
2. Assistant error message with "ai_completions:error" tag (id=3)

The error message was added AFTER the AI request failed. This is the FIRST call that failed.

### Current Understanding
The filtering of invalid roles should be present in the code, but it appears either:
- Messages are not being sent at all, OR
- The first message is being sent incorrectly

We need to see EXACTLY what body is being sent to the AI provider to diagnose the issue.

## Solution

### Task 1: Add Debug Logging for AI Request Body
**File**: `plugins/rhd_plugin_ai_completions/src/ai_request.rs`

Add debug logging that logs the EXACT JSON body being sent to the AI provider.

**Location**: In `handle_ai_request()` function, after building the request (around line 110)

The logging should show:
- The complete serialized JSON body that will be sent
- This will help us see exactly what messages array is being sent

```rust
// After building the request, log the exact body being sent
let request_body = serde_json::to_string(&request)
    .map_err(|e| AiRequestError::EventSend(e.to_string()))?;
tracing::debug!(
    chat_id = chat_id,
    request_body = %request_body,
    "sending AI completion request"
);
```

### Task 2: Investigate and Fix
After adding the debug logging, we need to:
1. Ask the user to reproduce the issue and share the debug logs
2. Analyze what body is actually being sent
3. Identify why the AI provider is rejecting the request
4. Implement the appropriate fix

## Implementation Order

1. Add debug logging to log the exact request body being sent
2. Ask user to reproduce and share logs
3. Analyze logs and identify root cause
4. Implement fix based on findings

## Expected Outcome

After adding debug logging, we will be able to see exactly what messages array is being sent to the AI provider, which will help us identify why the request is being rejected.
