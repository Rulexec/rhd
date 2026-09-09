//! WebSocket client for the chat server.

use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use exponential_backoff::Backoff;

use futures_util::{SinkExt, StreamExt};
use serde::de::DeserializeOwned;
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio::task::JoinHandle;
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, error, warn};
use uuid::Uuid;

use rhd_chat_api::protocol::{Event, Request, Response};
use rhd_chat_api::{
    AckCustomEventParams, AckCustomEventResult, AddMessageParams, AddMessageResult,
    ErrorResponse,
    AddQueueMessageParams, AddQueueMessageResult, AddToolsParams, AddToolsResult,
    CreateChatParams, CreateChatResult, DeleteChatParams, DeleteChatResult, DeleteMessageParams,
    DeleteMessageResult, DeleteQueueMessageParams, DeleteQueueMessageResult, GetChatParams,
    GetChatResult, GetMessagesParams, GetMessagesResult, GetPendingAcksParams, GetPendingAcksResult,
    GetPluginsParams, GetPluginsResult, GetPluginStatesParams, GetPluginStatesResult,
    GetQueueMessagesParams, GetQueueMessagesResult,
    GetToolsParams, GetToolsResult, ListChatsParams, ListChatsResult, RegisterPluginParams,
    RegisterPluginResult, RemovePluginParams, RemovePluginResult, RemovePluginStateParams,
    RemovePluginStateResult, RemoveToolsParams,
    RemoveToolsResult, SendCustomEventParams, SendCustomEventResult, StreamFinishParams,
    StreamFinishResult, StreamPushParams, StreamPushResult, StreamSubscribeParams,
    StreamSubscribeResult, SubscribeChatParams, SubscribeChatResult, SubscribeChatsListParams,
    SubscribeChatsListResult, SubscribePluginStatesParams, SubscribePluginStatesResult,
    SubscribePluginsListParams, SubscribePluginsListResult,
    UnsubscribeChatParams, UnsubscribeChatResult, UnsubscribeChatsListParams,
    UnsubscribeChatsListResult, UnsubscribePluginStatesParams, UnsubscribePluginStatesResult,
    UnsubscribePluginsListParams, UnsubscribePluginsListResult,
    UpdateChatParams, UpdateChatResult, UpdateMessageParams, UpdateMessageResult,
    UpdatePluginStateParams, UpdatePluginStateResult, UpdateQueueMessageParams,
    UpdateQueueMessageResult, UpdateToolCallTagsParams,
    UpdateToolCallTagsResult,
};

use crate::error::ClientError;
use crate::event_stream::{
    ChatEvent, ChatEventCallback, ChatSubscription, ChatsListEvent, ChatsListEventCallback,
    ChatsListSubscription, CancellationToken, CustomEventAcknowledgedCallback,
    CustomEventAcknowledgedSubscription, CustomEventCallback, CustomEventSubscription,
    EventSubscriptions, PluginStateEvent, PluginStateEventCallback, PluginStateSubscription,
    PluginsListEvent, PluginsListEventCallback, PluginsListSubscription,
};

/// Type alias for pending request senders.
type PendingSender = oneshot::Sender<Result<Response, ClientError>>;

/// WebSocket client for the chat server.
///
/// Provides typed async methods for all chat API operations and event subscriptions.
#[derive(Clone)]
pub struct ChatClient {
    /// Channel to send messages to the WebSocket writer.
    write_tx: mpsc::UnboundedSender<String>,
    /// Pending requests waiting for responses.
    pending_requests: Arc<Mutex<HashMap<String, PendingSender>>>,
    /// Event subscriptions.
    subscriptions: Arc<Mutex<EventSubscriptions>>,
    /// Connection task handle.
    _connection_task: Arc<JoinHandle<()>>,
}

impl ChatClient {
    /// Connect to a chat server at the given WebSocket URL.
    ///
    /// # Arguments
    /// * `url` - WebSocket URL (e.g., "ws://127.0.0.1:8080/")
    ///
    /// The client does not automatically register as a plugin. If plugin
    /// functionality is needed, call `register_plugin()` after connecting.
    pub async fn connect(url: &str) -> Result<Self, ClientError> {
        let (ws_stream, _) = tokio_tungstenite::connect_async(url).await?;
        let (mut write, mut read) = ws_stream.split();

        // Channel for outgoing messages
        let (write_tx, mut write_rx) = mpsc::unbounded_channel::<String>();

        // Pending requests map
        let pending_requests: Arc<Mutex<HashMap<String, PendingSender>>> =
            Arc::new(Mutex::new(HashMap::new()));

        // Event subscriptions
        let subscriptions = Arc::new(Mutex::new(EventSubscriptions::new()));

        // Clone for the read task
        let pending_requests_read = Arc::clone(&pending_requests);
        let subscriptions_read = Arc::clone(&subscriptions);

        // Spawn task to forward outgoing messages to WebSocket
        let write_task = tokio::spawn(async move {
            while let Some(msg) = write_rx.recv().await {
                if let Err(e) = write.send(Message::Text(msg)).await {
                    error!("Failed to send message: {}", e);
                    break;
                }
            }
        });

        // Spawn task to read incoming messages
        let read_task = tokio::spawn(async move {
            while let Some(msg) = read.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        debug!("Received raw message: {}", text);
                        Self::handle_incoming_message(
                            &text,
                            &pending_requests_read,
                            &subscriptions_read,
                        )
                        .await;
                    }
                    Ok(Message::Close(_)) => {
                        debug!("Connection closed");
                        break;
                    }
                    Ok(_) => {
                        // Ignore ping, pong, binary, frame
                    }
                    Err(e) => {
                        error!("WebSocket error: {}", e);
                        break;
                    }
                }
            }

