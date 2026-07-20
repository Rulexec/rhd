use rhd_ai::client::{ChatMessage, ToolCall};
use rhd_db::Message;

pub fn build_chat_messages(messages: &[Message]) -> Vec<ChatMessage> {
    messages
        .iter()
        .map(|m| match m.role.as_str() {
            "user" => ChatMessage::user(&m.content),
            "assistant" => ChatMessage::assistant_with_thinking(&m.content, m.thinking_content.clone()),
            "system" => ChatMessage::system(&m.content),
            _ => ChatMessage::user(&m.content),
        })
        .collect()
}

pub fn build_chat_messages_for_tools(messages: &[Message]) -> Vec<ChatMessage> {
    let mut chat_messages = Vec::new();
    
    for m in messages {
        match m.role.as_str() {
            "user" => chat_messages.push(ChatMessage::user(&m.content)),
            "system" => chat_messages.push(ChatMessage::system(&m.content)),
            "assistant" => {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&m.content) {
                    let content = json.get("content").and_then(|c| c.as_str()).map(|s| s.to_string());
                    let tool_calls: Vec<ToolCall> = json.get("toolCalls")
                        .and_then(|tc| serde_json::from_value(tc.clone()).ok())
                        .unwrap_or_default();
                    
                    if tool_calls.is_empty() {
                        chat_messages.push(ChatMessage::assistant_with_thinking(content.unwrap_or_default(), m.thinking_content.clone()));
                    } else {
                        chat_messages.push(ChatMessage::assistant_with_tool_calls(content, m.thinking_content.clone(), tool_calls));
                    }
                } else {
                    chat_messages.push(ChatMessage::assistant_with_thinking(&m.content, m.thinking_content.clone()));
                }
            }
            "tool" => {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&m.content) {
                    let tool_call_id = json.get("toolCallId")
                        .and_then(|id| id.as_str())
                        .unwrap_or("");
                    let result = json.get("result")
                        .and_then(|r| r.as_str())
                        .unwrap_or("");
                    chat_messages.push(ChatMessage::tool(tool_call_id, result));
                }
            }
            _ => chat_messages.push(ChatMessage::user(&m.content)),
        }
    }
    
    chat_messages
}
