use std::collections::HashSet;
use std::sync::Arc;

use rhd_api::project::{ProjectInfo, Role};
use rhd_db::ChatDb;
use tokio::sync::broadcast;

use crate::error::ChatError;
use crate::event::ChatEvent;
use crate::ProjectProvider;

pub fn check_role_conflicts<P: ProjectProvider>(
    db: &Arc<ChatDb>,
    project_provider: &Arc<P>,
    chat_id: i64,
    project_name: &str,
) -> Result<(), ChatError> {
    let attached_projects = db.get_chat_projects(chat_id)?;

    let mut existing_role_names = HashSet::new();
    for (attached_name, _) in &attached_projects {
        for role in project_provider.get_project_roles(attached_name) {
            existing_role_names.insert(role.name);
        }
    }

    let mut conflicting_names = Vec::new();
    for role in project_provider.get_project_roles(project_name) {
        if existing_role_names.contains(&role.name) {
            conflicting_names.push(role.name);
        }
    }

    if !conflicting_names.is_empty() {
        return Err(ChatError::RoleNameConflict(format!(
            "Role names already in use: {}",
            conflicting_names.join(", ")
        )));
    }

    Ok(())
}

pub async fn attach_project<P: ProjectProvider>(
    db: &Arc<ChatDb>,
    project_provider: &Arc<P>,
    chat_id: i64,
    project_name: &str,
    event_sender: broadcast::Sender<ChatEvent>,
) -> Result<(), ChatError> {
    if db.get_chat(chat_id)?.is_none() {
        return Err(ChatError::ChatNotFound);
    }

    if project_provider.get_project_system_prompt(project_name).is_none()
        && project_provider.get_project_mcp_refs(project_name).is_empty()
        && project_provider.get_project_roles(project_name).is_empty()
    {
        return Err(ChatError::ProjectNotFound(project_name.to_string()));
    }

    let attached_projects = db.get_chat_projects(chat_id)?;
    let mut existing_mcp_ids = HashSet::new();
    for (attached_name, _) in &attached_projects {
        for mcp_ref in project_provider.get_project_mcp_refs(attached_name) {
            existing_mcp_ids.insert(mcp_ref.effective_id().to_string());
        }
    }

    let mut conflicting_ids = Vec::new();
    for mcp_ref in project_provider.get_project_mcp_refs(project_name) {
        let eid = mcp_ref.effective_id();
        if existing_mcp_ids.contains(eid) {
            conflicting_ids.push(eid.to_string());
        }
    }

    if !conflicting_ids.is_empty() {
        return Err(ChatError::McpIdConflict(format!(
            "MCP ids already in use: {}",
            conflicting_ids.join(", ")
        )));
    }

    check_role_conflicts(db, project_provider, chat_id, project_name)?;

    project_provider
        .spawn_project_mcp(project_name)
        .await
        .map_err(ChatError::McpNotConnected)?;

    db.attach_project(chat_id, project_name)?;

    let roles = project_provider.get_project_roles(project_name);
    if !roles.is_empty() {
        db.reset_roles_list_injected(chat_id)?;
        let _ = event_sender.send(ChatEvent::RolesUpdated { chat_id });
    }

    let _ = event_sender.send(ChatEvent::ProjectAttached {
        chat_id,
        project_name: project_name.to_string(),
    });

    Ok(())
}

pub fn detach_project<P: ProjectProvider>(
    db: &Arc<ChatDb>,
    project_provider: &Arc<P>,
    chat_id: i64,
    project_name: &str,
    event_sender: broadcast::Sender<ChatEvent>,
) -> Result<(), ChatError> {
    let had_roles = !project_provider.get_project_roles(project_name).is_empty();

    let active_role = db.get_active_role(chat_id)?;
    let should_clear_active_role = active_role
        .as_ref()
        .map(|(proj, _)| proj == project_name)
        .unwrap_or(false);

    db.detach_project(chat_id, project_name)?;

    if should_clear_active_role {
        db.clear_active_role(chat_id)?;
        let _ = event_sender.send(ChatEvent::ActiveRoleCleared { chat_id });
    }

    if had_roles {
        db.reset_roles_list_injected(chat_id)?;
        let _ = event_sender.send(ChatEvent::RolesUpdated { chat_id });
    }

    let _ = event_sender.send(ChatEvent::ProjectDetached {
        chat_id,
        project_name: project_name.to_string(),
    });

    Ok(())
}

pub fn get_chat_projects<P: ProjectProvider>(
    db: &Arc<ChatDb>,
    project_provider: &Arc<P>,
    chat_id: i64,
) -> Result<Vec<ProjectInfo>, ChatError> {
    let projects = db.get_chat_projects(chat_id)?;
    let mut infos = Vec::new();
    for (name, _system_prompt_added) in projects {
        let has_mcp = !project_provider.get_project_mcp_refs(&name).is_empty();
        let has_system_prompt = project_provider.get_project_system_prompt(&name).is_some();
        let roles = project_provider.get_project_roles(&name);
        let has_roles = !roles.is_empty();
        let role_names: Vec<String> = roles.iter().map(|r| r.name.clone()).collect();
        infos.push(ProjectInfo {
            name,
            has_mcp,
            has_system_prompt,
            has_roles,
            role_names,
        });
    }
    Ok(infos)
}

