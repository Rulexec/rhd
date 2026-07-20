use super::*;
use rhd_api::project::{McpRef, Role};
use std::collections::HashMap;
use std::fs;

fn cleanup(path: &str) {
    let _ = fs::remove_file(path);
    let _ = fs::remove_file(format!("{}-wal", path));
    let _ = fs::remove_file(format!("{}-shm", path));
}

struct MockProjectProvider {
    roles: HashMap<String, Vec<Role>>,
    role_prompts: HashMap<(String, String), String>,
}

#[async_trait::async_trait]
impl ProjectProvider for MockProjectProvider {
    async fn get_mcp_status(&self, _project_name: &str) -> Vec<(String, crate::McpStatus)> {
        vec![]
    }

    fn get_project_system_prompt(&self, _project_name: &str) -> Option<String> {
        None
    }

    fn get_project_mcp_refs(&self, _project_name: &str) -> Vec<McpRef> {
        vec![]
    }

    async fn get_mcp_clients(
        &self,
        _project_name: &str,
    ) -> Vec<(String, Arc<rhd_mcp_client::client::McpClient>)> {
        vec![]
    }

    async fn spawn_project_mcp(&self, _project_name: &str) -> Result<(), String> {
        Ok(())
    }

    fn get_project_roles(&self, project_name: &str) -> Vec<Role> {
        self.roles.get(project_name).cloned().unwrap_or_default()
    }

    fn get_role_system_prompt(&self, project_name: &str, role_name: &str) -> Option<String> {
        self.role_prompts
            .get(&(project_name.to_string(), role_name.to_string()))
            .cloned()
    }
}

#[tokio::test]
async fn test_check_role_conflicts_no_conflict() {
    let path = "test_no_conflict.db";
    cleanup(path);

    let db = Arc::new(ChatDb::new(path).unwrap());
    let chat_id = db.create_chat("Test").unwrap();

    let mut provider = MockProjectProvider {
        roles: HashMap::new(),
        role_prompts: HashMap::new(),
    };
    provider.roles.insert(
        "project-a".to_string(),
        vec![Role {
            name: "developer".to_string(),
            system_prompt: "Dev".to_string(),
            when_to_use: "Coding".to_string(),
        }],
    );

    db.attach_project(chat_id, "project-a").unwrap();

    provider.roles.insert(
        "project-b".to_string(),
        vec![Role {
            name: "reviewer".to_string(),
            system_prompt: "Review".to_string(),
            when_to_use: "Reviewing".to_string(),
        }],
    );

    let result = check_role_conflicts(&db, &Arc::new(provider), chat_id, "project-b");
    assert!(result.is_ok());

    cleanup(path);
}

#[tokio::test]
async fn test_check_role_conflicts_with_conflict() {
    let path = "test_conflict.db";
    cleanup(path);

    let db = Arc::new(ChatDb::new(path).unwrap());
    let chat_id = db.create_chat("Test").unwrap();

    let mut provider = MockProjectProvider {
        roles: HashMap::new(),
        role_prompts: HashMap::new(),
    };
    provider.roles.insert(
        "project-a".to_string(),
        vec![Role {
            name: "developer".to_string(),
            system_prompt: "Dev".to_string(),
            when_to_use: "Coding".to_string(),
        }],
    );

    db.attach_project(chat_id, "project-a").unwrap();

    provider.roles.insert(
        "project-b".to_string(),
        vec![Role {
            name: "developer".to_string(),
            system_prompt: "Dev 2".to_string(),
            when_to_use: "Coding 2".to_string(),
        }],
    );

    let result = check_role_conflicts(&db, &Arc::new(provider), chat_id, "project-b");
    assert!(matches!(result, Err(ChatError::RoleNameConflict(_))));

    cleanup(path);
}

fn create_mock_template_loader() -> crate::stream::TemplateLoaderRef {
    crate::stream::TemplateLoaderRef::new(|name: &str| {
        match name {
            "roles/roles_list_prompt" => Some("Your behavior is defined by the current active role. Current active role is \"{currentRoleName}\". You can switch your role by tool `rhd_set_role`.\n\nThese are the currently available roles:\n\n{rolesList}".to_string()),
            "roles/role_switch_prompt" => Some("Your current role is now \"{roleName}\".\n\n-----\n\n{systemPrompt}".to_string()),
            _ => None,
        }
    })
}

#[tokio::test]
async fn test_inject_roles_prompt() {
    let path = "test_inject_roles.db";
    cleanup(path);

    let db = Arc::new(ChatDb::new(path).unwrap());
    let chat_id = db.create_chat("Test").unwrap();

    let mut provider = MockProjectProvider {
        roles: HashMap::new(),
        role_prompts: HashMap::new(),
    };
    provider.roles.insert(
        "project-a".to_string(),
        vec![
            Role {
                name: "developer".to_string(),
                system_prompt: "You are a developer.".to_string(),
                when_to_use: "Use for coding tasks.".to_string(),
            },
            Role {
                name: "reviewer".to_string(),
                system_prompt: "You are a reviewer.".to_string(),
                when_to_use: "Use for code review.".to_string(),
            },
        ],
    );

    db.attach_project(chat_id, "project-a").unwrap();

    let (event_sender, _) = broadcast::channel(100);
    let provider = Arc::new(provider);
    let manager = ChatManager::new(db.clone(), provider, None, false);
    let template_loader = create_mock_template_loader();

    inject_roles_prompt(&manager, chat_id, &event_sender, &template_loader)
        .await
        .unwrap();

    let messages = db.get_messages(chat_id).unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].role, "system");
    assert!(messages[0].content.contains("developer"));
    assert!(messages[0].content.contains("reviewer"));
    assert!(messages[0].content.contains("Current active role is \"none\""));

    inject_roles_prompt(&manager, chat_id, &event_sender, &template_loader)
        .await
        .unwrap();
    let messages = db.get_messages(chat_id).unwrap();
    assert_eq!(messages.len(), 1);

    cleanup(path);
}

#[tokio::test]
async fn test_inject_role_system_prompt() {
    let path = "test_inject_role_prompt.db";
    cleanup(path);

    let db = Arc::new(ChatDb::new(path).unwrap());
    let chat_id = db.create_chat("Test").unwrap();

    let mut provider = MockProjectProvider {
        roles: HashMap::new(),
        role_prompts: HashMap::new(),
    };
    provider.role_prompts.insert(
        ("project-a".to_string(), "developer".to_string()),
        "You are a developer.".to_string(),
    );

    let (event_sender, _) = broadcast::channel(100);
    let provider = Arc::new(provider);
    let manager = ChatManager::new(db.clone(), provider, None, false);
    let template_loader = create_mock_template_loader();

    inject_role_system_prompt(
        &manager,
        chat_id,
        "project-a",
        "developer",
        &event_sender,
        &template_loader,
    )
    .unwrap();

    let messages = db.get_messages(chat_id).unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].role, "system");
    assert!(messages[0].content.contains("Your current role is now \"developer\""));
    assert!(messages[0].content.contains("You are a developer."));

    cleanup(path);
}
