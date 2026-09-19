//! Executes queued slash-commands during `ai_completions:preDrainQueue`.

use std::collections::HashSet;

use rhd_chat_api::{
    AddQueueMessageParams, DeleteQueueMessageParams, GetQueueMessagesParams, Message,
    UpdateChatParams, UpdateQueueMessageParams,
};
use rhd_chat_client::ChatClient;

use crate::config::{CommandRegistry, ResolvedStep};
use crate::parser;
use crate::prompt_tag;

/// Process one chat's queue: parse commands, execute steps, finalize messages.
///
/// Only messages with role "user" from the pre-execution snapshot are parsed —
/// prompt messages inserted during this run are never re-scanned (a prompt file
/// whose text starts with '/' cannot recurse).
pub async fn process_queue(
    client: &ChatClient,
    registry: &CommandRegistry,
    chat_id: i64,
) -> Result<(), ExecutionError> {
    let names: HashSet<String> = registry.names();
    let snapshot = client
        .get_queue_messages(GetQueueMessagesParams { chat_id })
        .await
        .map_err(|e| ExecutionError::QueueFetch(e.to_string()))?;

    for msg in snapshot.messages {
        if msg.role != "user" {
            continue;
        }
        let Some(parsed) = parser::parse(&msg.content, &names) else {
            continue; // no recognized commands — leave byte-for-byte untouched
        };

        // 1. Execute steps in occurrence order (multi commands expand in place).
        for invocation in &parsed.invocations {
            let Some(steps) = registry.steps(invocation) else {
                continue;
            };
            for step in steps {
                if let Err(e) = apply_step(client, chat_id, msg.id, step).await {
                    // Partial application is possible on failure; log with full
                    // context and continue with the remaining messages. The
                    // event is still acknowledged so the chat is never parked.
                    tracing::error!(
                        chat_id = chat_id,
                        message_id = msg.id,
                        command = %invocation,
                        "command step failed: {}", e
                    );
                    break;
                }
            }
        }

        // 2. Finalize the carrying message. This `?` is the only early return
        // in the loop: a finalize failure aborts the remaining messages of
        // this queue pass (still logged upstream, and the event is acked
        // anyway by the caller). Per-message step failures never propagate.
        finalize_message(client, &msg, &parsed.remainder).await?;
    }
    Ok(())
}

/// Apply a single resolved step. Errors only from the client call itself.
async fn apply_step(
    client: &ChatClient,
    chat_id: i64,
    message_id: i64,
    step: &ResolvedStep,
) -> Result<(), ExecutionError> {
    match step {
        ResolvedStep::ChatTags { add, remove } => {
            client
                .update_chat(UpdateChatParams {
                    chat_id,
                    title: None,
                    add_tags: add.clone(),
                    remove_tags: remove.clone(),
                })
                .await
                .map_err(|e| ExecutionError::ChatTags(e.to_string()))?;
        }
        ResolvedStep::MessageTags { add, remove } => {
            client
                .update_queue_message(UpdateQueueMessageParams {
                    message_id,
                    content: None,
                    reasoning_content: None,
                    role: None,
                    add_tags: add.clone(),
                    remove_tags: remove.clone(),
                })
                .await
                .map_err(|e| ExecutionError::MessageTags(e.to_string()))?;
        }
        ResolvedStep::Prompt {
            name,
            role,
            content,
        } => {
            // Insert directly before the carrying message. Repeats before the
            // same anchor stay in config order (Phase 1 semantics).
            client
                .add_queue_message(AddQueueMessageParams {
                    chat_id,
                    role: role.clone(),
                    content: content.clone(),
                    tool_call_id: None,
                    reasoning_content: None,
                    tags: vec![prompt_tag(name)],
                    before_message_id: Some(message_id),
                })
                .await
                .map_err(|e| ExecutionError::PromptInsert(e.to_string()))?;
        }
    }
    tracing::debug!(chat_id, message_id, ?step, "command step applied");
    Ok(())
}

