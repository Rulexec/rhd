use std::collections::HashSet;
use std::sync::Arc;

use rhd_api::project::ProjectInfo;
use rhd_db::ChatDb;
use tokio::sync::broadcast;

use crate::error::ChatError;
use crate::event::ChatEvent;
use crate::ProjectProvider;

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

    project_provider
        .spawn_project_mcp(project_name)
        .await
        .map_err(ChatError::McpNotConnected)?;

    db.attach_project(chat_id, project_name)?;

    let _ = event_sender.send(ChatEvent::ProjectAttached {
        chat_id,
        project_name: project_name.to_string(),
    });

    Ok(())
}

pub fn detach_project(
    db: &Arc<ChatDb>,
    chat_id: i64,
    project_name: &str,
    event_sender: broadcast::Sender<ChatEvent>,
) -> Result<(), ChatError> {
    db.detach_project(chat_id, project_name)?;

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
        infos.push(ProjectInfo {
            name,
            has_mcp,
            has_system_prompt,
            has_roles: false,
            role_names: Vec::new(),
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
