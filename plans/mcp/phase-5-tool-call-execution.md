# Phase 5: Tool-call execution and result push

## Overview

Complete the round trip: subscribe to `assistantMessageWithToolCalls` events for the prefixed tool names this plugin registered, route each call to the owning MCP server via the pool, and answer every call with a `tool`-role message carrying the matching `toolCallId`. MCP/transport errors are returned as tool content (the model decides how to handle them — same behavior as the existing MCP tool loop), never as plugin crashes.

**Scope:**
- In: `tool_handler.rs` (new), subscription wiring at step 8 of `plugin.rs` (Phase 4 left the insertion point), `lib.rs` module list.
- Out: gating predicates (Phase 4), pool routing (Phase 3), tests (Phase 6).

**Depends on:** Phases 3 and 4.

## Files to Create/Modify

### 1. `plugins/rhd_plugin_mcp/src/lib.rs`

```rust
pub mod config;
pub mod gating;
pub mod mcp_pool;
pub mod plugin;
pub mod tool_handler;
```

### 2. `plugins/rhd_plugin_mcp/src/tool_handler.rs`

**Create** — full implementation:

```rust
//! Executes MCP tool calls and pushes results back to the chat.

use std::sync::Arc;

use rhd_chat_api::{
    AddMessageParams, AssistantMessageWithToolCallsData, GetChatParams, ToolCall,
};
use rhd_chat_client::ChatClient;

use crate::mcp_pool::McpPool;
use crate::plugin::ChatRegistrations;

/// Handle an `assistantMessageWithToolCalls` event for all calls this plugin owns.
///
/// For each tool call in the message:
/// 1. Skip names this plugin cannot route (not our prefix).
/// 2. Skip servers not registered on this chat (AD-3; defensive).
/// 3. Skip calls already answered (duplicate guard).
/// 4. Route `{name}:{tool}` → owning server, execute via the pool.
/// 5. Add a `tool`-role message with `toolCallId` and the result content.
///    Errors become content too — the model decides how to react.
pub async fn handle_tool_calls(
    client: Arc<ChatClient>,
    pool: Arc<McpPool>,
    registrations: ChatRegistrations,
    event: AssistantMessageWithToolCallsData,
) {
    let chat_id = event.chat_id;

    for tool_call in &event.message.tool_calls {
        if let Err(e) =
            handle_single_call(&client, &pool, &registrations, chat_id, tool_call).await
        {
            // A failed call must still be answered so the AI tool loop can proceed.
            tracing::error!(
                chat_id = chat_id,
                tool = %tool_call.function.name,
                tool_call_id = %tool_call.id,
                error = %e,
                "failed to answer MCP tool call"
            );
        }
    }
}

async fn handle_single_call(
    client: &ChatClient,
    pool: &McpPool,
    registrations: &ChatRegistrations,
    chat_id: i64,
    tool_call: &ToolCall,
) -> Result<(), HandlerError> {
    let prefixed_name = &tool_call.function.name;

    // 1. Not routable by us (subscription filter should prevent this).
    let Some(route) = pool.route(prefixed_name) else {
        tracing::debug!(chat_id, tool = %prefixed_name, "tool call not owned by this plugin");
        return Ok(());
    };

    // 2. Server's tools must have been registered on this chat (AD-3).
    {
        let regs = registrations.read().await;
        let registered = regs
            .get(&chat_id)
            .map(|ids| ids.contains(&route.server_id))
            .unwrap_or(false);
        if !registered {
            tracing::warn!(
                chat_id,
                tool = %prefixed_name,
                server_id = %route.server_id,
                "tool call for server not registered on this chat, skipping"
            );
            return Ok(());
        }
    }

    // 3. Duplicate guard: a tool message with this tool_call_id may exist already.
    if has_tool_result(client, chat_id, &tool_call.id).await? {
        tracing::debug!(
            chat_id,
            tool_call_id = %tool_call.id,
            "tool result already exists, skipping"
        );
        return Ok(());
    }

    tracing::info!(
        chat_id,
        tool = %prefixed_name,
        tool_call_id = %tool_call.id,
        arguments = %tool_call.function.arguments,
        "executing MCP tool call"
    );

    // 4. Execute (serialized per server inside the pool, AD-5).
    let content = match pool
        .call_tool(
            &route.server_id,
            &route.tool_name,
            &tool_call.function.arguments,
        )
        .await
    {
        Ok(result) => {
            if result.is_error.unwrap_or(false) {
                tracing::warn!(
                    chat_id,
                    tool = %prefixed_name,
                    "MCP server reported tool error"
                );
            }
            result.content
        }
        Err(e) => {
            // 5a. Error path: surface as tool content, keep the loop alive.
            tracing::error!(chat_id, tool = %prefixed_name, error = %e, "MCP call failed");
            format!("MCP tool call failed: {}", e)
        }
    };

    // 5b. Push result.
    client
        .add_message(AddMessageParams {
            chat_id,
            role: "tool".to_string(),
            content,
            tool_call_id: Some(tool_call.id.clone()),
            reasoning_content: None,
            tags: vec![],
            is_finished: true,
            is_streaming: false,
        })
        .await
        .map_err(|e| HandlerError::AddMessage(e.to_string()))?;

    tracing::debug!(chat_id, tool_call_id = %tool_call.id, "tool result pushed");
    Ok(())
}

/// True if the chat already contains a `tool`-role message answering this call id.
async fn has_tool_result(
    client: &ChatClient,
    chat_id: i64,
    tool_call_id: &str,
) -> Result<bool, HandlerError> {
    let chat = client
        .get_chat(GetChatParams {
            chat_id,
            if_version_higher_than: None,
        })
        .await
        .map_err(|e| HandlerError::GetChat(e.to_string()))?;

    Ok(chat
        .messages
        .iter()
        .any(|m| m.tool_call_id.as_deref() == Some(tool_call_id)))
}

#[derive(Debug, thiserror::Error)]
pub enum HandlerError {
    #[error("failed to fetch chat: {0}")]
    GetChat(String),
    #[error("failed to add tool message: {0}")]
    AddMessage(String),
}
```

