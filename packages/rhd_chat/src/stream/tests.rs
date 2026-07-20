use super::{inject_todo_tool_contract, TemplateLoaderRef};
use crate::manager::ChatManager;
use crate::ProjectProvider;
use rhd_db::ChatDb;
use std::fs;
use std::sync::Arc;
use tokio::sync::broadcast;

fn cleanup(path: &str) {
    let _ = fs::remove_file(path);
    let _ = fs::remove_file(format!("{}-wal", path));
    let _ = fs::remove_file(format!("{}-shm", path));
}

struct MockProjectProvider {
    roles: std::collections::HashMap<String, Vec<rhd_api::project::Role>>,
    role_prompts: std::collections::HashMap<(String, String), String>,
}

#[async_trait::async_trait]
impl ProjectProvider for MockProjectProvider {
    async fn get_mcp_status(&self, _project_name: &str) -> Vec<(String, crate::McpStatus)> {
        vec![]
    }

    fn get_project_system_prompt(&self, _project_name: &str) -> Option<String> {
        None
    }

    fn get_project_mcp_refs(&self, _project_name: &str) -> Vec<rhd_api::project::McpRef> {
        vec![]
    }

    async fn get_mcp_clients(&self, _project_name: &str) -> Vec<(String, Arc<rhd_mcp_client::client::McpClient>)> {
        vec![]
    }

    async fn spawn_project_mcp(&self, _project_name: &str) -> Result<(), String> {
        Ok(())
    }

    fn get_project_roles(&self, project_name: &str) -> Vec<rhd_api::project::Role> {
        self.roles.get(project_name).cloned().unwrap_or_default()
    }

    fn get_role_system_prompt(&self, project_name: &str, role_name: &str) -> Option<String> {
        self.role_prompts
            .get(&(project_name.to_string(), role_name.to_string()))
            .cloned()
    }
}

#[tokio::test]
async fn test_inject_todo_tool_contract_first_message() {
    let db = Arc::new(ChatDb::new("test_contract_inject.db").unwrap());
    let chat_id = db.create_chat("Test").unwrap();

    let provider = MockProjectProvider {
        roles: std::collections::HashMap::new(),
        role_prompts: std::collections::HashMap::new(),
    };

    let provider = Arc::new(provider);
    let manager = ChatManager::new(db.clone(), provider.clone(), None, false);

    let template_loader = TemplateLoaderRef::new(|name| {
        if name == "mcp_internal/rhd_set_todo_list/contract" {
            Some("# rhd_set_todo_list Tool Contract\n\nThis is the contract.".to_string())
        } else {
            None
        }
    });

    let (event_sender, _event_receiver) = broadcast::channel(100);
    inject_todo_tool_contract(&manager, chat_id, &template_loader, &event_sender).unwrap();

    let messages = db.get_messages(chat_id).unwrap();
    let system_msg = messages.iter().find(|m| m.role == "system").unwrap();

    assert!(system_msg.content.contains("rhd_set_todo_list Tool Contract"));

    cleanup("test_contract_inject.db");
}

#[tokio::test]
async fn test_inject_todo_tool_contract_not_first_message() {
    let db = Arc::new(ChatDb::new("test_contract_not_first.db").unwrap());
    let chat_id = db.create_chat("Test").unwrap();

    db.add_message(chat_id, "user", "Hello", None, None).unwrap();

    let provider = MockProjectProvider {
        roles: std::collections::HashMap::new(),
        role_prompts: std::collections::HashMap::new(),
    };

    let provider = Arc::new(provider);
    let manager = ChatManager::new(db.clone(), provider.clone(), None, false);

    let template_loader = TemplateLoaderRef::new(|name| {
        if name == "mcp_internal/rhd_set_todo_list/contract" {
            Some("# Contract".to_string())
        } else {
            None
        }
    });

    let (event_sender, _event_receiver) = broadcast::channel(100);
    inject_todo_tool_contract(&manager, chat_id, &template_loader, &event_sender).unwrap();

    let messages = db.get_messages(chat_id).unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].role, "user");

    cleanup("test_contract_not_first.db");
}

#[tokio::test]
async fn test_inject_todo_tool_contract_already_injected() {
    let db = Arc::new(ChatDb::new("test_contract_already.db").unwrap());
    let chat_id = db.create_chat("Test").unwrap();

    db.add_message(
        chat_id,
        "system",
        "# rhd_set_todo_list Tool Contract\n\nExisting contract.",
        None,
        None,
    )
    .unwrap();

    let provider = MockProjectProvider {
        roles: std::collections::HashMap::new(),
        role_prompts: std::collections::HashMap::new(),
    };

    let provider = Arc::new(provider);
    let manager = ChatManager::new(db.clone(), provider.clone(), None, false);

    let template_loader = TemplateLoaderRef::new(|name| {
        if name == "mcp_internal/rhd_set_todo_list/contract" {
            Some("# New Contract".to_string())
        } else {
            None
        }
    });

    let (event_sender, _event_receiver) = broadcast::channel(100);
    inject_todo_tool_contract(&manager, chat_id, &template_loader, &event_sender).unwrap();

    let messages = db.get_messages(chat_id).unwrap();
    let system_msgs: Vec<_> = messages.iter().filter(|m| m.role == "system").collect();
    assert_eq!(system_msgs.len(), 1);
    assert!(system_msgs[0].content.contains("Existing contract"));

    cleanup("test_contract_already.db");
}

#[tokio::test]
async fn test_inject_todo_tool_contract_missing_template() {
    let db = Arc::new(ChatDb::new("test_contract_missing.db").unwrap());
    let chat_id = db.create_chat("Test").unwrap();

    let provider = MockProjectProvider {
        roles: std::collections::HashMap::new(),
        role_prompts: std::collections::HashMap::new(),
    };

    let provider = Arc::new(provider);
    let manager = ChatManager::new(db.clone(), provider.clone(), None, false);

    let template_loader = TemplateLoaderRef::new(|_name| None);

    let (event_sender, _event_receiver) = broadcast::channel(100);
    let result = inject_todo_tool_contract(&manager, chat_id, &template_loader, &event_sender);
    assert!(result.is_ok());

    let messages = db.get_messages(chat_id).unwrap();
    assert_eq!(messages.len(), 0);

    cleanup("test_contract_missing.db");
}
