//! Chat monitor implementation for tracking chat state.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

use rhd_chat_api::{GetChatParams, ListChatsParams, Message};

use crate::client::ChatClient;
use crate::error::ClientError;
use crate::event_stream::{CancellationToken, ChatEvent, ChatsListEvent};

/// Represents the state of a chat being monitored.
#[derive(Clone)]
pub struct ChatState {
    pub chat_id: i64,
    pub messages: Vec<Message>,
    pub queued_messages_count: i64,
    pub tags: Vec<String>,
}

/// Monitor that tracks chats and their state.
pub struct ChatMonitor {
    client: Arc<ChatClient>,
    chat_states: Arc<RwLock<HashMap<i64, ChatState>>>,
    _chats_list_token: CancellationToken,
    chat_tokens: Arc<RwLock<Vec<CancellationToken>>>,
}

impl ChatMonitor {
    /// Create a new chat monitor.
    pub async fn new(client: Arc<ChatClient>) -> Result<Self, ClientError> {
        let chat_states = Arc::new(RwLock::new(HashMap::new()));
        let chat_tokens = Arc::new(RwLock::new(Vec::new()));

        // Subscribe to chats list events
        let chat_states_clone = Arc::clone(&chat_states);
        let client_clone = Arc::clone(&client);
        let chats_list_token = client.on_chats_list_event(move |event| {
            let states = Arc::clone(&chat_states_clone);
            let client = Arc::clone(&client_clone);
            async move {
                match event {
                    ChatsListEvent::ChatCreated(data) => {
                        if let Ok(chat_result) =
                            client.get_chat(GetChatParams { chat_id: data.chat.id }).await
                        {
                            let state = ChatState {
                                chat_id: data.chat.id,
                                messages: chat_result.messages,
                                queued_messages_count: chat_result.queued_messages_count,
                                tags: data.chat.tags,
                            };
                            states.write().await.insert(data.chat.id, state);
                        }
                    }
                    ChatsListEvent::ChatUpdated(data) => {
                        if let Ok(chat_result) =
                            client.get_chat(GetChatParams { chat_id: data.chat.id }).await
                        {
                            let state = ChatState {
                                chat_id: data.chat.id,
                                messages: chat_result.messages,
                                queued_messages_count: chat_result.queued_messages_count,
                                tags: data.chat.tags,
                            };
                            states.write().await.insert(data.chat.id, state);
                        }
                    }
                    ChatsListEvent::ChatDeleted(data) => {
                        states.write().await.remove(&data.chat_id);
                    }
                }
            }
        });

        // Fetch initial chat list
        let list_result = client.list_chats(ListChatsParams { tags: vec![] }).await?;
        for chat_summary in list_result.chats {
            if let Ok(chat_result) = client.get_chat(GetChatParams { chat_id: chat_summary.id }).await
            {
                let state = ChatState {
                    chat_id: chat_summary.id,
                    messages: chat_result.messages,
                    queued_messages_count: chat_result.queued_messages_count,
                    tags: chat_summary.tags,
                };
                chat_states.write().await.insert(chat_summary.id, state);
            }
        }

        Ok(Self {
            client,
            chat_states,
            _chats_list_token: chats_list_token,
            chat_tokens,
        })
    }

    /// Subscribe to events for all monitored chats.
    pub async fn subscribe_to_all_chats(&self) -> Result<(), ClientError> {
        let chat_ids: Vec<i64> = self.chat_states.read().await.keys().cloned().collect();

        for chat_id in chat_ids {
            self.subscribe_to_chat(chat_id).await?;
        }

        Ok(())
    }

    /// Subscribe to events for a specific chat.
    async fn subscribe_to_chat(&self, chat_id: i64) -> Result<(), ClientError> {
        let chat_states = Arc::clone(&self.chat_states);
        let client = Arc::clone(&self.client);

        let token = self.client.on_chat_event(chat_id, move |event| {
            let states = Arc::clone(&chat_states);
            let client = Arc::clone(&client);
            async move {
                match event {
                    ChatEvent::MessageAdded(_)
                    | ChatEvent::MessageUpdated(_)
                    | ChatEvent::MessageDeleted(_)
                    | ChatEvent::QueueMessageAdded(_)
                    | ChatEvent::QueueMessageUpdated(_)
                    | ChatEvent::QueueMessageDeleted(_) => {
                        if let Ok(chat_result) =
                            client.get_chat(GetChatParams { chat_id }).await
                        {
                            let mut states = states.write().await;
                            if let Some(state) = states.get_mut(&chat_id) {
                                state.messages = chat_result.messages;
                                state.queued_messages_count = chat_result.queued_messages_count;
                            }
                        }
                    }
                    ChatEvent::ToolsUpdated(_) => {
                        // Tools updated - no action needed
                    }
                }
            }
        });

        self.chat_tokens.write().await.push(token);
        Ok(())
    }

    /// Get all chat IDs being monitored.
    pub async fn get_chat_ids(&self) -> Vec<i64> {
        self.chat_states.read().await.keys().cloned().collect()
    }

    /// Get chat state for a specific chat.
    pub async fn get_chat_state(&self, chat_id: i64) -> Option<ChatState> {
        self.chat_states.read().await.get(&chat_id).cloned()
    }

    /// Refresh chat state from server.
    pub async fn refresh_chat(&self, chat_id: i64) -> Result<(), ClientError> {
        let chat_result = self.client.get_chat(GetChatParams { chat_id }).await?;
        let state = ChatState {
            chat_id,
            messages: chat_result.messages,
            queued_messages_count: chat_result.queued_messages_count,
            tags: chat_result.chat.tags,
        };
        self.chat_states.write().await.insert(chat_id, state);
        Ok(())
    }
}
