use std::sync::Arc;

use rhd_db::{ChatDb, FunctionCall, Message, ToolCall};
use rhd_fsm::tool_loop_fsm::{ChatMessage as FsmChatMessage, ToolLoopFsmEvent, ToolLoopListenerCallback, ToolCall as FsmToolCall};
use tokio::sync::broadcast;

use crate::error::ChatError;
use crate::event::ChatEvent;

/// Creates a DB synchronization listener callback
///
/// This listener receives FSM events and performs corresponding database operations
/// to keep the database in sync with the FSM's state.
///
/// # Arguments
///
/// * `db` - Arc reference to the ChatDb instance
/// * `chat_id` - The ID of the chat being synchronized
/// * `event_sender` - Broadcast sender for emitting ChatEvents to WebSocket clients
///
/// # Returns
///
/// A `ToolLoopListenerCallback` that can be registered with the FSM
pub fn create_db_sync_listener(
    db: Arc<ChatDb>,
    chat_id: i64,
    event_sender: broadcast::Sender<ChatEvent>,
) -> ToolLoopListenerCallback {
    Arc::new(move |event: ToolLoopFsmEvent| {
        let result = handle_event(&db, chat_id, &event_sender, &event);
        if let Err(e) = result {
            eprintln!("DB sync listener error: {}", e);
        }
    })
}

/// Handle a single FSM event and perform corresponding DB operations
fn handle_event(
    db: &ChatDb,
    chat_id: i64,
    event_sender: &broadcast::Sender<ChatEvent>,
    event: &ToolLoopFsmEvent,
) -> Result<(), ChatError> {
    match event {
        ToolLoopFsmEvent::MessageInserted { message } => {
            handle_message_inserted(db, chat_id, event_sender, message)
        }
        ToolLoopFsmEvent::MessageRemoved { message_id } => {
            handle_message_removed(db, chat_id, event_sender, *message_id)
        }
        ToolLoopFsmEvent::MessageReplaced { message_id, new_message } => {
            handle_message_replaced(db, chat_id, event_sender, *message_id, new_message)
        }
        ToolLoopFsmEvent::AllMessagesReplaced { messages } => {
            handle_all_messages_replaced(db, chat_id, event_sender, messages)
        }
        ToolLoopFsmEvent::ToolCallIdGenerated { tool_call_id: _ } => {
            // Tool call IDs are managed by FSM, no DB sync needed
            Ok(())
        }
        ToolLoopFsmEvent::ToolCallRequested { .. } => {
            // Tool call requests are handled by the async wrapper
            Ok(())
        }
        ToolLoopFsmEvent::ToolCallExecuted { .. } => {
            // Tool call execution is handled by the async wrapper
            Ok(())
        }
        ToolLoopFsmEvent::AiResponseReceived { .. } => {
            // AI response handling is done by the async wrapper
            Ok(())
        }
        ToolLoopFsmEvent::StateChanged { .. } => {
            // State changes don't require DB updates
            Ok(())
        }
    }
}

/// Handle MessageInserted event - add message to DB
fn handle_message_inserted(
    db: &ChatDb,
    chat_id: i64,
    event_sender: &broadcast::Sender<ChatEvent>,
    message: &FsmChatMessage,
) -> Result<(), ChatError> {
    // Convert FSM message to DB message
    let db_message = convert_fsm_message_to_db(message, chat_id);

    // Insert message into DB
    let inserted = db.insert_message(&db_message)?;

    // Emit ChatEvent to notify WebSocket clients
    let _ = event_sender.send(ChatEvent::MessageAdded {
        chat_id,
        message: inserted,
    });

    Ok(())
}

/// Handle MessageRemoved event - remove message from DB
fn handle_message_removed(
    db: &ChatDb,
    chat_id: i64,
    event_sender: &broadcast::Sender<ChatEvent>,
    message_id: i64,
) -> Result<(), ChatError> {
    // Remove message from DB
    db.delete_message(message_id)?;

    // Emit ChatEvent to notify WebSocket clients
    let _ = event_sender.send(ChatEvent::MessageRemoved {
        chat_id,
        message_id,
    });

    Ok(())
}

