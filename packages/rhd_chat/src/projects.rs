use std::collections::HashSet;
use std::sync::Arc;

use rhd_api::project::{ProjectInfo, Role};
use rhd_db::ChatDb;
use tokio::sync::broadcast;

use crate::error::ChatError;
use crate::event::ChatEvent;
use crate::manager::ChatManager;
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
    manager: &ChatManager<P>,
    chat_id: i64,
    event_sender: &broadcast::Sender<ChatEvent>,
) -> Result<(), ChatError> {
    let db = manager.db();
    let project_provider = manager.project_provider();
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
                manager.add_message_and_notify(chat_id, "system", &system_prompt, None, None, event_sender)?;
                db.mark_system_prompt_added(chat_id, project_name)?;
            }
        }
    }
    Ok(())
}

pub async fn inject_roles_prompt<P: ProjectProvider>(
    manager: &ChatManager<P>,
    chat_id: i64,
    event_sender: &broadcast::Sender<ChatEvent>,
    template_loader: &crate::stream::TemplateLoaderRef,
) -> Result<(), ChatError> {
    let db = manager.db();
    let project_provider = manager.project_provider();
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

    let mut roles_list = String::new();
    for (project_name, role) in &all_roles {
        roles_list.push_str(&format!("\n# {} (from project: {})\n\n", role.name, project_name));
        roles_list.push_str(&role.when_to_use);
        roles_list.push('\n');
    }

    let template = template_loader
        .get_template("roles/roles_list_prompt")
        .ok_or_else(|| ChatError::Internal("Template 'roles/roles_list_prompt' not found".to_string()))?;

    let prompt = template
        .replace("{currentRoleName}", current_role_name)
        .replace("{rolesList}", &roles_list);

    manager.add_message_and_notify(chat_id, "system", &prompt, None, None, event_sender)?;

    db.mark_roles_list_injected(chat_id)?;

    Ok(())
}

pub fn inject_role_system_prompt<P: ProjectProvider>(
    manager: &ChatManager<P>,
    chat_id: i64,
    project_name: &str,
    role_name: &str,
    event_sender: &broadcast::Sender<ChatEvent>,
    template_loader: &crate::stream::TemplateLoaderRef,
) -> Result<(), ChatError> {
    let project_provider = manager.project_provider();
    let system_prompt = project_provider
        .get_role_system_prompt(project_name, role_name)
        .ok_or_else(|| {
            ChatError::RoleNotFound(format!("{}:{}", project_name, role_name))
        })?;

    let template = template_loader
        .get_template("roles/role_switch_prompt")
        .ok_or_else(|| ChatError::Internal("Template 'roles/role_switch_prompt' not found".to_string()))?;

    let prompt = template
        .replace("{roleName}", role_name)
        .replace("{systemPrompt}", &system_prompt);

    manager.add_message_and_notify(chat_id, "system", &prompt, None, None, event_sender)?;

    Ok(())
}

pub fn inject_pending_role_prompt<P: ProjectProvider>(
    manager: &ChatManager<P>,
    chat_id: i64,
    event_sender: &broadcast::Sender<ChatEvent>,
    template_loader: &crate::stream::TemplateLoaderRef,
) -> Result<(), ChatError> {
    let db = manager.db();
    if !db.has_role_prompt_pending(chat_id)? {
        return Ok(());
    }

    let active_role = db.get_active_role(chat_id)?;
    if let Some((project_name, role_name)) = active_role {
        inject_role_system_prompt(
            manager,
            chat_id,
            &project_name,
            &role_name,
            event_sender,
            template_loader,
        )?;
    }

    db.set_role_prompt_pending(chat_id, false)?;

    Ok(())
}

#[cfg(test)]
mod tests;
