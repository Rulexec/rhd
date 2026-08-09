use std::sync::Arc;

use rhd_api::{
    ActiveRoleClearedEvent, ChatMessageAddedEvent, ChatMessageDto, ChatPausedEvent,
    ChatResumedEvent, ChatStreamChunkEvent, ChatStreamErrorEvent, ChatStreamFinishedEvent,
    ChatThinkingChunkEvent, DevNotificationEvent, MessageQueuedEvent, ProjectAttachedEvent,
    ProjectDetachedEvent, RoleChangedEvent, RoleInfo, RolesUpdatedEvent, StreamAbortedEvent,
    TodoItemDto, TodoListUpdatedEvent, ToolCallCompletedEvent, ToolCallStartedEvent, WsEvent,
};
use rhd_chat::ChatEvent;

use crate::daemon::DaemonState;

pub fn chat_event_to_ws_event(
    chat_evt: ChatEvent,
    state: &Arc<DaemonState>,
) -> Option<WsEvent> {
    match chat_evt {
        ChatEvent::StreamChunk { chat_id, content } => {
            let payload = ChatStreamChunkEvent { chat_id, content };
            Some(WsEvent::new("chatStreamChunk", serde_json::to_value(&payload).ok()?))
        }
        ChatEvent::ThinkingChunk { chat_id, content } => {
            let payload = ChatThinkingChunkEvent { chat_id, content };
            Some(WsEvent::new("chatThinkingChunk", serde_json::to_value(&payload).ok()?))
        }
        ChatEvent::StreamFinished { chat_id, message_id, finish_reason } => {
            let payload = ChatStreamFinishedEvent { chat_id, message_id, finish_reason };
            Some(WsEvent::new("chatStreamFinished", serde_json::to_value(&payload).ok()?))
        }
        ChatEvent::StreamError { chat_id, error } => {
            let payload = ChatStreamErrorEvent { chat_id, error };
            Some(WsEvent::new("chatStreamError", serde_json::to_value(&payload).ok()?))
        }
        ChatEvent::MessageAdded { chat_id, message } => {
            let payload = ChatMessageAddedEvent {
                chat_id,
                message: ChatMessageDto {
                    id: message.id,
                    chat_id: message.chat_id,
                    role: message.role,
                    content: message.content,
                    created_at: message.created_at,
                    model: message.model,
                    thinking_content: message.thinking_content,
                },
            };
            Some(WsEvent::new("chatMessageAdded", serde_json::to_value(&payload).ok()?))
        }
        ChatEvent::DevNotification { title, message } => {
            let payload = DevNotificationEvent { title, message };
            Some(WsEvent::new("devNotification", serde_json::to_value(&payload).ok()?))
        }
        ChatEvent::ProjectAttached { chat_id, project_name } => {
            let payload = ProjectAttachedEvent { chat_id, project_name };
            Some(WsEvent::new("projectAttached", serde_json::to_value(&payload).ok()?))
        }
        ChatEvent::ProjectDetached { chat_id, project_name } => {
            let payload = ProjectDetachedEvent { chat_id, project_name };
            Some(WsEvent::new("projectDetached", serde_json::to_value(&payload).ok()?))
        }
        ChatEvent::ToolCallStarted {
            chat_id,
            tool_call_id,
            tool_name,
            arguments,
            mcp_id,
        } => {
            let payload = ToolCallStartedEvent {
                chat_id,
                tool_call_id,
                tool_name,
                arguments,
                mcp_id,
            };
            Some(WsEvent::new("chatToolCallStarted", serde_json::to_value(&payload).ok()?))
        }
        ChatEvent::ToolCallCompleted {
            chat_id,
            tool_call_id,
            result,
            is_error,
        } => {
            let payload = ToolCallCompletedEvent {
                chat_id,
                tool_call_id,
                result,
                is_error,
            };
            Some(WsEvent::new("chatToolCallCompleted", serde_json::to_value(&payload).ok()?))
        }
        ChatEvent::ChatPaused { chat_id } => {
            let payload = ChatPausedEvent { chat_id };
            Some(WsEvent::new("chatPaused", serde_json::to_value(&payload).ok()?))
        }
        ChatEvent::ChatResumed { chat_id } => {
            let payload = ChatResumedEvent { chat_id };
            Some(WsEvent::new("chatResumed", serde_json::to_value(&payload).ok()?))
        }
        ChatEvent::RoleChanged { chat_id, project_name, role_name } => {
            let payload = RoleChangedEvent {
                chat_id,
                project_name,
                role_name,
            };
            Some(WsEvent::new("roleChanged", serde_json::to_value(&payload).ok()?))
        }
        ChatEvent::RolesUpdated { chat_id } => {
            let roles = state.chat_manager.get_available_roles(chat_id).unwrap_or_default();
            let active_role: Option<(String, String)> = state.chat_manager.get_active_role(chat_id).ok().flatten();
            
            let role_infos: Vec<RoleInfo> = roles
                .into_iter()
                .map(|(project_name, role_name, when_to_use)| RoleInfo {
                    project_name,
                    role_name,
                    when_to_use,
                })
                .collect();
            
            let payload = RolesUpdatedEvent {
                chat_id,
                roles: role_infos,
                active_role_project: active_role.as_ref().map(|(p, _)| p.clone()),
                active_role_name: active_role.as_ref().map(|(_, r)| r.clone()),
            };
            Some(WsEvent::new("rolesUpdated", serde_json::to_value(&payload).ok()?))
        }
        ChatEvent::ActiveRoleCleared { chat_id } => {
            let payload = ActiveRoleClearedEvent { chat_id };
            Some(WsEvent::new("activeRoleCleared", serde_json::to_value(&payload).ok()?))
        }
        ChatEvent::TodoListUpdated { chat_id, items } => {
            let payload = TodoListUpdatedEvent {
                chat_id,
                items: items.into_iter().map(|item| TodoItemDto {
                    content: item.content,
                    status: item.status.as_str().to_string(),
                }).collect(),
            };
            Some(WsEvent::new("todoListUpdated", serde_json::to_value(&payload).ok()?))
        }
        ChatEvent::StreamAborted { chat_id } => {
            let payload = StreamAbortedEvent { chat_id };
            Some(WsEvent::new("streamAborted", serde_json::to_value(&payload).ok()?))
        }
        ChatEvent::MessageQueued { chat_id, content, model } => {
            let payload = MessageQueuedEvent { chat_id, content, model };
            Some(WsEvent::new("messageQueued", serde_json::to_value(&payload).ok()?))
        }
        ChatEvent::MessageRemoved { chat_id, message_id } => {
            let payload = serde_json::json!({
                "chatId": chat_id,
                "messageId": message_id
            });
            Some(WsEvent::new("chatMessageRemoved", payload))
        }
        ChatEvent::MessageReplaced { chat_id, message } => {
            let payload = serde_json::json!({
                "chatId": chat_id,
                "message": {
                    "id": message.id,
                    "chatId": message.chat_id,
                    "role": message.role,
                    "content": message.content,
                    "createdAt": message.created_at,
                    "model": message.model,
                    "thinkingContent": message.thinking_content
                }
            });
            Some(WsEvent::new("chatMessageReplaced", payload))
        }
    }
}
