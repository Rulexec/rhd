use super::*;
use rhd_api::project::McpRef;
use std::path::PathBuf;

fn make_project(name: &str, mcp_count: usize) -> Project {
    let mcp_configs: Vec<McpRef> = (0..mcp_count)
        .map(|i| McpRef {
            name: format!("mcp-{}", i),
            id: None,
            args: None,
            env: None,
        })
        .collect();
    Project {
        name: name.to_string(),
        path: PathBuf::from(format!("/tmp/{}", name)),
        mcp_configs,
        system_prompt: Some("test prompt".to_string()),
        roles: Vec::new(),
    }
}

fn make_manager(projects: Vec<Project>) -> ProjectManager {
    ProjectManager::new(projects, HashMap::new(), Arc::new(McpServerCache::new()))
}

#[test]
fn test_list_projects_empty() {
    let manager = make_manager(vec![]);
    assert!(manager.list_projects().is_empty());
}

#[test]
fn test_list_projects_sorted() {
    let manager = make_manager(vec![
        make_project("charlie", 0),
        make_project("alpha", 0),
        make_project("bravo", 0),
    ]);
    let infos = manager.list_projects();
    assert_eq!(infos.len(), 3);
    assert_eq!(infos[0].name, "alpha");
    assert_eq!(infos[1].name, "bravo");
    assert_eq!(infos[2].name, "charlie");
}

#[test]
fn test_get_project_found() {
    let manager = make_manager(vec![make_project("test", 1)]);
    let project = manager.get_project("test");
    assert!(project.is_some());
    assert_eq!(project.unwrap().name, "test");
}

#[test]
fn test_get_project_not_found() {
    let manager = make_manager(vec![make_project("test", 1)]);
    assert!(manager.get_project("nonexistent").is_none());
}

#[tokio::test]
async fn test_get_mcp_status_empty() {
    let manager = make_manager(vec![make_project("test", 0)]);
    let status = manager.get_mcp_status("test").await;
    assert!(status.is_empty());
}

#[tokio::test]
async fn test_get_mcp_clients_empty() {
    let manager = make_manager(vec![make_project("test", 0)]);
    let clients = manager.get_mcp_clients("test").await;
    assert!(clients.is_empty());
}

#[test]
fn test_project_info_has_mcp() {
    let manager = make_manager(vec![
        make_project("with-mcp", 2),
        make_project("without-mcp", 0),
    ]);
    let infos = manager.list_projects();
    let with_mcp = infos.iter().find(|i| i.name == "with-mcp").unwrap();
    let without_mcp = infos.iter().find(|i| i.name == "without-mcp").unwrap();
    assert!(with_mcp.has_mcp);
    assert!(!without_mcp.has_mcp);
}

#[test]
fn test_project_info_has_system_prompt() {
    let mut project = make_project("test", 0);
    project.system_prompt = Some("prompt".to_string());
    let manager = make_manager(vec![project]);
    let info = &manager.list_projects()[0];
    assert!(info.has_system_prompt);
}

#[test]
fn test_mcp_status_is_connected() {
    assert!(McpStatus::Connected.is_connected());
    assert!(!McpStatus::Connecting.is_connected());
    assert!(!McpStatus::Failed("err".to_string()).is_connected());
}

#[test]
fn test_get_project_roles() {
    let mut project = make_project("test", 0);
    project.roles = vec![
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
    ];
    let manager = make_manager(vec![project]);

    let roles = manager.get_project_roles("test");
    assert_eq!(roles.len(), 2);
    assert_eq!(roles[0].name, "developer");
    assert_eq!(roles[1].name, "reviewer");
}

#[test]
fn test_get_project_roles_empty() {
    let project = make_project("test", 0);
    let manager = make_manager(vec![project]);

    let roles = manager.get_project_roles("test");
    assert!(roles.is_empty());

    let roles = manager.get_project_roles("nonexistent");
    assert!(roles.is_empty());
}

#[test]
fn test_get_role_system_prompt() {
    let mut project = make_project("test", 0);
    project.roles = vec![
        Role {
            name: "developer".to_string(),
            system_prompt: "Dev prompt".to_string(),
            when_to_use: "When coding".to_string(),
        },
    ];
    let manager = make_manager(vec![project]);

    let prompt = manager.get_role_system_prompt("test", "developer");
    assert_eq!(prompt, Some("Dev prompt".to_string()));

    let prompt = manager.get_role_system_prompt("test", "nonexistent");
    assert_eq!(prompt, None);

    let prompt = manager.get_role_system_prompt("nonexistent", "developer");
    assert_eq!(prompt, None);
}
