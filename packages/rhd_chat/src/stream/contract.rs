use tokio::sync::broadcast;

use crate::error::ChatError;
use crate::event::ChatEvent;
use crate::manager::ChatManager;
use crate::ProjectProvider;

use super::TemplateLoaderRef;

pub fn inject_todo_tool_contract<P: ProjectProvider>(
    manager: &ChatManager<P>,
    chat_id: i64,
    template_loader: &TemplateLoaderRef,
    event_sender: &broadcast::Sender<ChatEvent>,
) -> Result<(), ChatError> {
    let messages = manager.db().get_messages(chat_id)?;

    let user_message_count = messages.iter().filter(|m| m.role == "user").count();
    
    eprintln!("[DEBUG] inject_todo_tool_contract: chat_id={}, user_message_count={}", chat_id, user_message_count);
    
    if user_message_count > 0 {
        eprintln!("[DEBUG] inject_todo_tool_contract: skipping, already have user messages");
        return Ok(());
    }

    let has_contract = messages.iter().any(|m| {
        m.role == "system" && m.content.contains("rhd_set_todo_list Tool Contract")
    });
    if has_contract {
        eprintln!("[DEBUG] inject_todo_tool_contract: skipping, contract already exists");
        return Ok(());
    }

    let contract_content = match template_loader.get_template("mcp_internal/rhd_set_todo_list/contract") {
        Some(content) => content,
        None => {
            let error_msg = "Template 'mcp_internal/rhd_set_todo_list/contract' not found, cannot inject todo contract".to_string();
            eprintln!("[ERROR] {}", error_msg);
            let _ = event_sender.send(ChatEvent::DevNotification {
                title: "Todo Contract Injection Failed".to_string(),
                message: error_msg,
            });
            return Ok(());
        }
    };

    eprintln!("[DEBUG] inject_todo_tool_contract: injecting contract, content length={}", contract_content.len());
    manager.add_message_and_notify(chat_id, "system", &contract_content, None, None, event_sender)?;

    Ok(())
}
