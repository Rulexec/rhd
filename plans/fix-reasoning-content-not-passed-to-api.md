# Fix: Reasoning Content Not Passed to AI API

## Problem

When building chat messages to send to the AI API, the `thinking_content` (reasoning content) from previous assistant messages is NOT being included in the API request. This means the AI model doesn't have context about its previous reasoning when continuing a conversation with tool calls.

### Evidence from raw.txt

The raw API request shows:
```json
{
  "role": "assistant",
  "content": "",
  "tool_calls": [...]
}
```

There's no `reasoning_content` field, even though the database has `thinking_content` stored:
```json
{
  "id": 466,
  "chatId": 67,
  "role": "assistant",
  "content": "",
  "thinkingContent": "The user wants to know which directories I'm allowed to access..."
}
```

## Root Cause

1. **`ChatMessage::Assistant` variant** in [`packages/rhd_ai/src/client.rs:59`](packages/rhd_ai/src/client.rs:59) doesn't have a field for `reasoning_content`:
   ```rust
   Assistant {
       role: String,
       content: Option<String>,
       tool_calls: Option<Vec<ToolCall>>,
   }
   ```

2. **`build_chat_messages()`** in [`packages/rhd_chat/src/tools.rs:353`](packages/rhd_chat/src/tools.rs:353) creates assistant messages without passing `thinking_content`:
   ```rust
   "assistant" => ChatMessage::assistant(&m.content),
   ```

3. **`build_chat_messages_for_tools()`** in [`packages/rhd_chat/src/tools.rs:365`](packages/rhd_chat/src/tools.rs:365) also doesn't pass `thinking_content` when building assistant messages.

4. The database `Message` struct correctly stores `thinking_content`, but it's never used when building API requests.

## Solution

### Step 1: Add `reasoning_content` field to `ChatMessage::Assistant`

In [`packages/rhd_ai/src/client.rs`](packages/rhd_ai/src/client.rs:59), add the field:

```rust
Assistant {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<ToolCall>>,
},
```

### Step 2: Update constructor methods

Update `ChatMessage::assistant()` to accept optional thinking content:

```rust
pub fn assistant(content: impl Into<String>) -> Self {
    ChatMessage::Assistant {
        role: "assistant".to_string(),
        content: Some(content.into()),
        reasoning_content: None,
        tool_calls: None,
    }
}

pub fn assistant_with_thinking(content: impl Into<String>, thinking: Option<String>) -> Self {
    ChatMessage::Assistant {
        role: "assistant".to_string(),
        content: Some(content.into()),
        reasoning_content: thinking,
        tool_calls: None,
    }
}
```

Update `ChatMessage::assistant_with_tool_calls()`:

```rust
pub fn assistant_with_tool_calls(
    content: Option<String>,
    reasoning_content: Option<String>,
    tool_calls: Vec<ToolCall>,
) -> Self {
    ChatMessage::Assistant {
        role: "assistant".to_string(),
        content,
        reasoning_content,
        tool_calls: if tool_calls.is_empty() {
            None
        } else {
            Some(tool_calls)
        },
    }
}
```

### Step 3: Update `build_chat_messages()` in tools.rs

In [`packages/rhd_chat/src/tools.rs:353`](packages/rhd_chat/src/tools.rs:353):

```rust
pub fn build_chat_messages(messages: &[Message]) -> Vec<ChatMessage> {
    messages
        .iter()
        .map(|m| match m.role.as_str() {
            "user" => ChatMessage::user(&m.content),
            "assistant" => ChatMessage::assistant_with_thinking(&m.content, m.thinking_content.clone()),
            "system" => ChatMessage::system(&m.content),
            _ => ChatMessage::user(&m.content),
        })
        .collect()
}
```

### Step 4: Update `build_chat_messages_for_tools()` in tools.rs

In [`packages/rhd_chat/src/tools.rs:365`](packages/rhd_chat/src/tools.rs:365), update the assistant message building:

```rust
"assistant" => {
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&m.content) {
        let content = json.get("content").and_then(|c| c.as_str()).map(|s| s.to_string());
        let tool_calls: Vec<ToolCall> = json.get("toolCalls")
            .and_then(|tc| serde_json::from_value(tc.clone()).ok())
            .unwrap_or_default();
        
        if tool_calls.is_empty() {
            chat_messages.push(ChatMessage::assistant_with_thinking(content.unwrap_or_default(), m.thinking_content.clone()));
        } else {
            chat_messages.push(ChatMessage::assistant_with_tool_calls(content, m.thinking_content.clone(), tool_calls));
        }
    } else {
        chat_messages.push(ChatMessage::assistant_with_thinking(&m.content, m.thinking_content.clone()));
    }
}
```

### Step 5: Update tests

Update existing tests in [`packages/rhd_chat/src/tools.rs`](packages/rhd_chat/src/tools.rs:406) to verify reasoning content is included in built messages.

## Expected Result

After the fix, the raw API request should include `reasoning_content`:

```json
{
  "role": "assistant",
  "content": "",
  "reasoning_content": "The user wants to know which directories I'm allowed to access. I should use the fs/list_allowed_directories function to get this information.\n",
  "tool_calls": [...]
}
```

## Files to Modify

1. [`packages/rhd_ai/src/client.rs`](packages/rhd_ai/src/client.rs) - Add `reasoning_content` field and update constructors
2. [`packages/rhd_chat/src/tools.rs`](packages/rhd_chat/src/tools.rs) - Update message building functions to pass thinking content
3. Tests in [`packages/rhd_chat/src/tools.rs`](packages/rhd_chat/src/tools.rs:406)