### 3. `plugins/rhd_plugin_mcp/src/plugin.rs`

**Modification A — imports:** add

```rust
use crate::tool_handler;
```

(No `rhd_chat_api` import needed here: the event type is inferred from the `on_tool_call` closure signature.)

**Modification B — replace the step-8 marker** (currently `// 8. Phase 5 inserts ...`) with:

```rust
    // 8. Execute tool calls for the prefixed names we registered.
    {
        let client_for_tools = Arc::clone(&client);
        let pool_for_tools = Arc::clone(&pool);
        let regs_for_tools = Arc::clone(&registrations);

        let _tool_call_token = client.on_tool_call(0, pool.all_tool_names(), move |event| {
            let client = Arc::clone(&client_for_tools);
            let pool = Arc::clone(&pool_for_tools);
            let regs = Arc::clone(&regs_for_tools);

            async move {
                tracing::debug!(
                    chat_id = event.chat_id,
                    tool_count = event.message.tool_calls.len(),
                    "received MCP tool call event"
                );
                tool_handler::handle_tool_calls(client, pool, regs, event).await;
            }
        });
        tracing::info!("Subscribed to MCP tool calls");
    }
```

## Integration points with existing code

- **Subscription filter is exact-match:** `client.rs` dispatch compares `sub.tool_names` against `data.tool_names` (the extracted names of the event). `pool.all_tool_names()` is the complete prefixed set, known at startup — no dynamic resubscription needed.
- **Event dispatch already `tokio::spawn`s the callback** (`client.rs` ~line 534), so awaiting inside the handler does not block the WebSocket read task — compliant with the `memory/development.md` blocking rule.
- **Tool message contract:** `role: "tool"` requires `toolCallId` (server rejects without it); the id reaches the provider as `tool.tool_call_id` on the next request (plugins/README.md "Tool Messages").
- **Loop continuation:** `rhd_plugin_ai_completions` re-triggers once every tool call id in the last assistant message is answered — our per-call `add_message` is exactly the resolution signal.

## Tests

Authored in Phase 6:
- `handle_tool_calls` with a manually constructed `AssistantMessageWithToolCallsData` against the in-process server + stub MCP: result message appears with correct `tool_call_id`/content.
- Duplicate guard: running the handler twice yields one tool message.
- Unregistered-server path: no tool message, no crash.
- Error path: pool call failure still produces a `tool` message containing "MCP tool call failed".

## Implementation Notes

1. **Answer-or-skip invariant:** every call we own and haven't answered gets exactly one tool message. Calls we don't own, aren't registered for the chat, or were already answered are skipped silently (debug/warn logs) — the owner or a previous run answers them.
2. **Errors as content** (not `isError` propagation): the chat protocol has no error flag on messages; the model reads the text. This matches the stale-behavior note in the old MCP feature doc and keeps the tool loop unblocked.
3. **Duplicate guard via `get_chat`:** proven pattern from `rhd_plugin_todo_list/src/tool_handler.rs`. The monitor's cached `ChatState.messages` could serve the same purpose but may lag behind the just-added assistant message; a fresh fetch is the safe choice. Cost: one request per call — acceptable for tool-call rates.
4. **`is_error` from MCP** is logged at warn but does not change handling — content is pushed either way.
5. **Arguments pass-through:** `tool_call.function.arguments` is a JSON string; `McpClient::call_tool` parses it (falls back to `{}` on malformed JSON). No re-serialization needed.
6. **Concurrent chats:** handlers for different chats run on separate spawned tasks; same-server calls serialize on the pool mutex. No shared mutable state besides `registrations` (`RwLock`) and the client (cloneable).
7. **Token binding:** `_tool_call_token` must live until the keep-alive loop (see Phase 4 note 7) — dropping it would cancel the subscription on the next dispatched event.

## Dependencies

- Requires Phases 3 (`route`, `call_tool`, `all_tool_names`) and 4 (`ChatRegistrations`, insertion point).
- Required by Phase 6 (integration tests).