            // Connection closed - fail all pending requests
            let mut pending = pending_requests_read.lock().await;
            for (_, sender) in pending.drain() {
                let _ = sender.send(Err(ClientError::ConnectionClosed));
            }
        });

        // Combine tasks into one handle
        let connection_task = tokio::spawn(async move {
            let _ = tokio::join!(write_task, read_task);
        });

        Ok(ChatClient {
            write_tx,
            pending_requests,
            subscriptions,
            _connection_task: Arc::new(connection_task),
        })
    }

    /// Connect to a chat server with exponential backoff retry.
    ///
    /// Retries connection attempts with increasing delays:
    /// - Initial delay: 1 second
    /// - Maximum delay: 10 seconds
    /// - Attempts: effectively infinite (u32::MAX)
    ///
    /// # Arguments
    /// * `url` - WebSocket URL (e.g., "ws://127.0.0.1:8080/")
    ///
    /// Returns the connected client or an error if all retries are exhausted.
    pub async fn connect_with_retry(url: &str) -> Result<Self, ClientError> {
        let attempts = u32::MAX;
        let min_delay = Duration::from_secs(1);
        let max_delay = Duration::from_secs(10);

        let mut attempt = 0;
        for duration in Backoff::new(attempts, min_delay, max_delay) {
            match Self::connect(url).await {
                Ok(client) => {
                    if attempt > 0 {
                        tracing::info!(
                            attempt = attempt,
                            "Successfully connected after {} retries",
                            attempt
                        );
                    }
                    return Ok(client);
                }
                Err(e) => {
                    attempt += 1;
                    match duration {
                        Some(delay) => {
                            tracing::warn!(
                                attempt = attempt,
                                delay_ms = delay.as_millis(),
                                error = %e,
                                "Connection failed, retrying"
                            );
                            tokio::time::sleep(delay).await;
                        }
                        None => {
                            tracing::error!(
                                attempt = attempt,
                                error = %e,
                                "Connection failed and retry limit exhausted"
                            );
                            return Err(e);
                        }
                    }
                }
            }
        }

        // This should never be reached with u32::MAX attempts
        Err(ClientError::ConnectionClosed)
    }

    /// Handle an incoming message from the server.
    async fn handle_incoming_message(
        text: &str,
        pending_requests: &Arc<Mutex<HashMap<String, PendingSender>>>,
        subscriptions: &Arc<Mutex<EventSubscriptions>>,
    ) {
        // Try to parse as a response first
        if let Ok(response) = serde_json::from_str::<Response>(text) {
            if response.r#type == "response" {
                tracing::debug!(
                    response_id = %response.id,
                    success = response.success,
                    "received response"
                );
                let mut pending = pending_requests.lock().await;
                if let Some(sender) = pending.remove(&response.id) {
                    tracing::debug!(
                        response_id = %response.id,
                        remaining_pending = pending.len(),
                        "matched response to pending request"
                    );
                    let _ = sender.send(Ok(response));
                } else {
                    tracing::warn!(
                        response_id = %response.id,
                        pending_count = pending.len(),
                        "no pending request found for response"
                    );
                }
                return;
            }
        }

        // Try to parse as an error response. The server serializes errors using
        // the ErrorResponse envelope (top-level error/errorCode, no data field),
        // which the Response struct cannot deserialize. Route it to the pending
        // request as a failed Response carrying data.error.{code,message}, the
        // shape send_request expects.
        if let Ok(error_response) = serde_json::from_str::<ErrorResponse>(text) {
            if error_response.r#type == "response" {
                tracing::debug!(
                    response_id = %error_response.id,
                    error_code = %error_response.error_code,
                    "received error response"
                );
                let mut pending = pending_requests.lock().await;
                if let Some(sender) = pending.remove(&error_response.id) {
                    let error = serde_json::json!({
                        "error": {
                            "code": error_response.error_code.to_string(),
                            "message": error_response.error,
                        }
                    });
                    let _ = sender.send(Ok(Response {
                        r#type: "response".to_string(),
                        id: error_response.id,
                        success: false,
                        data: error,
                    }));
                }
                return;
            }
        }

        // Try to parse as an event
        if let Ok(event) = serde_json::from_str::<Event>(text) {
            if event.r#type == "event" {
                Self::dispatch_event(&event, subscriptions).await;
                return;
            }
        }

        warn!("Unknown message format: {}", text);
    }

    /// Dispatch an event to the appropriate subscribers.
    async fn dispatch_event(event: &Event, subscriptions: &Arc<Mutex<EventSubscriptions>>) {
        let mut subs = subscriptions.lock().await;
        subs.cleanup();

        match event.event.as_str() {
            "messageAdded" => {
                if let Ok(data) = serde_json::from_value::<rhd_chat_api::MessageAddedData>(event.data.clone()) {
                    let chat_id = data.chat_id;
                    for sub in &subs.chat_subscriptions {
                        if sub.chat_id == chat_id {
                            let callback = sub.callback.clone();
                            let data_clone = data.clone();
                            tokio::spawn(async move {
                                (callback)(ChatEvent::MessageAdded(data_clone)).await;
                            });
                        }
                    }
                }
            }
            "messageUpdated" => {
                if let Ok(data) = serde_json::from_value::<rhd_chat_api::MessageUpdatedData>(event.data.clone()) {
                    let chat_id = data.chat_id;
                    for sub in &subs.chat_subscriptions {
                        if sub.chat_id == chat_id {
                            let callback = sub.callback.clone();
                            let data_clone = data.clone();
                            tokio::spawn(async move {
                                (callback)(ChatEvent::MessageUpdated(data_clone)).await;
                            });
                        }
                    }
                }
            }
            "messageDeleted" => {
                if let Ok(data) = serde_json::from_value::<rhd_chat_api::MessageDeletedData>(event.data.clone()) {
                    let chat_id = data.chat_id;
                    for sub in &subs.chat_subscriptions {
                        if sub.chat_id == chat_id {
                            let callback = sub.callback.clone();
                            let data_clone = data.clone();
                            tokio::spawn(async move {
                                (callback)(ChatEvent::MessageDeleted(data_clone)).await;
                            });
                        }
                    }
                }
            }
            "queueMessageAdded" => {
                if let Ok(data) = serde_json::from_value::<rhd_chat_api::QueueMessageAddedData>(event.data.clone()) {
                    let chat_id = data.chat_id;
                    for sub in &subs.chat_subscriptions {
                        if sub.chat_id == chat_id {
                            let callback = sub.callback.clone();
                            let data_clone = data.clone();
                            tokio::spawn(async move {
                                (callback)(ChatEvent::QueueMessageAdded(data_clone)).await;
                            });
                        }
                    }
                }
            }
            "queueMessageUpdated" => {
                if let Ok(data) = serde_json::from_value::<rhd_chat_api::QueueMessageUpdatedData>(event.data.clone()) {
                    let chat_id = data.chat_id;
                    for sub in &subs.chat_subscriptions {
                        if sub.chat_id == chat_id {
                            let callback = sub.callback.clone();
                            let data_clone = data.clone();
                            tokio::spawn(async move {
                                (callback)(ChatEvent::QueueMessageUpdated(data_clone)).await;
                            });
                        }
                    }
                }
            }
            "queueMessageDeleted" => {
                if let Ok(data) = serde_json::from_value::<rhd_chat_api::QueueMessageDeletedData>(event.data.clone()) {
                    let chat_id = data.chat_id;
                    for sub in &subs.chat_subscriptions {
                        if sub.chat_id == chat_id {
                            let callback = sub.callback.clone();
                            let data_clone = data.clone();
                            tokio::spawn(async move {
                                (callback)(ChatEvent::QueueMessageDeleted(data_clone)).await;
                            });
                        }
                    }
                }
            }
            "toolsUpdated" => {
                if let Ok(data) = serde_json::from_value::<rhd_chat_api::ToolsUpdatedData>(event.data.clone()) {
                    let chat_id = data.chat_id;
                    for sub in &subs.chat_subscriptions {
                        if sub.chat_id == chat_id {
                            let callback = sub.callback.clone();
                            let data_clone = data.clone();
                            tokio::spawn(async move {
                                (callback)(ChatEvent::ToolsUpdated(data_clone)).await;
                            });
                        }
                    }
                }
            }
            "streamChunk" => {
                if let Ok(data) = serde_json::from_value::<rhd_chat_api::StreamChunkData>(event.data.clone()) {
                    let chat_id = data.chat_id;
                    for sub in &subs.chat_subscriptions {
                        if sub.chat_id == chat_id {
                            let callback = sub.callback.clone();
                            let data_clone = data.clone();
                            tokio::spawn(async move {
                                (callback)(ChatEvent::StreamChunk(data_clone)).await;
                            });
                        }
                    }
                }
            }
            "streamFinished" => {
                if let Ok(data) = serde_json::from_value::<rhd_chat_api::StreamFinishedData>(event.data.clone()) {
                    let chat_id = data.chat_id;
                    for sub in &subs.chat_subscriptions {
                        if sub.chat_id == chat_id {
                            let callback = sub.callback.clone();
                            let data_clone = data.clone();
                            tokio::spawn(async move {
                                (callback)(ChatEvent::StreamFinished(data_clone)).await;
                            });
                        }
                    }
                }
            }
            "chatCreated" => {
                if let Ok(data) = serde_json::from_value::<rhd_chat_api::ChatCreatedData>(event.data.clone()) {
                    tracing::debug!(
                        event = "chatCreated",
                        chat_id = data.chat.id,
                        subscription_count = subs.chats_list_subscriptions.len(),
                        "dispatching event"
                    );
                    for sub in &subs.chats_list_subscriptions {
                        let callback = sub.callback.clone();
                        let data_clone = data.clone();
                        tokio::spawn(async move {
                            (callback)(ChatsListEvent::ChatCreated(data_clone)).await;
                        });
                    }
                }
            }
            "chatUpdated" => {
                if let Ok(data) = serde_json::from_value::<rhd_chat_api::ChatUpdatedData>(event.data.clone()) {
                    for sub in &subs.chats_list_subscriptions {
                        let callback = sub.callback.clone();
                        let data_clone = data.clone();
                        tokio::spawn(async move {
                            (callback)(ChatsListEvent::ChatUpdated(data_clone)).await;
                        });
                    }
                }
            }
            "chatDeleted" => {
                if let Ok(data) = serde_json::from_value::<rhd_chat_api::ChatDeletedData>(event.data.clone()) {
                    for sub in &subs.chats_list_subscriptions {
                        let callback = sub.callback.clone();
                        let data_clone = data.clone();
                        tokio::spawn(async move {
                            (callback)(ChatsListEvent::ChatDeleted(data_clone)).await;
                        });
                    }
                }
            }
            "pluginRegistered" => {
                if let Ok(data) = serde_json::from_value::<rhd_chat_api::PluginRegisteredData>(event.data.clone()) {
                    for sub in &subs.plugins_list_subscriptions {
                        let callback = sub.callback.clone();
                        let data_clone = data.clone();
                        tokio::spawn(async move {
                            (callback)(PluginsListEvent::PluginRegistered(data_clone)).await;
                        });
                    }
                }
            }
            "pluginUpdated" => {
                if let Ok(data) = serde_json::from_value::<rhd_chat_api::PluginUpdatedData>(event.data.clone()) {
                    for sub in &subs.plugins_list_subscriptions {
                        let callback = sub.callback.clone();
                        let data_clone = data.clone();
                        tokio::spawn(async move {
                            (callback)(PluginsListEvent::PluginUpdated(data_clone)).await;
                        });
                    }
                }
            }
            "pluginRemoved" => {
                if let Ok(data) = serde_json::from_value::<rhd_chat_api::PluginRemovedData>(event.data.clone()) {
                    for sub in &subs.plugins_list_subscriptions {
                        let callback = sub.callback.clone();
                        let data_clone = data.clone();
                        tokio::spawn(async move {
                            (callback)(PluginsListEvent::PluginRemoved(data_clone)).await;
                        });
                    }
                }
            }
            "pluginStateChanged" => {
                if let Ok(data) = serde_json::from_value::<rhd_chat_api::PluginStateChangedData>(event.data.clone()) {
                    for sub in &subs.plugin_state_subscriptions {
                        let callback = sub.callback.clone();
                        let evt = PluginStateEvent::Changed(data.clone());
                        tokio::spawn(async move {
                            (callback)(evt).await;
                        });
                    }
                }
            }
            "pluginStateRemoved" => {
                if let Ok(data) = serde_json::from_value::<rhd_chat_api::PluginStateRemovedData>(event.data.clone()) {
                    for sub in &subs.plugin_state_subscriptions {
                        let callback = sub.callback.clone();
                        let evt = PluginStateEvent::Removed(data.clone());
                        tokio::spawn(async move {
                            (callback)(evt).await;
                        });
                    }
                }
            }
            "customEvent" => {
                if let Ok(data) = serde_json::from_value::<rhd_chat_api::CustomEventData>(event.data.clone()) {
                    for sub in &subs.custom_event_subscriptions {
                        let callback = sub.callback.clone();
                        let data_clone = data.clone();
                        tokio::spawn(async move {
                            (callback)(data_clone).await;
                        });
                    }
                }
            }
            "customEventAcknowledged" => {
                if let Ok(data) = serde_json::from_value::<rhd_chat_api::CustomEventAcknowledgedData>(event.data.clone()) {
                    for sub in &subs.custom_event_acknowledged_subscriptions {
                        let callback = sub.callback.clone();
                        let data_clone = data.clone();
                        tokio::spawn(async move {
                            (callback)(data_clone).await;
                        });
                    }
                }
            }
            "assistantMessageWithToolCalls" => {
                if let Ok(data) = serde_json::from_value::<rhd_chat_api::AssistantMessageWithToolCallsData>(event.data.clone()) {
                    let chat_id = data.chat_id;
                    
                    // Dispatch to chat subscribers
                    for sub in &subs.chat_subscriptions {
                        if sub.chat_id == chat_id {
                            let callback = sub.callback.clone();
                            let data_clone = data.clone();
                            tokio::spawn(async move {
                                (callback)(ChatEvent::AssistantMessageWithToolCalls(data_clone)).await;
                            });
                        }
                    }
                    
                    // Dispatch to tool call subscribers with filtering
                    // chat_id=0 in subscription means "all chats" (wildcard)
                    for sub in &subs.tool_call_subscriptions {
                        if sub.chat_id == 0 || sub.chat_id == chat_id {
                            // Check if any of the tool names match the subscription filter
                            let matches = sub.tool_names.is_empty()
                                || sub.tool_names.iter().any(|name| data.tool_names.contains(name));
                            
                            if matches {
                                let callback = sub.callback.clone();
                                let data_clone = data.clone();
                                tokio::spawn(async move {
                                    (callback)(data_clone).await;
                                });
                            }
                        }
                    }
                }
            }
            _ => {
                debug!("Unknown event type: {}", event.event);
            }
        }
    }

    /// Send a request and wait for the response.
    async fn send_request<P, R>(&self, method: &str, params: P) -> Result<R, ClientError>
    where
        P: serde::Serialize,
        R: DeserializeOwned,
    {
        let request_id = Uuid::new_v4().to_string();
        let params_value = serde_json::to_value(params)?;
        let request = Request::new(&request_id, method, params_value);
        let request_json = serde_json::to_string(&request)?;

        tracing::debug!(
            method = method,
            request_id = %request_id,
            "sending request"
        );

        // Create oneshot channel for the response
        let (response_tx, response_rx) = oneshot::channel();

        // Register pending request
        {
            let mut pending = self.pending_requests.lock().await;
            pending.insert(request_id.clone(), response_tx);
            tracing::debug!(
                request_id = %request_id,
                pending_count = pending.len(),
                "registered pending request"
            );
        }

        // Send the request
        self.write_tx
            .send(request_json)
            .map_err(|_| ClientError::ConnectionClosed)?;

        // Wait for response
        let response = response_rx
            .await
            .map_err(|_| ClientError::ConnectionClosed)??;

        // Check for error response
        if !response.success {
            if let Some(error) = response.data.get("error") {
                let code = error
                    .get("code")
                    .and_then(|c| c.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                let message = error
                    .get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("Unknown error")
                    .to_string();
                return Err(ClientError::Server { code, message });
            }
            return Err(ClientError::Server {
                code: "unknown".to_string(),
                message: "Unknown error".to_string(),
            });
        }

        // Parse the result
        let result: R = serde_json::from_value(response.data)?;
        Ok(result)
    }

    // ========================================================================
    // Chat Methods
    // ========================================================================

    /// Create a new chat.
    pub async fn create_chat(&self, params: CreateChatParams) -> Result<CreateChatResult, ClientError> {
        self.send_request("createChat", params).await
    }

    /// List all chats.
    pub async fn list_chats(&self, params: ListChatsParams) -> Result<ListChatsResult, ClientError> {
        self.send_request("listChats", params).await
    }

    /// Get a specific chat by ID.
    pub async fn get_chat(&self, params: GetChatParams) -> Result<GetChatResult, ClientError> {
        self.send_request("getChat", params).await
    }

    /// Update a chat.
    pub async fn update_chat(&self, params: UpdateChatParams) -> Result<UpdateChatResult, ClientError> {
        self.send_request("updateChat", params).await
    }

    /// Delete a chat.
    pub async fn delete_chat(&self, params: DeleteChatParams) -> Result<DeleteChatResult, ClientError> {
        self.send_request("deleteChat", params).await
    }

    // ========================================================================
    // Message Methods
    // ========================================================================

    /// Add a message to a chat.
    pub async fn add_message(&self, params: AddMessageParams) -> Result<AddMessageResult, ClientError> {
        self.send_request("addMessage", params).await
    }

    /// Update a message.
    pub async fn update_message(&self, params: UpdateMessageParams) -> Result<UpdateMessageResult, ClientError> {
        self.send_request("updateMessage", params).await
    }

    /// Delete a message.
    pub async fn delete_message(&self, params: DeleteMessageParams) -> Result<DeleteMessageResult, ClientError> {
        self.send_request("deleteMessage", params).await
    }

    /// Get messages with optional filters (unresolved tool calls, tags).
    ///
    /// Returns filtered messages and the current chat version for consistency checking.
    pub async fn get_messages(&self, params: GetMessagesParams) -> Result<GetMessagesResult, ClientError> {
        self.send_request("getMessages", params).await
    }

    /// Add and/or remove tags on a single tool call within a message.
    ///
    /// The server parses the message's stored tool calls, applies the tag
    /// changes, saves, and broadcasts a `messageUpdated` event to subscribers.
    pub async fn update_tool_call_tags(&self, params: UpdateToolCallTagsParams) -> Result<UpdateToolCallTagsResult, ClientError> {
        self.send_request("updateToolCallTags", params).await
    }

    // ========================================================================
    // Queue Message Methods
    // ========================================================================

    /// Add a message to the queue.
    pub async fn add_queue_message(&self, params: AddQueueMessageParams) -> Result<AddQueueMessageResult, ClientError> {
        self.send_request("addQueueMessage", params).await
    }

    /// Update a queue message.
    pub async fn update_queue_message(&self, params: UpdateQueueMessageParams) -> Result<UpdateQueueMessageResult, ClientError> {
        self.send_request("updateQueueMessage", params).await
    }

    /// Delete a queue message.
    pub async fn delete_queue_message(&self, params: DeleteQueueMessageParams) -> Result<DeleteQueueMessageResult, ClientError> {
        self.send_request("deleteQueueMessage", params).await
    }

    /// Get all queue messages for a chat.
    pub async fn get_queue_messages(&self, params: GetQueueMessagesParams) -> Result<GetQueueMessagesResult, ClientError> {
        self.send_request("getQueueMessages", params).await
    }

    // ========================================================================
    // Tool Methods
    // ========================================================================

    /// Add tools to a chat.
    pub async fn add_tools(&self, params: AddToolsParams) -> Result<AddToolsResult, ClientError> {
        self.send_request("addTools", params).await
    }

    /// Remove tools from a chat.
    pub async fn remove_tools(&self, params: RemoveToolsParams) -> Result<RemoveToolsResult, ClientError> {
        self.send_request("removeTools", params).await
    }

    /// Get all tools for a chat.
    pub async fn get_tools(&self, params: GetToolsParams) -> Result<GetToolsResult, ClientError> {
        self.send_request("getTools", params).await
    }

    // ========================================================================
    // Subscription Methods
    // ========================================================================

    /// Subscribe to events for a specific chat.
    pub async fn subscribe_chat(&self, params: SubscribeChatParams) -> Result<SubscribeChatResult, ClientError> {
        self.send_request("subscribeChat", params).await
    }

    /// Unsubscribe from events for a specific chat.
    pub async fn unsubscribe_chat(&self, params: UnsubscribeChatParams) -> Result<UnsubscribeChatResult, ClientError> {
        self.send_request("unsubscribeChat", params).await
    }

    /// Subscribe to chats list events.
    pub async fn subscribe_chats_list(&self, params: SubscribeChatsListParams) -> Result<SubscribeChatsListResult, ClientError> {
        self.send_request("subscribeChatsList", params).await
    }

    /// Unsubscribe from chats list events.
    pub async fn unsubscribe_chats_list(&self, params: UnsubscribeChatsListParams) -> Result<UnsubscribeChatsListResult, ClientError> {
        self.send_request("unsubscribeChatsList", params).await
    }

    // ========================================================================
    // Plugin Methods
    // ========================================================================

    /// Register as a plugin.
    pub async fn register_plugin(&self, params: RegisterPluginParams) -> Result<RegisterPluginResult, ClientError> {
        self.send_request("registerPlugin", params).await
    }

    /// Remove a plugin registration.
    pub async fn remove_plugin(&self, params: RemovePluginParams) -> Result<RemovePluginResult, ClientError> {
        self.send_request("removePlugin", params).await
    }

    /// Get list of all plugins.
    pub async fn get_plugins(&self, params: GetPluginsParams) -> Result<GetPluginsResult, ClientError> {
        self.send_request("getPlugins", params).await
    }

    /// Subscribe to plugins list events.
    pub async fn subscribe_plugins_list(&self, params: SubscribePluginsListParams) -> Result<SubscribePluginsListResult, ClientError> {
        self.send_request("subscribePluginsList", params).await
    }

    /// Unsubscribe from plugins list events.
    pub async fn unsubscribe_plugins_list(&self, params: UnsubscribePluginsListParams) -> Result<UnsubscribePluginsListResult, ClientError> {
        self.send_request("unsubscribePluginsList", params).await
    }

    // ========================================================================
    // Plugin State Methods
    // ========================================================================

    /// Upsert a state owned by this connection's registered plugin.
    /// The result carries the stored state including the server-assigned version.
    pub async fn update_plugin_state(&self, params: UpdatePluginStateParams) -> Result<UpdatePluginStateResult, ClientError> {
        self.send_request("updatePluginState", params).await
    }

    /// Remove (tombstone) one of this plugin's states by key.
    pub async fn remove_plugin_state(&self, params: RemovePluginStateParams) -> Result<RemovePluginStateResult, ClientError> {
        self.send_request("removePluginState", params).await
    }

    /// Query live states, optionally filtered by plugin id and/or schema.
    pub async fn get_plugin_states(&self, params: GetPluginStatesParams) -> Result<GetPluginStatesResult, ClientError> {
        self.send_request("getPluginStates", params).await
    }

    /// Subscribe to plugin state events and atomically catch up on states
    /// newer than the passed versions (pass version 0 to receive the latest).
    pub async fn subscribe_plugin_states(&self, params: SubscribePluginStatesParams) -> Result<SubscribePluginStatesResult, ClientError> {
        self.send_request("subscribePluginStates", params).await
    }

    /// Unsubscribe from plugin state events.
    pub async fn unsubscribe_plugin_states(&self, params: UnsubscribePluginStatesParams) -> Result<UnsubscribePluginStatesResult, ClientError> {
        self.send_request("unsubscribePluginStates", params).await
    }

    // ========================================================================
    // Custom Event Methods
    // ========================================================================

    /// Send a custom event.
    pub async fn send_custom_event(&self, params: SendCustomEventParams) -> Result<SendCustomEventResult, ClientError> {
        self.send_request("sendCustomEvent", params).await
    }

    /// Acknowledge a custom event.
    pub async fn ack_custom_event(&self, params: AckCustomEventParams) -> Result<AckCustomEventResult, ClientError> {
        self.send_request("ackCustomEvent", params).await
    }

    /// Get pending acknowledgments for this plugin.
    pub async fn get_pending_acks(&self, params: GetPendingAcksParams) -> Result<GetPendingAcksResult, ClientError> {
        self.send_request("getPendingAcks", params).await
    }

    // ========================================================================
    // Stream Methods
    // ========================================================================

    /// Push streaming content deltas to a stream.
    pub async fn stream_push(&self, params: StreamPushParams) -> Result<StreamPushResult, ClientError> {
        self.send_request("streamPush", params).await
    }

    /// Subscribe to a stream and get current accumulated content.
    pub async fn stream_subscribe(&self, params: StreamSubscribeParams) -> Result<StreamSubscribeResult, ClientError> {
        self.send_request("streamSubscribe", params).await
    }

    /// Finish a stream.
    pub async fn stream_finish(&self, params: StreamFinishParams) -> Result<StreamFinishResult, ClientError> {
        self.send_request("streamFinish", params).await
    }

    // ========================================================================
    // Event Subscription Methods
    // ========================================================================

    /// Subscribe to events for a specific chat.
    ///
    /// The callback will be invoked for each event (messageAdded, messageUpdated, messageDeleted)
    /// on the specified chat. Returns a cancellation token that can be used to unsubscribe.
    pub fn on_chat_event<F, Fut>(&self, chat_id: i64, callback: F) -> CancellationToken
    where
        F: Fn(ChatEvent) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let (token, cancel_rx) = CancellationToken::new();
        let boxed_callback: ChatEventCallback = Arc::new(move |event| Box::pin(callback(event)));

        let subscription = ChatSubscription {
            chat_id,
            callback: boxed_callback,
            cancel_rx,
        };

        // We need to spawn a task to add the subscription since we need async lock
        let subscriptions = Arc::clone(&self.subscriptions);
        tokio::spawn(async move {
            let mut subs = subscriptions.lock().await;
            subs.chat_subscriptions.push(subscription);
        });

        token
    }

    /// Subscribe to chats list events.
    ///
    /// The callback will be invoked for each event (chatCreated, chatUpdated, chatDeleted).
    /// Returns a cancellation token that can be used to unsubscribe.
    pub async fn on_chats_list_event<F, Fut>(&self, callback: F) -> CancellationToken
    where
        F: Fn(ChatsListEvent) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let (token, cancel_rx) = CancellationToken::new();
        let boxed_callback: ChatsListEventCallback = Arc::new(move |event| Box::pin(callback(event)));

        let subscription = ChatsListSubscription {
            callback: boxed_callback,
            cancel_rx,
        };

        let subscriptions = Arc::clone(&self.subscriptions);
        let mut subs = subscriptions.lock().await;
        subs.chats_list_subscriptions.push(subscription);

        token
    }

    /// Subscribe to plugins list events.
    ///
    /// The callback will be invoked for each event (pluginRegistered, pluginUpdated, pluginRemoved).
    /// Returns a cancellation token that can be used to unsubscribe.
    pub fn on_plugins_list_event<F, Fut>(&self, callback: F) -> CancellationToken
    where
        F: Fn(PluginsListEvent) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let (token, cancel_rx) = CancellationToken::new();
        let boxed_callback: PluginsListEventCallback = Arc::new(move |event| Box::pin(callback(event)));

        let subscription = PluginsListSubscription {
            callback: boxed_callback,
            cancel_rx,
        };

        let subscriptions = Arc::clone(&self.subscriptions);
        tokio::spawn(async move {
            let mut subs = subscriptions.lock().await;
            subs.plugins_list_subscriptions.push(subscription);
        });

        token
    }

    /// Subscribe to plugin state events (pluginStateChanged, pluginStateRemoved).
    ///
    /// All state changes are broadcast to every subscriber; filter by
    /// `state.pluginId` / `state.schema` / version inside the callback.
    /// Returns a cancellation token — dropping or cancelling it unsubscribes.
    pub fn on_plugin_state_event<F, Fut>(&self, callback: F) -> CancellationToken
    where
        F: Fn(PluginStateEvent) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let (token, cancel_rx) = CancellationToken::new();
        let boxed_callback: PluginStateEventCallback =
            Arc::new(move |event| Box::pin(callback(event)));

        let subscription = PluginStateSubscription {
            callback: boxed_callback,
            cancel_rx,
        };

        let subscriptions = Arc::clone(&self.subscriptions);
        tokio::spawn(async move {
            let mut subs = subscriptions.lock().await;
            subs.plugin_state_subscriptions.push(subscription);
        });

        token
    }

    /// Subscribe to custom events.
    ///
    /// The callback will be invoked when a custom event is received.
    /// Returns a cancellation token that can be used to unsubscribe.
    pub fn on_custom_event<F, Fut>(&self, callback: F) -> CancellationToken
    where
        F: Fn(rhd_chat_api::CustomEventData) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let (token, cancel_rx) = CancellationToken::new();
        let boxed_callback: CustomEventCallback = Arc::new(move |event| Box::pin(callback(event)));

        let subscription = CustomEventSubscription {
            callback: boxed_callback,
            cancel_rx,
        };

        let subscriptions = Arc::clone(&self.subscriptions);
        tokio::spawn(async move {
            let mut subs = subscriptions.lock().await;
            subs.custom_event_subscriptions.push(subscription);
        });

        token
    }

    /// Subscribe to custom event acknowledgments.
    ///
    /// The callback will be invoked when a custom event is acknowledged.
    /// Returns a cancellation token that can be used to unsubscribe.
    pub fn on_custom_event_acknowledged<F, Fut>(&self, callback: F) -> CancellationToken
    where
        F: Fn(rhd_chat_api::CustomEventAcknowledgedData) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let (token, cancel_rx) = CancellationToken::new();
        let boxed_callback: CustomEventAcknowledgedCallback =
            Arc::new(move |event| Box::pin(callback(event)));

        let subscription = CustomEventAcknowledgedSubscription {
            callback: boxed_callback,
            cancel_rx,
        };

        let subscriptions = Arc::clone(&self.subscriptions);
        tokio::spawn(async move {
            let mut subs = subscriptions.lock().await;
            subs.custom_event_acknowledged_subscriptions.push(subscription);
        });

        token
    }

    /// Subscribe to assistant messages with tool calls.
    ///
    /// The callback will be invoked when an assistant message with tool calls is added.
    /// If `tool_names` is empty, all tool calls will trigger the callback.
    /// If `tool_names` is non-empty, only tool calls matching any of the names will trigger.
    /// Returns a cancellation token that can be used to unsubscribe.
    pub fn on_tool_call<F, Fut>(&self, chat_id: i64, tool_names: Vec<String>, callback: F) -> CancellationToken
    where
        F: Fn(rhd_chat_api::AssistantMessageWithToolCallsData) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let (token, cancel_rx) = CancellationToken::new();
        let boxed_callback: crate::event_stream::ToolCallCallback =
            Arc::new(move |event| Box::pin(callback(event)));

        let subscription = crate::event_stream::ToolCallSubscription {
            chat_id,
            tool_names,
            callback: boxed_callback,
            cancel_rx,
        };

        let subscriptions = Arc::clone(&self.subscriptions);
        tokio::spawn(async move {
            let mut subs = subscriptions.lock().await;
            subs.tool_call_subscriptions.push(subscription);
        });

        token
    }

    // ========================================================================
    // Monitor Creation Methods
    // ========================================================================

    /// Create a plugins monitor that tracks ALL registered plugins (active and inactive).
    ///
    /// The monitor automatically subscribes to plugins list events and maintains
    /// the set of all registered plugins. It also provides methods for waiting
    /// on custom event acknowledgments from all plugins.
    pub async fn create_plugins_monitor(&self) -> Result<crate::plugins_monitor::PluginsMonitor, ClientError> {
        crate::plugins_monitor::PluginsMonitor::new(self).await
    }

    /// Create a chat monitor that tracks chats and their state.
    ///
    /// The monitor automatically subscribes to chat list and individual chat events.
    /// It maintains the current state of all chats including messages, queue count, and tags.
    pub async fn create_chat_monitor(&self) -> Result<crate::chat_monitor::ChatMonitor, ClientError> {
        crate::chat_monitor::ChatMonitor::new(Arc::new(self.clone())).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn register_plugin_state_callback(
        subscriptions: &Arc<Mutex<EventSubscriptions>>,
        tx: mpsc::UnboundedSender<PluginStateEvent>,
    ) -> CancellationToken {
        let (token, cancel_rx) = CancellationToken::new();
        let callback: PluginStateEventCallback = Arc::new(move |event| {
            let tx = tx.clone();
            Box::pin(async move {
                let _ = tx.send(event);
            })
        });
        let mut subs = subscriptions.lock().await;
        subs.plugin_state_subscriptions.push(PluginStateSubscription { callback, cancel_rx });
        token
    }

    #[tokio::test]
    async fn dispatch_plugin_state_changed_reaches_callback() {
        let subscriptions = Arc::new(Mutex::new(EventSubscriptions::new()));
        let (tx, mut rx) = mpsc::unbounded_channel();
        let token = register_plugin_state_callback(&subscriptions, tx).await;

        let event = Event::new(
            "pluginStateChanged",
            serde_json::json!({ "state": {
                "pluginId": "p1", "key": "status", "content": "{}",
                "format": "json", "schema": "mcpStatus:1", "version": 1,
                "updatedAt": "2026-09-05 22:41:07"
            }}),
        );
        ChatClient::dispatch_event(&event, &subscriptions).await;

        let received = tokio::time::timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("timed out waiting for pluginStateChanged dispatch");
        assert!(matches!(received, Some(PluginStateEvent::Changed(d)) if d.state.version == 1));
        token.cancel();
    }

    #[tokio::test]
    async fn dispatch_plugin_state_removed_reaches_callback() {
        let subscriptions = Arc::new(Mutex::new(EventSubscriptions::new()));
        let (tx, mut rx) = mpsc::unbounded_channel();
        let token = register_plugin_state_callback(&subscriptions, tx).await;

        let event = Event::new(
            "pluginStateRemoved",
            serde_json::json!({ "pluginId": "p1", "key": "status", "version": 4 }),
        );
        ChatClient::dispatch_event(&event, &subscriptions).await;

        let received = tokio::time::timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("timed out waiting for pluginStateRemoved dispatch");
        assert!(matches!(received, Some(PluginStateEvent::Removed(d)) if d.version == 4));
        token.cancel();
    }
}