/// Handle MessageReplaced event - update message in DB
fn handle_message_replaced(
    db: &ChatDb,
    chat_id: i64,
    event_sender: &broadcast::Sender<ChatEvent>,
    message_id: i64,
    new_message: &FsmChatMessage,
) -> Result<(), ChatError> {
    // Convert FSM message to DB message
    let db_message = convert_fsm_message_to_db(new_message, chat_id);

    // Update message in DB
    let updated = db.update_message_full(&db_message)?;

    // Emit ChatEvent to notify WebSocket clients
    let _ = event_sender.send(ChatEvent::MessageReplaced {
        chat_id,
        message: updated,
    });

    Ok(())
}

/// Handle AllMessagesReplaced event - truncate and re-add all messages
fn handle_all_messages_replaced(
    db: &ChatDb,
    chat_id: i64,
    event_sender: &broadcast::Sender<ChatEvent>,
    messages: &[FsmChatMessage],
) -> Result<(), ChatError> {
    // Delete all existing messages for this chat
    db.delete_all_messages(chat_id)?;

    // Insert all new messages
    for message in messages {
        let db_message = convert_fsm_message_to_db(message, chat_id);
        let inserted = db.insert_message(&db_message)?;

        // Emit ChatEvent for each inserted message
        let _ = event_sender.send(ChatEvent::MessageAdded {
            chat_id,
            message: inserted,
        });
    }

    Ok(())
}

/// Convert FSM ChatMessage to DB Message
fn convert_fsm_message_to_db(
    fsm_message: &FsmChatMessage,
    chat_id: i64,
) -> Message {
    Message {
        id: fsm_message.id,
        chat_id,
        role: fsm_message.role.clone(),
        content: fsm_message.content.clone(),
        thinking_content: fsm_message.thinking_content.clone(),
        tool_calls: fsm_message.tool_calls.as_ref().map(|tcs| {
            tcs.iter()
                .map(|tc: &FsmToolCall| ToolCall {
                    id: tc.id.clone(),
                    function: FunctionCall {
                        name: tc.name.clone(),
                        arguments: tc.arguments.clone(),
                    },
                })
                .collect()
        }),
        model: None,
        created_at: chrono::Utc::now().to_rfc3339(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rhd_fsm::tool_loop_fsm::ChatMessage as FsmChatMessage;
    use rhd_fsm::tool_loop_fsm::ToolCall as FsmToolCall;

    #[test]
    fn test_convert_fsm_message_to_db() {
        let fsm_message = FsmChatMessage {
            id: 1,
            role: "user".to_string(),
            content: "Hello".to_string(),
            thinking_content: None,
            tool_calls: None,
        };

        let db_message = convert_fsm_message_to_db(&fsm_message, 100);
        
        assert_eq!(db_message.id, 1);
        assert_eq!(db_message.chat_id, 100);
        assert_eq!(db_message.role, "user");
        assert_eq!(db_message.content, "Hello");
        assert_eq!(db_message.thinking_content, None);
        assert_eq!(db_message.tool_calls, None);
    }

    #[test]
    fn test_convert_fsm_message_to_db_with_tool_calls() {
        let fsm_message = FsmChatMessage {
            id: 2,
            role: "assistant".to_string(),
            content: "I'll use the tool".to_string(),
            thinking_content: Some("Thinking...".to_string()),
            tool_calls: Some(vec![FsmToolCall {
                id: "call_1".to_string(),
                name: "test_tool".to_string(),
                arguments: "{\"arg\":\"value\"}".to_string(),
            }]),
        };

        let db_message = convert_fsm_message_to_db(&fsm_message, 200);
        
        assert_eq!(db_message.id, 2);
        assert_eq!(db_message.chat_id, 200);
        assert_eq!(db_message.role, "assistant");
        assert_eq!(db_message.content, "I'll use the tool");
        assert_eq!(db_message.thinking_content, Some("Thinking...".to_string()));
        assert!(db_message.tool_calls.is_some());
        let tool_calls = db_message.tool_calls.unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].id, "call_1");
        assert_eq!(tool_calls[0].function.name, "test_tool");
        assert_eq!(tool_calls[0].function.arguments, "{\"arg\":\"value\"}");
    }
}
