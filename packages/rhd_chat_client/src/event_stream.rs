//! Event subscription handling for the chat client.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use tokio::sync::oneshot;

use rhd_chat_api::{
    ChatCreatedData, ChatDeletedData, ChatUpdatedData, CustomEventAcknowledgedData, CustomEventData,
    MessageAddedData, MessageDeletedData, MessageUpdatedData, PluginRegisteredData,
    PluginRemovedData, PluginUpdatedData,
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

/// Type alias for async event callbacks.
pub type ChatEventCallback =
    Arc<dyn Fn(ChatEvent) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

/// Type alias for async chats list event callbacks.
pub type ChatsListEventCallback =
    Arc<dyn Fn(ChatsListEvent) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

/// Type alias for async plugins list event callbacks.
pub type PluginsListEventCallback =
    Arc<dyn Fn(PluginsListEvent) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

/// Type alias for async custom event callbacks.
pub type CustomEventCallback =
    Arc<dyn Fn(CustomEventData) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

/// Type alias for async custom event acknowledged callbacks.
pub type CustomEventAcknowledgedCallback =
    Arc<dyn Fn(CustomEventAcknowledgedData) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

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
    pub custom_event_subscriptions: Vec<CustomEventSubscription>,
    pub custom_event_acknowledged_subscriptions: Vec<CustomEventAcknowledgedSubscription>,
}

impl EventSubscriptions {
    /// Create a new empty subscriptions container.
    pub fn new() -> Self {
        EventSubscriptions {
            chat_subscriptions: Vec::new(),
            chats_list_subscriptions: Vec::new(),
            plugins_list_subscriptions: Vec::new(),
            custom_event_subscriptions: Vec::new(),
            custom_event_acknowledged_subscriptions: Vec::new(),
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
        self.custom_event_subscriptions
            .retain(|s| !s.cancel_rx.is_terminated());
        self.custom_event_acknowledged_subscriptions
            .retain(|s| !s.cancel_rx.is_terminated());
    }
}

impl Default for EventSubscriptions {
    fn default() -> Self {
        Self::new()
    }
}
