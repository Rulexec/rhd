use std::collections::HashMap;
use std::sync::Arc;

use rhd_ai::client::{ChatMessage, OpenAiClient};
use rhd_ai::config::ModelConfig;
use rhd_db::{ChatDb, ChatInfo, Message};
use thiserror::Error;
use tokio::sync::{broadcast, Mutex};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Error)]
pub enum ChatError {
    #[error("database error: {0}")]
    Db(#[from] rhd_db::DbError),

    #[error("AI error: {0}")]
    Ai(#[from] rhd_ai::client::AiError),

    #[error("chat not found")]
    ChatNotFound,

    #[error("message not found")]
    MessageNotFound,

    #[error("model not found: {0}")]
    ModelNotFound(String),
}

#[derive(Debug, Clone)]
pub enum ChatEvent {
    StreamChunk { chat_id: i64, content: String },
    StreamFinished { chat_id: i64, message_id: i64, finish_reason: String },
    StreamError { chat_id: i64, error: String },
    MessageAdded { chat_id: i64, message: Message },
}

pub struct ChatManager {
    db: Arc<ChatDb>,
    active_streams: Mutex<HashMap<i64, CancellationToken>>,
}

impl ChatManager {
    pub fn new(db: Arc<ChatDb>) -> Self {
        Self {
            db,
            active_streams: Mutex::new(HashMap::new()),
        }
    }

    pub fn create_chat(&self, title: &str) -> Result<i64, ChatError> {
        Ok(self.db.create_chat(title)?)
    }

    pub fn list_chats(&self) -> Result<Vec<ChatInfo>, ChatError> {
        Ok(self.db.list_chats()?)
    }

    pub fn get_chat(&self, id: i64) -> Result<Option<(ChatInfo, Vec<Message>)>, ChatError> {
        let chat = self.db.get_chat(id)?;
        match chat {
            Some(chat_info) => {
                let messages = self.db.get_messages(id)?;
                Ok(Some((chat_info, messages)))
            }
            None => Ok(None),
        }
    }

    pub fn delete_chat(&self, id: i64) -> Result<(), ChatError> {
        self.db.delete_chat(id)?;
        Ok(())
    }

    pub async fn send_message(
        &self,
        chat_id: i64,
        content: String,
        model: &str,
        models: &HashMap<String, ModelConfig>,
        event_sender: broadcast::Sender<ChatEvent>,
    ) -> Result<i64, ChatError> {
        if self.db.get_chat(chat_id)?.is_none() {
            return Err(ChatError::ChatNotFound);
        }

        let model_config = models.get(model).ok_or_else(|| ChatError::ModelNotFound(model.to_string()))?;

        // Update chat's active model
        self.db.update_chat_active_model(chat_id, model)?;

        let user_message_id = self.db.add_message(chat_id, "user", &content, Some(model))?;
        let user_message = Message {
            id: user_message_id,
            chat_id,
            role: "user".to_string(),
            content,
            created_at: chrono::Utc::now().to_rfc3339(),
            model: Some(model.to_string()),
        };
        let _ = event_sender.send(ChatEvent::MessageAdded {
            chat_id,
            message: user_message,
        });

        let messages = self.db.get_messages(chat_id)?;
        let chat_messages: Vec<ChatMessage> = messages
            .iter()
            .map(|m| match m.role.as_str() {
                "user" => ChatMessage::user(&m.content),
                "assistant" => ChatMessage::assistant(&m.content),
                "system" => ChatMessage::system(&m.content),
                _ => ChatMessage::user(&m.content),
            })
            .collect();

        let cancel_token = CancellationToken::new();
        {
            let mut active = self.active_streams.lock().await;
            if let Some(existing) = active.get(&chat_id) {
                existing.cancel();
            }
            active.insert(chat_id, cancel_token.clone());
        }

        let client = OpenAiClient::new(&model_config.base_url, &model_config.api_key);
        let accumulated_content = Arc::new(Mutex::new(String::new()));
        let accumulated_clone = accumulated_content.clone();
        let sender_for_closure = event_sender.clone();

        let result = client
            .chat_stream_cancellable(model, &chat_messages, cancel_token.clone(), move |chunk| {
                let sender = sender_for_closure.clone();
                let acc = accumulated_clone.clone();
                Box::pin(async move {
                    if let Some(content) = chunk.content {
                        if !content.is_empty() {
                            let _ = sender.send(ChatEvent::StreamChunk {
                                chat_id,
                                content: content.clone(),
                            });
                            let mut acc_guard = acc.lock().await;
                            acc_guard.push_str(&content);
                        }
                    }
                })
            })
            .await;

        {
            let mut active = self.active_streams.lock().await;
            active.remove(&chat_id);
        }

        match result {
            Ok(stream_result) => {
                let full_content = accumulated_content.lock().await.clone();
                let assistant_message_id = self.db.add_message(chat_id, "assistant", &full_content, Some(model))?;
                let assistant_message = Message {
                    id: assistant_message_id,
                    chat_id,
                    role: "assistant".to_string(),
                    content: full_content,
                    created_at: chrono::Utc::now().to_rfc3339(),
                    model: Some(model.to_string()),
                };
                let _ = event_sender.send(ChatEvent::MessageAdded {
                    chat_id,
                    message: assistant_message,
                });
                let finish_reason = stream_result.finish_reason.unwrap_or_else(|| "stop".to_string());
                let _ = event_sender.send(ChatEvent::StreamFinished {
                    chat_id,
                    message_id: assistant_message_id,
                    finish_reason,
                });
                Ok(assistant_message_id)
            }
            Err(rhd_ai::client::AiError::Aborted { .. }) => {
                let _ = event_sender.send(ChatEvent::StreamError {
                    chat_id,
                    error: "aborted".to_string(),
                });
                Err(ChatError::Ai(rhd_ai::client::AiError::Aborted {
                    model: model.to_string(),
                }))
            }
            Err(err) => {
                let _ = event_sender.send(ChatEvent::StreamError {
                    chat_id,
                    error: err.to_string(),
                });
                Err(ChatError::Ai(err))
            }
        }
    }

    pub async fn edit_and_resend(
        &self,
        message_id: i64,
        new_content: String,
        model: &str,
        models: &HashMap<String, ModelConfig>,
        event_sender: broadcast::Sender<ChatEvent>,
    ) -> Result<i64, ChatError> {
        let original_message = self.db.get_message(message_id)?.ok_or(ChatError::MessageNotFound)?;
        let chat_id = original_message.chat_id;

        self.db.update_message(message_id, &new_content)?;
        self.db.truncate_messages(chat_id, message_id)?;

        // Update chat's active model
        self.db.update_chat_active_model(chat_id, model)?;

        let updated_message = Message {
            id: message_id,
            chat_id,
            role: original_message.role.clone(),
            content: new_content,
            created_at: chrono::Utc::now().to_rfc3339(),
            model: Some(model.to_string()),
        };
        let _ = event_sender.send(ChatEvent::MessageAdded {
            chat_id,
            message: updated_message,
        });

        let messages = self.db.get_messages(chat_id)?;
        let chat_messages: Vec<ChatMessage> = messages
            .iter()
            .map(|m| match m.role.as_str() {
                "user" => ChatMessage::user(&m.content),
                "assistant" => ChatMessage::assistant(&m.content),
                "system" => ChatMessage::system(&m.content),
                _ => ChatMessage::user(&m.content),
            })
            .collect();

        let model_config = models.get(model).ok_or_else(|| ChatError::ModelNotFound(model.to_string()))?;

        let cancel_token = CancellationToken::new();
        {
            let mut active = self.active_streams.lock().await;
            if let Some(existing) = active.get(&chat_id) {
                existing.cancel();
            }
            active.insert(chat_id, cancel_token.clone());
        }

        let client = OpenAiClient::new(&model_config.base_url, &model_config.api_key);
        let accumulated_content = Arc::new(Mutex::new(String::new()));
        let accumulated_clone = accumulated_content.clone();
        let sender_for_closure = event_sender.clone();

        let result = client
            .chat_stream_cancellable(model, &chat_messages, cancel_token.clone(), move |chunk| {
                let sender = sender_for_closure.clone();
                let acc = accumulated_clone.clone();
                Box::pin(async move {
                    if let Some(content) = chunk.content {
                        if !content.is_empty() {
                            let _ = sender.send(ChatEvent::StreamChunk {
                                chat_id,
                                content: content.clone(),
                            });
                            let mut acc_guard = acc.lock().await;
                            acc_guard.push_str(&content);
                        }
                    }
                })
            })
            .await;

        {
            let mut active = self.active_streams.lock().await;
            active.remove(&chat_id);
        }

        match result {
            Ok(stream_result) => {
                let full_content = accumulated_content.lock().await.clone();
                let assistant_message_id = self.db.add_message(chat_id, "assistant", &full_content, Some(model))?;
                let assistant_message = Message {
                    id: assistant_message_id,
                    chat_id,
                    role: "assistant".to_string(),
                    content: full_content,
                    created_at: chrono::Utc::now().to_rfc3339(),
                    model: Some(model.to_string()),
                };
                let _ = event_sender.send(ChatEvent::MessageAdded {
                    chat_id,
                    message: assistant_message,
                });
                let finish_reason = stream_result.finish_reason.unwrap_or_else(|| "stop".to_string());
                let _ = event_sender.send(ChatEvent::StreamFinished {
                    chat_id,
                    message_id: assistant_message_id,
                    finish_reason,
                });
                Ok(assistant_message_id)
            }
            Err(rhd_ai::client::AiError::Aborted { .. }) => {
                let _ = event_sender.send(ChatEvent::StreamError {
                    chat_id,
                    error: "aborted".to_string(),
                });
                Err(ChatError::Ai(rhd_ai::client::AiError::Aborted {
                    model: model.to_string(),
                }))
            }
            Err(err) => {
                let _ = event_sender.send(ChatEvent::StreamError {
                    chat_id,
                    error: err.to_string(),
                });
                Err(ChatError::Ai(err))
            }
        }
    }

    pub async fn abort_chat(&self, chat_id: i64) -> bool {
        let active = self.active_streams.lock().await;
        if let Some(token) = active.get(&chat_id) {
            token.cancel();
            true
        } else {
            false
        }
    }
}
