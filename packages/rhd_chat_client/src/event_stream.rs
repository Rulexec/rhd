//! Event subscription handling for the chat client.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use tokio::sync::oneshot;

use rhd_chat_api::{
    AssistantMessageWithToolCallsData, ChatCreatedData, ChatDeletedData, ChatUpdatedData,
    CustomEventAcknowledgedData, CustomEventData, MessageAddedData, MessageDeletedData,
    MessageUpdatedData, PluginRegisteredData, PluginRemovedData, PluginStateChangedData,
    PluginStateRemovedData, PluginUpdatedData, QueueMessageAddedData, QueueMessageDeletedData,
    QueueMessageUpdatedData, StreamChunkData, StreamFinishedData, ToolsUpdatedData,
};

/// Events that can occur on a subscribed chat.
#[derive(Debug, Clone)]
pub enum ChatEvent {
    /// A message was added to the chat.
    MessageAdded(MessageAddedData),
    /// A message was updated in the chat.
    MessageUpdated(MessageUpdatedData),
    /// A message was deleted from the chat.
    MessageDeleted(MessageDeletedData),
    /// A queue message was added to the chat.
    QueueMessageAdded(QueueMessageAddedData),
    /// A queue message was updated in the chat.
    QueueMessageUpdated(QueueMessageUpdatedData),
    /// A queue message was deleted from the chat.
    QueueMessageDeleted(QueueMessageDeletedData),
    /// Tools were updated in the chat.
    ToolsUpdated(ToolsUpdatedData),
    /// A streaming chunk was received.
    StreamChunk(StreamChunkData),
    /// A stream finished.
    StreamFinished(StreamFinishedData),
    /// An assistant message with tool calls was added.
    AssistantMessageWithToolCalls(AssistantMessageWithToolCallsData),
}

/// Events that can occur on the chats list.
#[derive(Debug, Clone)]
pub enum ChatsListEvent {
    /// A new chat was created.
    ChatCreated(ChatCreatedData),
    /// A chat was updated.
    ChatUpdated(ChatUpdatedData),
    /// A chat was deleted.
    ChatDeleted(ChatDeletedData),
}

/// Events that can occur on the plugins list.
#[derive(Debug, Clone)]
pub enum PluginsListEvent {
    /// A new plugin was registered.
    PluginRegistered(PluginRegisteredData),
    /// A plugin was updated.
    PluginUpdated(PluginUpdatedData),
    /// A plugin was removed.
    PluginRemoved(PluginRemovedData),
}

/// Events on plugin states (broadcast to all state subscribers; consumers
/// filter by pluginId/schema/version client-side).
#[derive(Debug, Clone)]
pub enum PluginStateEvent {
    /// A state was created or updated (full stored state incl. new version).
    Changed(PluginStateChangedData),
    /// A state was removed (tombstoned); version is the bumped version.
    Removed(PluginStateRemovedData),
}

/// Type alias for async event callbacks.
pub type ChatEventCallback =
    Arc<dyn Fn(ChatEvent) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

/// Type alias for async chats list event callbacks.
pub type ChatsListEventCallback =
    Arc<dyn Fn(ChatsListEvent) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

/// Type alias for async plugins list event callbacks.
pub type PluginsListEventCallback =
    Arc<dyn Fn(PluginsListEvent) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

/// Type alias for async plugin state event callbacks.
pub type PluginStateEventCallback =
    Arc<dyn Fn(PluginStateEvent) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

/// Type alias for async custom event callbacks.
pub type CustomEventCallback =
    Arc<dyn Fn(CustomEventData) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

/// Type alias for async custom event acknowledged callbacks.
pub type CustomEventAcknowledgedCallback =
    Arc<dyn Fn(CustomEventAcknowledgedData) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

/// Type alias for async tool call callbacks.
pub type ToolCallCallback = Arc<
    dyn Fn(AssistantMessageWithToolCallsData) -> Pin<Box<dyn Future<Output = ()> + Send>>
        + Send
        + Sync,
>;

/// A subscription to chat events for a specific chat.
pub(crate) struct ChatSubscription {
    /// The chat ID this subscription is for.
    pub chat_id: i64,
    /// The callback to invoke when an event occurs.
    pub callback: ChatEventCallback,
    /// Channel to signal cancellation.
    pub cancel_rx: oneshot::Receiver<()>,
}

/// A subscription to chats list events.
pub(crate) struct ChatsListSubscription {
    /// The callback to invoke when an event occurs.
    pub callback: ChatsListEventCallback,
    /// Channel to signal cancellation.
    pub cancel_rx: oneshot::Receiver<()>,
}

