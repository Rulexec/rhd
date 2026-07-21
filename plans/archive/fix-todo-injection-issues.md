# Fix Todo List Injection Issues

## Problem Analysis

### Issue 1: System prompt with todo contract is not injected

**Root Cause**: Logic error in [`inject_todo_tool_contract()`](packages/rhd_chat/src/stream.rs:36)

The function checks if there are ANY user messages and returns early if true:
```rust
let has_user_messages = messages.iter().any(|m| m.role == "user");
if has_user_messages {
    return Ok(());
}
```

This is inverted logic. The contract should be injected when the FIRST user message is being added, not when there are NO user messages. Since `inject_todo_tool_contract()` is called BEFORE the user message is added to the database, the check should allow injection when there are 0 user messages (first message scenario).

**Current behavior**: Contract is never injected because by the time we check, we're about to add the first user message, but the logic skips injection if any user messages exist.

**Expected behavior**: Contract should be injected on the first user message (when there are 0 existing user messages in the database).

**Evidence**: The API request shows no system message with the contract:
```json
{
  "model": "qwen3.7-plus",
  "messages": [
    {
      "role": "user",
      "content": "Hello, do you have todo lists capability?"
    }
  ],
  "tools": [...]
}
```

### Issue 2: Tool loop hangs after tool call completion

**Root Cause**: Missing error handling in [`tool_loop()`](packages/rhd_chat/src/tools.rs:662)

After tool execution, `inject_todo_list_message()` is called:
```rust
inject_todo_list_message(manager, chat_id, template_loader)?;
```

If this function returns an error (e.g., missing template), the tool loop exits immediately without:
1. Sending `StreamFinished` or `StreamError` event
2. Unregistering the stream
3. Cleaning up state

This leaves the frontend in a hanging state with `isStreaming: true` but no further updates.

**Evidence from output.json**:
- Tool call completed successfully (status: "completed", result: "Todo list updated successfully.")
- But `isStreaming: true` and `streamingMessageId: null`
- No final assistant message was generated

## Solution

### Fix 1: Correct contract injection logic

**File**: `packages/rhd_chat/src/stream.rs`

Change the logic in `inject_todo_tool_contract()` to properly detect first message scenario:

```rust
pub fn inject_todo_tool_contract<P: ProjectProvider>(
    manager: &ChatManager<P>,
    chat_id: i64,
    template_loader: &TemplateLoaderRef,
) -> Result<(), ChatError> {
    let messages = manager.db().get_messages(chat_id)?;

    // Count user messages to determine if this is the first message
    let user_message_count = messages.iter().filter(|m| m.role == "user").count();
    
    // Only inject on first user message (when there are 0 existing user messages)
    if user_message_count > 0 {
        return Ok(());
    }

    // Check if contract already exists (safety check)
    let has_contract = messages.iter().any(|m| {
        m.role == "system" && m.content.contains("rhd_set_todo_list Tool Contract")
    });
    if has_contract {
        return Ok(());
    }

    let contract_content = match template_loader.get_template("rhd_set_todo_list_contract") {
        Some(content) => content,
        None => {
            return Ok(());
        }
    };

    manager.db().add_message(chat_id, "system", &contract_content, None, None)?;

    Ok(())
}
```

### Fix 2: Add error handling for todo list injection

**File**: `packages/rhd_chat/src/tools.rs`

Make `inject_todo_list_message()` more resilient by skipping injection if templates are missing instead of returning an error:

```rust
pub fn inject_todo_list_message<P: ProjectProvider>(
    manager: &ChatManager<P>,
    chat_id: i64,
    template_loader: &crate::stream::TemplateLoaderRef,
) -> Result<(), ChatError> {
    // Get current todo list from database
    let todo_list_str = match manager.db().get_todo_list(chat_id)? {
        Some(list) => list,
        None => {
            // No todo list exists, inject prompt to create one
            let empty_prompt = match template_loader.get_template("todo_list_empty") {
                Some(template) => template,
                None => {
                    // Template missing, skip injection silently
                    return Ok(());
                }
            };
            
            // Add as system message (will be included in AI context)
            manager.db().add_message(chat_id, "system", &empty_prompt, None, None)?;
            return Ok(());
        }
    };
    
    // Parse todo list
    let todo_items = match parse_todo_list(&todo_list_str) {
        Ok(items) => items,
        Err(_) => {
            // Failed to parse, skip injection
            return Ok(());
        }
    };
    
    // Get active role if any
    let active_role = manager.db().get_active_role(chat_id)?;
    
    // Render environment details with todo list
    let environment_content = match render_environment_details_for_injection(
        template_loader,
        &todo_items,
        active_role.as_ref(),
    ) {
        Ok(content) => content,
        Err(_) => {
            // Failed to render, skip injection silently
            return Ok(());
        }
    };
    
    // Add as system message
    manager.db().add_message(chat_id, "system", &environment_content, None, None)?;
    
    Ok(())
}
```

Also update `render_environment_details_for_injection()` to return `Option<String>` instead of `Result<String, ChatError>`:

```rust
fn render_environment_details_for_injection(
    template_loader: &crate::stream::TemplateLoaderRef,
    todo_items: &[crate::TodoItem],
    active_role: Option<&(String, String)>,
) -> Option<String> {
    // Render todo items
    let todo_items_str = if todo_items.is_empty() {
        template_loader.get_template("todo_list_empty")?
    } else {
        let mut items_output = String::new();
        for (idx, item) in todo_items.iter().enumerate() {
            let status_str = match item.status {
                crate::TodoStatus::Pending => "Pending",
                crate::TodoStatus::InProgress => "In Progress",
                crate::TodoStatus::Completed => "Completed",
                crate::TodoStatus::Discarded => "Discarded",
            };
            items_output.push_str(&format!("| {} | {} | {} |\n", idx + 1, item.content, status_str));
        }
        
        let template = template_loader.get_template("todo_list_with_items")?;
        template.replace("{todoItems}", &items_output)
    };
    
    // Select template based on role
    let template_name = if active_role.is_some() {
        "environment_details_with_role"
    } else {
        "environment_details_no_role"
    };
    
    let template = template_loader.get_template(template_name)?;
    
    let mut result = template.replace("{todoItems}", &todo_items_str);
    
    // Replace role placeholder if present
    if let Some((project_name, role_name)) = active_role {
        result = result.replace("{currentRoleName}", &format!("{} ({})", role_name, project_name));
    }
    
    Some(result)
}
```

## Implementation Steps

1. **Fix contract injection logic** in `packages/rhd_chat/src/stream.rs`:
   - Verify the logic is correct for first message scenario
   - Add debug logging if needed to trace the issue

2. **Make todo list injection resilient** in `packages/rhd_chat/src/tools.rs`:
   - Change `inject_todo_list_message()` to skip injection silently if templates are missing
   - Update `render_environment_details_for_injection()` to return `Option<String>` instead of `Result`

3. **Add error handling in tool loop** in `packages/rhd_chat/src/tools.rs`:
   - Wrap `inject_todo_list_message()` call in a match statement
   - Log errors but don't propagate them
   - Ensure `StreamFinished` or `StreamError` event is always sent

4. **Test the fixes**:
   - Create a new chat and verify contract is injected
   - Trigger a tool call and verify the loop completes successfully
   - Verify no hanging state in the frontend

## Files to Modify

- `packages/rhd_chat/src/stream.rs` - Fix contract injection logic
- `packages/rhd_chat/src/tools.rs` - Make todo list injection resilient, add error handling
