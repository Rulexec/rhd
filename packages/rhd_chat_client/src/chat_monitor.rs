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
    /// Last known version of the chat. Used to detect missed events.
    pub version: i64,
}

/// Monitor that tracks chats and their state.
pub struct ChatMonitor {
    client: Arc<ChatClient>,
    chat_states: Arc<RwLock<HashMap<i64, ChatState>>>,
    _chats_list_token: CancellationToken,
    chat_tokens: Arc<RwLock<Vec<CancellationToken>>>,
    state_change_callbacks: Arc<RwLock<Vec<Box<dyn Fn(i64, ChatState) + Send + Sync + 'static>>>>,
}

impl ChatMonitor {
    /// Create a new chat monitor.
    pub async fn new(client: Arc<ChatClient>) -> Result<Self, ClientError> {
        let chat_states = Arc::new(RwLock::new(HashMap::new()));
        let chat_tokens = Arc::new(RwLock::new(Vec::new()));
        let state_change_callbacks: Arc<RwLock<Vec<Box<dyn Fn(i64, ChatState) + Send + Sync + 'static>>>> = Arc::new(RwLock::new(Vec::new()));

        // Tell the server to send chats list events
        client
            .subscribe_chats_list(rhd_chat_api::SubscribeChatsListParams {})
            .await?;

        // Register local callback for chats list events
        let chat_states_clone = Arc::clone(&chat_states);
        let chat_tokens_clone = Arc::clone(&chat_tokens);
        let client_clone = Arc::clone(&client);
        let callbacks_clone = Arc::clone(&state_change_callbacks);
        let chats_list_token = client.on_chats_list_event(move |event| {
            let states = Arc::clone(&chat_states_clone);
            let tokens = Arc::clone(&chat_tokens_clone);
            let client = Arc::clone(&client_clone);
            let callbacks: Arc<RwLock<Vec<Box<dyn Fn(i64, ChatState) + Send + Sync + 'static>>>> = Arc::clone(&callbacks_clone);
            async move {
                match event {
                    ChatsListEvent::ChatCreated(data) => {
                        let chat_id = data.chat.id;
                        tracing::debug!(
                            event = "chatCreated",
                            chat_id = chat_id,
                            "received chat created event"
                        );
                        match client.get_chat(GetChatParams { chat_id, if_version_higher_than: None }).await {
                            Ok(chat_result) => {
                                tracing::debug!(
                                    chat_id = chat_id,
                                    messages_count = chat_result.messages.len(),
                                    queued_messages_count = chat_result.queued_messages_count,
                                    "fetched initial chat state"
                                );
                            let state = ChatState {
                                chat_id,
                                messages: chat_result.messages,
                                queued_messages_count: chat_result.queued_messages_count,
                                tags: data.chat.tags.clone(),
                                version: chat_result.chat.version,
                            };
                            states.write().await.insert(chat_id, state.clone());
                            
                            // Notify callbacks about state change
                            let cbs = callbacks.read().await;
                            for cb in cbs.iter() {
                                cb(chat_id, state.clone());
                            }
                            
                            // Tell server to send events for this chat
                            let _ = client.subscribe_chat(rhd_chat_api::SubscribeChatParams { chat_id }).await;
                            
                            // Fetch chat state again after subscribing to catch any changes
                            // that happened between the initial get_chat and subscribe
                            if let Ok(latest_chat_result) = client.get_chat(GetChatParams { chat_id, if_version_higher_than: None }).await {
                                let state = ChatState {
                                    chat_id,
                                    messages: latest_chat_result.messages,
                                    queued_messages_count: latest_chat_result.queued_messages_count,
                                    tags: data.chat.tags.clone(),
                                    version: latest_chat_result.chat.version,
                                };
                                states.write().await.insert(chat_id, state.clone());
                                
                                // Notify callbacks about state change
                                let cbs = callbacks.read().await;
                                for cb in cbs.iter() {
                                    cb(chat_id, state.clone());
                                }
                            }
                            
                            // Subscribe to the new chat's events
                            let chat_states_for_sub = Arc::clone(&states);
                            let client_for_sub = Arc::clone(&client);
                            let callbacks_for_sub: Arc<RwLock<Vec<Box<dyn Fn(i64, ChatState) + Send + Sync + 'static>>>> = Arc::clone(&callbacks);
                            let token = client.on_chat_event(chat_id, move |event| {
                                let states = Arc::clone(&chat_states_for_sub);
                                let client = Arc::clone(&client_for_sub);
                                let callbacks: Arc<RwLock<Vec<Box<dyn Fn(i64, ChatState) + Send + Sync + 'static>>>> = Arc::clone(&callbacks_for_sub);
                                async move {
                                    // Extract version from event
                                    let event_version = match &event {
                                        ChatEvent::MessageAdded(data) => data.chat_version,
                                        ChatEvent::MessageUpdated(data) => data.chat_version,
                                        ChatEvent::MessageDeleted(data) => data.chat_version,
                                        ChatEvent::QueueMessageAdded(data) => data.chat_version,
                                        ChatEvent::QueueMessageUpdated(data) => data.chat_version,
                                        ChatEvent::QueueMessageDeleted(data) => data.chat_version,
                                        ChatEvent::ToolsUpdated(data) => data.chat_version,
                                        // Streaming events don't have chat_version, use 0 to skip version check
                                        ChatEvent::StreamChunk(_) => 0,
                                        ChatEvent::StreamFinished(_) => 0,
                                    };

                                    // Check local state version
                                    let local_version = {
                                        let states_read = states.read().await;
                                        states_read.get(&chat_id).map(|s| s.version)
                                    };

                                    // Validate version
                                    match local_version {
                                        Some(local_ver) if event_version <= local_ver => {
                                            // Stale event, ignore
                                            tracing::debug!(
                                                chat_id = chat_id,
                                                event_version = event_version,
                                                local_version = local_ver,
                                                "ignoring stale event"
                                            );
                                            return;
                                        }
                                        Some(local_ver) if event_version > local_ver + 1 => {
                                            // Version gap detected, refetch state
                                            tracing::warn!(
                                                chat_id = chat_id,
                                                event_version = event_version,
                                                local_version = local_ver,
                                                "version gap detected, refetching state"
                                            );
                                            if let Ok(chat_result) = client
                                                .get_chat(GetChatParams {
                                                    chat_id,
                                                    if_version_higher_than: Some(local_ver),
                                                })
                                                .await
                                            {
                                                let mut states_write = states.write().await;
                                                if let Some(state) = states_write.get_mut(&chat_id) {
                                                    state.messages = chat_result.messages;
                                                    state.queued_messages_count = chat_result.queued_messages_count;
                                                    state.version = chat_result.chat.version;
                                                    
                                                    // Notify callbacks about state change
                                                    let cbs = callbacks.read().await;
                                                    for cb in cbs.iter() {
                                                        cb(chat_id, state.clone());
                                                    }
                                                }
                                            }
                                            return;
                                        }
                                        _ => {
                                            // Normal case: event_version == local_ver + 1 or first event
                                            tracing::debug!(
                                                chat_id = chat_id,
                                                event_version = event_version,
                                                "processing event"
                                            );
                                        }
                                    }

                                    match event {
                                        ChatEvent::MessageAdded(_)
                                        | ChatEvent::MessageUpdated(_)
                                        | ChatEvent::MessageDeleted(_)
                                        | ChatEvent::QueueMessageAdded(_)
                                        | ChatEvent::QueueMessageUpdated(_)
                                        | ChatEvent::QueueMessageDeleted(_)
                                        | ChatEvent::StreamChunk(_)
                                        | ChatEvent::StreamFinished(_) => {
                                            tracing::debug!(chat_id = chat_id, "updating chat state after event");
                                            if let Ok(chat_result) =
                                                client.get_chat(GetChatParams { chat_id, if_version_higher_than: None }).await
                                            {
                                                let mut states = states.write().await;
                                                if let Some(state) = states.get_mut(&chat_id) {
                                                    state.messages = chat_result.messages;
                                                    state.queued_messages_count = chat_result.queued_messages_count;
                                                    state.version = chat_result.chat.version;
                                                    
                                                    // Notify callbacks about state change
                                                    let cbs = callbacks.read().await;
                                                    for cb in cbs.iter() {
                                                        cb(chat_id, state.clone());
                                                    }
                                                }
                                            }
                                        }
                                        ChatEvent::ToolsUpdated(_) => {
                                            // Tools updated - no action needed
                                        }
                                    }
                                }
                            });
                            tokens.write().await.push(token);
                            }
                            Err(e) => {
                                tracing::debug!("get_chat failed for chat_id={}: {}", chat_id, e);
                            }
                        }
                    }
                    ChatsListEvent::ChatUpdated(data) => {
                        if let Ok(chat_result) =
                            client.get_chat(GetChatParams { chat_id: data.chat.id, if_version_higher_than: None }).await
                        {
                            let state = ChatState {
                                chat_id: data.chat.id,
                                messages: chat_result.messages,
                                queued_messages_count: chat_result.queued_messages_count,
                                tags: data.chat.tags,
                                version: chat_result.chat.version,
                            };
                            states.write().await.insert(data.chat.id, state.clone());
                            
                            // Notify callbacks about state change
                            let cbs = callbacks.read().await;
                            for cb in cbs.iter() {
                                cb(data.chat.id, state.clone());
                            }
                        }
                    }
                    ChatsListEvent::ChatDeleted(data) => {
                        states.write().await.remove(&data.chat_id);
                    }
                }
            }
        }).await;

        // Fetch initial chat list
        let list_result = client.list_chats(ListChatsParams { tags: vec![] }).await?;
        for chat_summary in list_result.chats {
            if let Ok(chat_result) = client.get_chat(GetChatParams { chat_id: chat_summary.id, if_version_higher_than: None }).await
            {
                let state = ChatState {
                    chat_id: chat_summary.id,
                    messages: chat_result.messages,
                    queued_messages_count: chat_result.queued_messages_count,
                    tags: chat_summary.tags,
                    version: chat_result.chat.version,
                };
                chat_states.write().await.insert(chat_summary.id, state);
            }
        }

        Ok(Self {
            client,
            chat_states,
            _chats_list_token: chats_list_token,
            chat_tokens,
            state_change_callbacks,
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
        // Tell the server to send events for this chat
        self.client
            .subscribe_chat(rhd_chat_api::SubscribeChatParams { chat_id })
            .await?;

        let chat_states = Arc::clone(&self.chat_states);
        let client = Arc::clone(&self.client);
        let callbacks: Arc<RwLock<Vec<Box<dyn Fn(i64, ChatState) + Send + Sync + 'static>>>> = Arc::clone(&self.state_change_callbacks);

        // Register local callback for chat events
        let token = self.client.on_chat_event(chat_id, move |event| {
            let states = Arc::clone(&chat_states);
            let client = Arc::clone(&client);
            let callbacks = Arc::clone(&callbacks);
            async move {
                // Extract version from event
                let event_version = match &event {
                    ChatEvent::MessageAdded(data) => data.chat_version,
                    ChatEvent::MessageUpdated(data) => data.chat_version,
                    ChatEvent::MessageDeleted(data) => data.chat_version,
                    ChatEvent::QueueMessageAdded(data) => data.chat_version,
                    ChatEvent::QueueMessageUpdated(data) => data.chat_version,
                    ChatEvent::QueueMessageDeleted(data) => data.chat_version,
                    ChatEvent::ToolsUpdated(data) => data.chat_version,
                    // Streaming events don't have chat_version, use 0 to skip version check
                    ChatEvent::StreamChunk(_) => 0,
                    ChatEvent::StreamFinished(_) => 0,
                };

                // Check local state version
                let local_version = {
                    let states_read = states.read().await;
                    states_read.get(&chat_id).map(|s| s.version)
                };

                // Validate version
                match local_version {
                    Some(local_ver) if event_version <= local_ver => {
                        // Stale event, ignore
                        tracing::debug!(
                            chat_id = chat_id,
                            event_version = event_version,
                            local_version = local_ver,
                            "ignoring stale event"
                        );
                        return;
                    }
                    Some(local_ver) if event_version > local_ver + 1 => {
                        // Version gap detected, refetch state
                        tracing::warn!(
                            chat_id = chat_id,
                            event_version = event_version,
                            local_version = local_ver,
                            "version gap detected, refetching state"
                        );
                        if let Ok(chat_result) = client
                            .get_chat(GetChatParams {
                                chat_id,
                                if_version_higher_than: Some(local_ver),
                            })
                            .await
                        {
                            let mut states_write = states.write().await;
                            if let Some(state) = states_write.get_mut(&chat_id) {
                                state.messages = chat_result.messages;
                                state.queued_messages_count = chat_result.queued_messages_count;
                                state.version = chat_result.chat.version;
                                
                                // Notify callbacks about state change
                                let cbs = callbacks.read().await;
                                for cb in cbs.iter() {
                                    cb(chat_id, state.clone());
                                }
                            }
                        }
                        return;
                    }
                    _ => {
                        // Normal case: event_version == local_ver + 1 or first event
                        tracing::debug!(
                            chat_id = chat_id,
                            event_version = event_version,
                            "processing event"
                        );
                    }
                }

                match event {
                    ChatEvent::MessageAdded(_)
                    | ChatEvent::MessageUpdated(_)
                    | ChatEvent::MessageDeleted(_)
                    | ChatEvent::QueueMessageAdded(_)
                    | ChatEvent::QueueMessageUpdated(_)
                    | ChatEvent::QueueMessageDeleted(_)
                    | ChatEvent::StreamChunk(_)
                    | ChatEvent::StreamFinished(_) => {
                        if let Ok(chat_result) =
                            client.get_chat(GetChatParams { chat_id, if_version_higher_than: None }).await
                        {
                            let mut states = states.write().await;
                            if let Some(state) = states.get_mut(&chat_id) {
                                state.messages = chat_result.messages;
                                state.queued_messages_count = chat_result.queued_messages_count;
                                state.version = chat_result.chat.version;
                                
                                // Notify callbacks about state change
                                let cbs = callbacks.read().await;
                                for cb in cbs.iter() {
                                    cb(chat_id, state.clone());
                                }
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
        let chat_result = self.client.get_chat(GetChatParams { chat_id, if_version_higher_than: None }).await?;
        let state = ChatState {
            chat_id,
            messages: chat_result.messages,
            queued_messages_count: chat_result.queued_messages_count,
            tags: chat_result.chat.tags,
            version: chat_result.chat.version,
        };
        self.chat_states.write().await.insert(chat_id, state);
        Ok(())
    }

    /// Register a callback to be notified when a chat's state changes.
    /// The callback receives the chat_id and the new ChatState.
    pub async fn on_chat_state_change<F>(&self, callback: F)
    where
        F: Fn(i64, ChatState) + Send + Sync + 'static,
    {
        self.state_change_callbacks
            .write()
            .await
            .push(Box::new(callback));
    }

    /// Notify all registered callbacks about a chat state change.
    async fn notify_state_change(&self, chat_id: i64, state: ChatState) {
        let callbacks = self.state_change_callbacks.read().await;
        for callback in callbacks.iter() {
            callback(chat_id, state.clone());
        }
    }
}
