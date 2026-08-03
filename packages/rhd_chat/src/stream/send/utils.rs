use std::sync::Arc;

use rhd_ai::client::AiError;
use rhd_db::Message;
use tokio::sync::{broadcast, Mutex};

use crate::chat_log::ChatLogSink;
use crate::error::ChatError;
use crate::event::ChatEvent;
use crate::manager::ChatManager;
use crate::ProjectProvider;

pub(super) fn format_messages_for_log(messages: &[Message]) -> String {
    let mut output = String::new();
    for msg in messages {
        match msg.role.as_str() {
            "system" => {
                output.push_str(&format!("[system] {}\n", msg.content));
            }
            "user" => {
                output.push_str(&format!("[user] {}\n", msg.content));
            }
            "assistant" => {
                output.push_str(&format!("[assistant] {}\n", msg.content));
            }
            "tool" => {
                output.push_str(&format!("[tool {}] {}\n", msg.id, msg.content));
            }
            _ => {
                output.push_str(&format!("[{}] {}\n", msg.role, msg.content));
            }
        }
    }
    output
}

pub(super) async fn handle_stream_result<P: ProjectProvider>(
    manager: &ChatManager<P>,
    result: Result<rhd_ai::client::StreamResult, AiError>,
    chat_id: i64,
    model: &str,
    accumulated_content: Arc<Mutex<String>>,
    accumulated_thinking: Arc<Mutex<String>>,
    event_sender: &broadcast::Sender<ChatEvent>,
    chat_log: Option<&mut ChatLogSink>,
) -> Result<i64, ChatError> {
    match result {
        Ok(stream_result) => {
            let full_content = accumulated_content.lock().await.clone();
            let full_thinking = accumulated_thinking.lock().await.clone();
            let finish_reason = stream_result
                .finish_reason
                .clone()
                .unwrap_or_else(|| "stop".to_string());
            
            if let Some(sink) = chat_log {
                let reasoning = if full_thinking.is_empty() {
                    None
                } else {
                    Some(full_thinking.as_str())
                };
                sink.log_assistant_response(
                    reasoning,
                    &full_content,
                    &finish_reason,
                    stream_result.usage.as_ref(),
                );
                sink.log_stream_finished(&finish_reason, 0);
            }
            
            let thinking_option = if full_thinking.is_empty() {
                None
            } else {
                Some(full_thinking.as_str())
            };
            let assistant_message = manager.add_message_and_notify(
                chat_id,
                "assistant",
                &full_content,
                Some(model),
                thinking_option,
                event_sender,
            )?;
            let assistant_message_id = assistant_message.id;
            
            let _ = event_sender.send(ChatEvent::StreamFinished {
                chat_id,
                message_id: assistant_message_id,
                finish_reason,
            });
            Ok(assistant_message_id)
        }
        Err(AiError::Aborted { .. }) => {
            if let Some(sink) = chat_log {
                sink.log_stream_error("aborted");
            }
            let _ = event_sender.send(ChatEvent::StreamAborted { chat_id });
            Err(ChatError::Ai(AiError::Aborted {
                model: model.to_string(),
            }))
        }
        Err(err) => {
            if let Some(sink) = chat_log {
                sink.log_stream_error(&err.to_string());
            }
            let _ = event_sender.send(ChatEvent::StreamError {
                chat_id,
                error: err.to_string(),
            });
            Err(ChatError::Ai(err))
        }
    }
}