/// A subscription to plugins list events.
pub(crate) struct PluginsListSubscription {
    /// The callback to invoke when an event occurs.
    pub callback: PluginsListEventCallback,
    /// Channel to signal cancellation.
    pub cancel_rx: oneshot::Receiver<()>,
}

/// A subscription to plugin state events.
pub(crate) struct PluginStateSubscription {
    /// The callback to invoke when an event occurs.
    pub callback: PluginStateEventCallback,
    /// Channel to signal cancellation.
    pub cancel_rx: oneshot::Receiver<()>,
}

/// A subscription to custom events.
pub(crate) struct CustomEventSubscription {
    /// The callback to invoke when a custom event is received.
    pub callback: CustomEventCallback,
    /// Channel to signal cancellation.
    pub cancel_rx: oneshot::Receiver<()>,
}

/// A subscription to custom event acknowledgments.
pub(crate) struct CustomEventAcknowledgedSubscription {
    /// The callback to invoke when a custom event is acknowledged.
    pub callback: CustomEventAcknowledgedCallback,
    /// Channel to signal cancellation.
    pub cancel_rx: oneshot::Receiver<()>,
}

/// A subscription to tool call events with optional filtering.
pub(crate) struct ToolCallSubscription {
    /// The chat ID this subscription is for.
    pub chat_id: i64,
    /// Tool names to filter by. Empty means all tools.
    pub tool_names: Vec<String>,
    /// The callback to invoke when a matching tool call event occurs.
    pub callback: ToolCallCallback,
    /// Channel to signal cancellation.
    pub cancel_rx: oneshot::Receiver<()>,
}

/// A token that can be used to cancel an event subscription.
pub struct CancellationToken {
    cancel_tx: Option<oneshot::Sender<()>>,
}

impl CancellationToken {
    /// Create a new cancellation token pair.
    pub(crate) fn new() -> (Self, oneshot::Receiver<()>) {
        let (tx, rx) = oneshot::channel();
        (CancellationToken { cancel_tx: Some(tx) }, rx)
    }

    /// Cancel the subscription.
    pub fn cancel(mut self) {
        if let Some(tx) = self.cancel_tx.take() {
            let _ = tx.send(());
        }
    }
}

impl Drop for CancellationToken {
    fn drop(&mut self) {
        // If the token is dropped without being cancelled, cancel automatically
        if let Some(tx) = self.cancel_tx.take() {
            let _ = tx.send(());
        }
    }
}

/// Container for all event subscriptions.
pub(crate) struct EventSubscriptions {
    pub chat_subscriptions: Vec<ChatSubscription>,
    pub chats_list_subscriptions: Vec<ChatsListSubscription>,
    pub plugins_list_subscriptions: Vec<PluginsListSubscription>,
    pub plugin_state_subscriptions: Vec<PluginStateSubscription>,
    pub custom_event_subscriptions: Vec<CustomEventSubscription>,
    pub custom_event_acknowledged_subscriptions: Vec<CustomEventAcknowledgedSubscription>,
    pub tool_call_subscriptions: Vec<ToolCallSubscription>,
}

impl EventSubscriptions {
    /// Create a new empty subscriptions container.
    pub fn new() -> Self {
        EventSubscriptions {
            chat_subscriptions: Vec::new(),
            chats_list_subscriptions: Vec::new(),
            plugins_list_subscriptions: Vec::new(),
            plugin_state_subscriptions: Vec::new(),
            custom_event_subscriptions: Vec::new(),
            custom_event_acknowledged_subscriptions: Vec::new(),
            tool_call_subscriptions: Vec::new(),
        }
    }

    /// Clean up cancelled subscriptions.
    pub fn cleanup(&mut self) {
        self.chat_subscriptions
            .retain(|s| !s.cancel_rx.is_terminated());
        self.chats_list_subscriptions
            .retain(|s| !s.cancel_rx.is_terminated());
        self.plugins_list_subscriptions
            .retain(|s| !s.cancel_rx.is_terminated());
        self.plugin_state_subscriptions
            .retain(|s| !s.cancel_rx.is_terminated());
        self.custom_event_subscriptions
            .retain(|s| !s.cancel_rx.is_terminated());
        self.custom_event_acknowledged_subscriptions
            .retain(|s| !s.cancel_rx.is_terminated());
        self.tool_call_subscriptions
            .retain(|s| !s.cancel_rx.is_terminated());
    }
}

impl Default for EventSubscriptions {
    fn default() -> Self {
        Self::new()
    }
}
