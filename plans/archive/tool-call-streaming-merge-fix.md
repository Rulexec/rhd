# Tool Call Streaming Merge Fix

## Problem

When the AI provider sends tool calls in streaming chunks, the plugin incorrectly stores them as separate entries in the database. Example of corrupted data:

```json
[
  {"id":"call_44e4a95e0b454ae6a152d254","type":"function","function":{"name":"rhd_set_todo_list","arguments":""},"tags":[]},
  {"id":"","type":"function","function":{"name":"","arguments":"{\"todos\": \"[ ] Task 1: First todo item\\n[ ] Task 2: Second todo item\"}"},"tags":[]}
]
```

## Root Cause

In [`plugins/rhd_plugin_ai_completions/src/ai_request.rs`](plugins/rhd_plugin_ai_completions/src/ai_request.rs:322), the code merges tool calls by `id`:

```rust
// Merge with existing tool call by ID
if let Some(existing) = final_tool_calls.iter_mut().find(|t| t.id == id) {
    existing.arguments.push_str(&arguments);
} else {
    final_tool_calls.push(StreamToolCallDelta {
        id: id.clone(),
        name,
        arguments,
    });
}
```

**The problem:** The OpenAI streaming format uses the `index` field to identify which tool call a delta belongs to, not `id`. The `id` is only sent in the first chunk for each tool call. Subsequent chunks have `id=None` (which becomes `""`), so they don't match and create new entries.

Looking at [`ToolCallDelta`](packages/rhd_ai_client/src/types.rs:140):
```rust
pub struct ToolCallDelta {
    pub index: usize,  // <-- This is the correct field to use for merging
    pub id: Option<String>,
    pub call_type: Option<String>,
    pub function: Option<FunctionCallDelta>,
}
```

## Solution

Change the merge logic to use `index` instead of `id`:

1. Track tool calls by their `index` field
2. When a new chunk arrives, find the existing tool call with the same `index`
3. If found, append the arguments and update name if it was empty
4. If not found, create a new entry

## Implementation Steps

### Step 1: Add chunked streaming method to mock AI provider

Add `stream_tool_call_chunked()` method to [`packages/rhd_mock_ai_provider/src/types.rs`](packages/rhd_mock_ai_provider/src/types.rs) that simulates real OpenAI behavior:
- First chunk: `index=0`, `id="call_chunked_1"`, `name="function_name"`, `arguments=""`
- Subsequent chunks: `index=0`, `id=None`, `name=None`, `arguments="{\"partial\": ...}"`

### Step 2: Write test that reproduces the bug

Add test `test_tool_call_chunked_streaming` to [`plugins/rhd_plugin_ai_completions/tests/tool_call_e2e_test.rs`](plugins/rhd_plugin_ai_completions/tests/tool_call_e2e_test.rs) that:
1. Uses `stream_tool_call_chunked()` to simulate real streaming
2. Verifies that the stored tool call has:
   - Non-empty `id`
   - Non-empty `name`
   - Complete `arguments` (concatenated from all chunks)
3. Verifies only ONE tool call is stored (not two)

### Step 3: Fix the bug in ai_request.rs

Change the merge logic in [`plugins/rhd_plugin_ai_completions/src/ai_request.rs`](plugins/rhd_plugin_ai_completions/src/ai_request.rs:322):

```rust
// Tool call deltas
if let Some(tool_calls) = &chunk.tool_calls {
    let mut tool_deltas = Vec::new();
    for tc in tool_calls {
        let index = tc.index;
        let id = tc.id.clone().unwrap_or_default();
        let name = tc.function.as_ref().and_then(|f| f.name.clone()).unwrap_or_default();
        let arguments = tc.function.as_ref().and_then(|f| f.arguments.clone()).unwrap_or_default();

        // Merge with existing tool call by INDEX (not ID)
        if let Some(existing) = final_tool_calls.iter_mut().find(|t| t.index == index) {
            existing.arguments.push_str(&arguments);
            // Update id and name if they were empty (first chunk had them)
            if existing.id.is_empty() && !id.is_empty() {
                existing.id = id.clone();
            }
            if existing.name.is_empty() && !name.is_empty() {
                existing.name = name.clone();
            }
        } else {
            final_tool_calls.push(StreamToolCallDelta {
                index,
                id: id.clone(),
                name,
                arguments,
            });
        }

        tool_deltas.push(StreamToolCallDelta {
            index,
            id,
            name: tc.function.as_ref().and_then(|f| f.name.clone()).unwrap_or_default(),
            arguments: tc.function.as_ref().and_then(|f| f.arguments.clone()).unwrap_or_default(),
        });
    }
    // ...
}
```

### Step 4: Update StreamToolCallDelta struct

Add `index` field to [`StreamToolCallDelta`](packages/rhd_chat_api/src/methods/stream_push.rs:10):

```rust
pub struct StreamToolCallDelta {
    /// Index for merging deltas (from OpenAI streaming format)
    pub index: usize,
    /// Unique identifier for the tool call.
    pub id: String,
    /// Tool/function name.
    pub name: String,
    /// Arguments delta (incremental JSON string fragment).
    pub arguments: String,
}
```

### Step 5: Update all usages of StreamToolCallDelta

Update all places that create or use `StreamToolCallDelta` to include the `index` field.

## Files to Modify

1. `packages/rhd_mock_ai_provider/src/types.rs` - Add `stream_tool_call_chunked()` method
2. `plugins/rhd_plugin_ai_completions/tests/tool_call_e2e_test.rs` - Add test
3. `plugins/rhd_plugin_ai_completions/src/ai_request.rs` - Fix merge logic
4. `packages/rhd_chat_api/src/methods/stream_push.rs` - Add `index` field to `StreamToolCallDelta`
5. Any other files that create `StreamToolCallDelta` instances

## Success Criteria

1. Test `test_tool_call_chunked_streaming` passes
2. Existing tests continue to pass
3. Database stores tool calls correctly with non-empty `id`, `name`, and complete `arguments`
