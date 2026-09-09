//! Request handlers for the chat WebSocket protocol.

pub mod chat;
pub mod message;
pub mod plugin;
pub mod plugin_state;
pub mod queue_message;
pub mod stream;
pub mod subscription;
pub mod tools;

use serde_json::Value;

use rhd_chat_api::protocol::Request;
use rhd_chat_api::ErrorResponse;
use rhd_db::ChatDb;

use crate::error::ServerError;
use crate::plugins::SharedPluginRegistry;
use crate::streams::SharedStreamManager;
use crate::subscriptions::SharedSubscriptionManager;

/// Convert a DB tool call to its API representation.
///
/// The DB does not store the OpenAI-style `type` discriminator, so it is
/// always emitted as `"function"` on the wire.
pub(crate) fn convert_tool_call_to_api(tc: rhd_db::ToolCall) -> rhd_chat_api::ToolCall {
    rhd_chat_api::ToolCall {
        id: tc.id,
        call_type: "function".to_string(),
        function: rhd_chat_api::FunctionCall {
            name: tc.function.name,
            arguments: tc.function.arguments,
        },
        tags: tc.tags,
    }
}

/// Route a request to the appropriate handler.
pub async fn handle_request(
    request: Request,
    db: &ChatDb,
    connection_id: &str,
    subscription_manager: SharedSubscriptionManager,
    plugin_registry: SharedPluginRegistry,
    stream_manager: SharedStreamManager,
) -> Result<Value, ServerError> {
    let request_id = request.id.clone();
    
    match request.method.as_str() {
        // Chat methods
        "createChat" => chat::create_chat(request.params, db, &request_id, &subscription_manager).await,
        "listChats" => chat::list_chats(request.params, db, &request_id).await,
        "getChat" => chat::get_chat(request.params, db, &request_id).await,
        "deleteChat" => chat::delete_chat(request.params, db, &request_id, &subscription_manager).await,
        "updateChat" => chat::update_chat(request.params, db, &request_id, &subscription_manager).await,
        
        // Message methods
        "addMessage" => message::add_message(request.params, db, &request_id, &subscription_manager).await,
        "updateMessage" => message::update_message(request.params, db, &request_id, &subscription_manager).await,
        "deleteMessage" => message::delete_message(request.params, db, &request_id, &subscription_manager).await,
        "updateToolCallTags" => message::update_tool_call_tags(request.params, db, &request_id, &subscription_manager).await,
        "getMessages" => message::get_messages(request.params, db, &request_id).await,
        
        // Subscription methods
        "subscribeChat" => subscription::subscribe_chat(request.params, db, &request_id, connection_id, subscription_manager).await,
        "unsubscribeChat" => subscription::unsubscribe_chat(request.params, db, &request_id, connection_id, subscription_manager).await,
        "subscribeChatsList" => subscription::subscribe_chats_list(request.params, db, &request_id, connection_id, subscription_manager).await,
        "unsubscribeChatsList" => subscription::unsubscribe_chats_list(request.params, db, &request_id, connection_id, subscription_manager).await,
        
        // Plugin methods
        "registerPlugin" => plugin::register_plugin(request.params, db, &request_id, connection_id, plugin_registry, subscription_manager).await,
        "getPlugins" => plugin::get_plugins(request.params, db, &request_id).await,
        "subscribePluginsList" => plugin::subscribe_plugins_list(request.params, db, &request_id, connection_id, subscription_manager).await,
        "unsubscribePluginsList" => plugin::unsubscribe_plugins_list(request.params, db, &request_id, connection_id, subscription_manager).await,
        "removePlugin" => plugin::remove_plugin(request.params, db, &request_id, plugin_registry, subscription_manager).await,
        "sendCustomEvent" => plugin::send_custom_event(request.params, db, &request_id, connection_id, plugin_registry, subscription_manager).await,
        "ackCustomEvent" => plugin::ack_custom_event(request.params, db, &request_id, connection_id, plugin_registry, subscription_manager).await,
        "getPendingAcks" => plugin::get_pending_acks(request.params, db, &request_id, connection_id, plugin_registry).await,
        
        // Plugin state methods
        "updatePluginState" => {
            let plugin_id = {
                let registry = plugin_registry.read().await;
                let plugins = registry.get_plugins_for_connection(connection_id);
                plugins.into_iter().next()
            };
            plugin_state::update_plugin_state(request.params, db, &request_id, plugin_id.as_deref(), &subscription_manager).await
        }
        "removePluginState" => {
            let plugin_id = {
                let registry = plugin_registry.read().await;
                let plugins = registry.get_plugins_for_connection(connection_id);
                plugins.into_iter().next()
            };
            plugin_state::remove_plugin_state(request.params, db, &request_id, plugin_id.as_deref(), &subscription_manager).await
        }
        "getPluginStates" => plugin_state::get_plugin_states(request.params, db, &request_id).await,
        "subscribePluginStates" => plugin_state::subscribe_plugin_states(request.params, db, &request_id, connection_id, subscription_manager).await,
        "unsubscribePluginStates" => plugin_state::unsubscribe_plugin_states(request.params, db, &request_id, connection_id, subscription_manager).await,
        
        // Queue message methods
        "addQueueMessage" => queue_message::add_queue_message(request.params, db, &request_id, &subscription_manager).await,
        "updateQueueMessage" => queue_message::update_queue_message(request.params, db, &request_id, &subscription_manager).await,
        "deleteQueueMessage" => queue_message::delete_queue_message(request.params, db, &request_id, &subscription_manager).await,
        "getQueueMessages" => queue_message::get_queue_messages(request.params, db, &request_id).await,
        
        // Tool methods
        "addTools" => {
            let plugin_id = {
                let registry = plugin_registry.read().await;
                let plugins = registry.get_plugins_for_connection(connection_id);
                plugins.into_iter().next()
            };
            tools::add_tools(request.params, db, &request_id, plugin_id.as_deref(), &subscription_manager).await
        }
        "removeTools" => {
            let plugin_id = {
                let registry = plugin_registry.read().await;
                let plugins = registry.get_plugins_for_connection(connection_id);
                plugins.into_iter().next()
            };
            tools::remove_tools(request.params, db, &request_id, plugin_id.as_deref(), &subscription_manager).await
        }
        "getTools" => tools::get_tools(request.params, db, &request_id).await,
        
        // Stream methods
        "streamPush" => {
            let chat_id = request.params.get("chatId")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            stream::stream_push(request.params, &request_id, &stream_manager, &subscription_manager, chat_id).await
        }
        "streamSubscribe" => stream::stream_subscribe(request.params, &request_id, connection_id, &stream_manager, &subscription_manager).await,
        "streamFinish" => {
            let chat_id = request.params.get("chatId")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            stream::stream_finish(request.params, &request_id, &stream_manager, &subscription_manager, chat_id).await
        }
        
        // Unknown method
        _ => Ok(serde_json::to_value(ErrorResponse::invalid_request(
            request_id,
            format!("Unknown method: {}", request.method),
        ))?),
    }
}