/// Strip the executed commands from the carrying message, or delete it when
/// nothing but whitespace remains.
async fn finalize_message(
    client: &ChatClient,
    msg: &Message,
    remainder: &str,
) -> Result<(), ExecutionError> {
    match finalize_plan(&msg.content, remainder) {
        FinalizeAction::Keep => {}
        FinalizeAction::Delete => {
            client
                .delete_queue_message(DeleteQueueMessageParams { message_id: msg.id })
                .await
                .map_err(|e| ExecutionError::MessageDelete(e.to_string()))?;
            tracing::info!(
                chat_id = msg.chat_id,
                message_id = msg.id,
                "command message consumed"
            );
        }
        FinalizeAction::Update(content) => {
            client
                .update_queue_message(UpdateQueueMessageParams {
                    message_id: msg.id,
                    content: Some(content),
                    reasoning_content: None,
                    role: None,
                    add_tags: vec![],
                    remove_tags: vec![],
                })
                .await
                .map_err(|e| ExecutionError::MessageUpdate(e.to_string()))?;
        }
    }
    Ok(())
}

/// What finalization should do with a command-carrying message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FinalizeAction {
    /// Content already equals the remainder — no request to issue.
    Keep,
    /// Remainder is empty/whitespace — the message is fully consumed.
    Delete,
    /// Replace the content with the remainder string.
    Update(String),
}

/// Pure decision table behind [`finalize_message`]'s branches.
pub fn finalize_plan(current: &str, remainder: &str) -> FinalizeAction {
    if remainder.trim().is_empty() {
        FinalizeAction::Delete
    } else if remainder != current {
        FinalizeAction::Update(remainder.to_string())
    } else {
        FinalizeAction::Keep
    }
}

/// Errors that can occur while executing a chat's queued commands.
#[derive(Debug, thiserror::Error)]
pub enum ExecutionError {
    #[error("failed to fetch queued messages: {0}")]
    QueueFetch(String),
    #[error("failed to update chat tags: {0}")]
    ChatTags(String),
    #[error("failed to update message tags: {0}")]
    MessageTags(String),
    #[error("failed to insert prompt message: {0}")]
    PromptInsert(String),
    #[error("failed to update message content: {0}")]
    MessageUpdate(String),
    #[error("failed to delete message: {0}")]
    MessageDelete(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn s(v: &str) -> String {
        v.to_string()
    }

    #[test]
    fn test_finalize_plan_decision_table() {
        // Command-only message is consumed.
        assert_eq!(finalize_plan("/a", ""), FinalizeAction::Delete);
        assert_eq!(finalize_plan("/a", "   "), FinalizeAction::Delete);
        // Command + text strips the commands.
        assert_eq!(
            finalize_plan("/a hello", "hello"),
            FinalizeAction::Update(s("hello"))
        );
        // Identical content needs no update round-trip.
        assert_eq!(finalize_plan("hello", "hello"), FinalizeAction::Keep);
    }

    /// The executor iterates `registry.steps(name)` in slice order, so a
    /// multi command's expansion order is the config order — smoke-checked
    /// here against the canonical multi_example shape.
    #[test]
    fn test_multi_command_steps_keep_config_order() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("commands")).unwrap();
        std::fs::write(dir.path().join("commands/prompt.md"), "one\n").unwrap();
        std::fs::write(dir.path().join("commands/another_prompt.md"), "two\n").unwrap();
        let yaml = "commands:\n  multi_example:\n    - type: message_tags\n      add: [some_tag]\n    - ./commands/prompt.md\n    - ./commands/another_prompt.md\n";
        let path = dir.path().join("config.yaml");
        std::fs::write(&path, yaml).unwrap();

        let registry = crate::config::load_config(path.to_str().unwrap()).unwrap();
        assert_eq!(
            registry.steps("multi_example"),
            Some(
                &[
                    ResolvedStep::MessageTags {
                        add: vec![s("some_tag")],
                        remove: vec![],
                    },
                    ResolvedStep::Prompt {
                        name: s("multi_example"),
                        role: s("user"),
                        content: "one\n".to_string(),
                    },
                    ResolvedStep::Prompt {
                        name: s("multi_example"),
                        role: s("user"),
                        content: "two\n".to_string(),
                    },
                ][..]
            )
        );
    }

    /// The shipped example config must load and validate with its sample
    /// prompt files (README promise).
    #[test]
    fn test_example_config_loads() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/config.example.yaml");
        let registry =
            crate::config::load_config(path).unwrap_or_else(|e| panic!("{path} must load: {e}"));
        assert!(Path::new(path).exists());
        assert_eq!(registry.len(), 4);
        for name in [
            "tags_example",
            "prompt_example",
            "system_prompt_example",
            "multi_example",
        ] {
            assert!(registry.has(name), "missing command {name}");
        }
    }
}