pub async fn inject_system_prompts<P: ProjectProvider>(
    db: &Arc<ChatDb>,
    project_provider: &Arc<P>,
    chat_id: i64,
    event_sender: &broadcast::Sender<ChatEvent>,
) -> Result<(), ChatError> {
    let attached_projects = db.get_chat_projects(chat_id)?;
    for (project_name, system_prompt_added) in &attached_projects {
        let mcp_status = project_provider.get_mcp_status(project_name).await;
        for (mcp_id, status) in &mcp_status {
            if !matches!(status, crate::McpStatus::Connected) {
                return Err(ChatError::McpNotConnected(format!(
                    "{}:{}",
                    project_name, mcp_id
                )));
            }
        }

        if !system_prompt_added {
            if let Some(system_prompt) = project_provider.get_project_system_prompt(project_name) {
                let system_message_id =
                    db.add_message(chat_id, "system", &system_prompt, None, None)?;
                let system_message = rhd_db::Message {
                    id: system_message_id,
                    chat_id,
                    role: "system".to_string(),
                    content: system_prompt.clone(),
                    created_at: chrono::Utc::now().to_rfc3339(),
                    model: None,
                    thinking_content: None,
                };
                let _ = event_sender.send(ChatEvent::MessageAdded {
                    chat_id,
                    message: system_message,
                });
                db.mark_system_prompt_added(chat_id, project_name)?;
            }
        }
    }
    Ok(())
}

pub async fn inject_roles_prompt<P: ProjectProvider>(
    db: &Arc<ChatDb>,
    project_provider: &Arc<P>,
    chat_id: i64,
    event_sender: &broadcast::Sender<ChatEvent>,
) -> Result<(), ChatError> {
    if db.has_roles_list_been_injected(chat_id)? {
        return Ok(());
    }

    let attached_projects = db.get_chat_projects(chat_id)?;
    let mut all_roles: Vec<(String, Role)> = Vec::new();

    for (project_name, _) in &attached_projects {
        let roles = project_provider.get_project_roles(project_name);
        for role in roles {
            all_roles.push((project_name.clone(), role));
        }
    }

    if all_roles.is_empty() {
        return Ok(());
    }

    let active_role = db.get_active_role(chat_id)?;
    let current_role_name = active_role
        .as_ref()
        .map(|(_, name)| name.as_str())
        .unwrap_or("none");

    let mut prompt = String::new();
    prompt.push_str(&format!(
        "Your behavior is defined by the current active role. Current active role is \"{}\". You can switch your role by tool `rhd_set_role`.\n\n",
        current_role_name
    ));
    prompt.push_str("These are the currently available roles:\n");

    for (project_name, role) in &all_roles {
        prompt.push_str(&format!("\n# {} (from project: {})\n\n", role.name, project_name));
        prompt.push_str(&role.when_to_use);
        prompt.push('\n');
    }

    let system_message_id = db.add_message(chat_id, "system", &prompt, None, None)?;
    let system_message = rhd_db::Message {
        id: system_message_id,
        chat_id,
        role: "system".to_string(),
        content: prompt,
        created_at: chrono::Utc::now().to_rfc3339(),
        model: None,
        thinking_content: None,
    };
    let _ = event_sender.send(ChatEvent::MessageAdded {
        chat_id,
        message: system_message,
    });

    db.mark_roles_list_injected(chat_id)?;

    Ok(())
}

pub fn inject_role_system_prompt<P: ProjectProvider>(
    db: &Arc<ChatDb>,
    project_provider: &Arc<P>,
    chat_id: i64,
    project_name: &str,
    role_name: &str,
    event_sender: &broadcast::Sender<ChatEvent>,
) -> Result<(), ChatError> {
    let system_prompt = project_provider
        .get_role_system_prompt(project_name, role_name)
        .ok_or_else(|| {
            ChatError::RoleNotFound(format!("{}:{}", project_name, role_name))
        })?;

    let mut prompt = String::new();
    prompt.push_str(&format!("Your current role is now \"{}\".\n\n", role_name));
    prompt.push_str("-----\n\n");
    prompt.push_str(&system_prompt);

    let system_message_id = db.add_message(chat_id, "system", &prompt, None, None)?;
    let system_message = rhd_db::Message {
        id: system_message_id,
        chat_id,
        role: "system".to_string(),
        content: prompt,
        created_at: chrono::Utc::now().to_rfc3339(),
        model: None,
        thinking_content: None,
    };
    let _ = event_sender.send(ChatEvent::MessageAdded {
        chat_id,
        message: system_message,
    });

    Ok(())
}

pub fn inject_pending_role_prompt<P: ProjectProvider>(
    db: &Arc<ChatDb>,
    project_provider: &Arc<P>,
    chat_id: i64,
    event_sender: &broadcast::Sender<ChatEvent>,
) -> Result<(), ChatError> {
    if !db.has_role_prompt_pending(chat_id)? {
        return Ok(());
    }

    let active_role = db.get_active_role(chat_id)?;
    if let Some((project_name, role_name)) = active_role {
        inject_role_system_prompt(
            db,
            project_provider,
            chat_id,
            &project_name,
            &role_name,
            event_sender,
        )?;
    }

    db.set_role_prompt_pending(chat_id, false)?;

    Ok(())
}

#[cfg(test)]
mod tests {
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

        inject_roles_prompt(&db, &provider, chat_id, &event_sender)
            .await
            .unwrap();

        let messages = db.get_messages(chat_id).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "system");
        assert!(messages[0].content.contains("developer"));
        assert!(messages[0].content.contains("reviewer"));
        assert!(messages[0].content.contains("Current active role is \"none\""));

        inject_roles_prompt(&db, &provider, chat_id, &event_sender)
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

        inject_role_system_prompt(
            &db,
            &provider,
            chat_id,
            "project-a",
            "developer",
            &event_sender,
        )
        .unwrap();

        let messages = db.get_messages(chat_id).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "system");
        assert!(messages[0].content.contains("Your current role is now \"developer\""));
        assert!(messages[0].content.contains("You are a developer."));

        cleanup(path);
    }
}
