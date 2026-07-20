use super::*;
use std::fs;

#[test]
fn test_load_project_with_mcp_and_system_prompt() {
    let dir = tempfile::tempdir().unwrap();
    let project_dir = dir.path().join("test-project");
    fs::create_dir_all(&project_dir).unwrap();

    fs::write(
        project_dir.join("mcp.yaml"),
        "mcp:\n  - name: test-server\n    args:\n      - --port\n      - '8080'\n",
    )
    .unwrap();
    fs::write(project_dir.join("systemPrompt.md"), "You are a helpful assistant.").unwrap();

    let project = load_project(&project_dir).unwrap();
    assert_eq!(project.name, "test-project");
    assert_eq!(project.mcp_configs.len(), 1);
    assert_eq!(project.mcp_configs[0].name, "test-server");
    assert_eq!(
        project.mcp_configs[0].args.as_ref().unwrap(),
        &vec!["--port".to_string(), "8080".to_string()]
    );
    assert_eq!(
        project.system_prompt.as_deref(),
        Some("You are a helpful assistant.")
    );
}

#[test]
fn test_load_project_with_only_mcp() {
    let dir = tempfile::tempdir().unwrap();
    let project_dir = dir.path().join("mcp-only");
    fs::create_dir_all(&project_dir).unwrap();

    fs::write(project_dir.join("mcp.yaml"), "mcp:\n  - name: server-a\n").unwrap();

    let project = load_project(&project_dir).unwrap();
    assert_eq!(project.name, "mcp-only");
    assert_eq!(project.mcp_configs.len(), 1);
    assert!(project.system_prompt.is_none());
}

#[test]
fn test_load_project_with_only_system_prompt() {
    let dir = tempfile::tempdir().unwrap();
    let project_dir = dir.path().join("prompt-only");
    fs::create_dir_all(&project_dir).unwrap();

    fs::write(project_dir.join("systemPrompt.md"), "Custom prompt").unwrap();

    let project = load_project(&project_dir).unwrap();
    assert_eq!(project.name, "prompt-only");
    assert!(project.mcp_configs.is_empty());
    assert_eq!(project.system_prompt.as_deref(), Some("Custom prompt"));
}

#[test]
fn test_load_project_empty_mcp_yaml() {
    let dir = tempfile::tempdir().unwrap();
    let project_dir = dir.path().join("empty-mcp");
    fs::create_dir_all(&project_dir).unwrap();

    fs::write(project_dir.join("mcp.yaml"), "mcp: []\n").unwrap();

    let project = load_project(&project_dir).unwrap();
    assert!(project.mcp_configs.is_empty());
}

#[test]
fn test_load_projects_skips_empty_dirs() {
    let dir = tempfile::tempdir().unwrap();
    let projects_dir = dir.path();

    fs::create_dir_all(projects_dir.join("valid-project")).unwrap();
    fs::write(
        projects_dir.join("valid-project/mcp.yaml"),
        "mcp:\n  - name: srv\n",
    )
    .unwrap();

    fs::create_dir_all(projects_dir.join("empty-dir")).unwrap();

    let projects = load_projects(projects_dir).unwrap();
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].name, "valid-project");
}

#[test]
fn test_load_projects_nonexistent_dir() {
    let projects = load_projects(Path::new("/nonexistent/path")).unwrap();
    assert!(projects.is_empty());
}

#[test]
fn test_load_projects_invalid_yaml() {
    let dir = tempfile::tempdir().unwrap();
    let project_dir = dir.path().join("bad-yaml");
    fs::create_dir_all(&project_dir).unwrap();

    fs::write(project_dir.join("mcp.yaml"), "invalid: [yaml: content").unwrap();

    let result = load_project(&project_dir);
    assert!(result.is_err());
}

#[test]
fn test_load_projects_sorted_by_name() {
    let dir = tempfile::tempdir().unwrap();
    let projects_dir = dir.path();

    for name in &["charlie", "alpha", "bravo"] {
        let project_dir = projects_dir.join(name);
        fs::create_dir_all(&project_dir).unwrap();
        fs::write(project_dir.join("mcp.yaml"), "mcp: []\n").unwrap();
    }

    let projects = load_projects(projects_dir).unwrap();
    assert_eq!(projects.len(), 3);
    assert_eq!(projects[0].name, "alpha");
    assert_eq!(projects[1].name, "bravo");
    assert_eq!(projects[2].name, "charlie");
}

#[test]
fn test_load_project_with_env_vars() {
    let dir = tempfile::tempdir().unwrap();
    let project_dir = dir.path().join("env-project");
    fs::create_dir_all(&project_dir).unwrap();

    std::env::set_var("TEST_PROJECT_PORT", "9090");
    fs::write(
        project_dir.join("mcp.yaml"),
        "mcp:\n  - name: test\n    args:\n      - --port\n      - $TEST_PROJECT_PORT\n",
    )
    .unwrap();

    let project = load_project(&project_dir).unwrap();
    assert_eq!(
        project.mcp_configs[0].args.as_ref().unwrap(),
        &vec!["--port".to_string(), "9090".to_string()]
    );

    std::env::remove_var("TEST_PROJECT_PORT");
}

#[test]
fn test_load_project_with_roles() {
    let dir = tempfile::tempdir().unwrap();
    let project_dir = dir.path().join("test-project");
    fs::create_dir_all(&project_dir).unwrap();

    let roles_dir = project_dir.join("roles");
    fs::create_dir_all(&roles_dir).unwrap();

    let role1_dir = roles_dir.join("developer");
    fs::create_dir_all(&role1_dir).unwrap();
    fs::write(role1_dir.join("systemPrompt.md"), "You are a developer.").unwrap();
    fs::write(role1_dir.join("whenToUse.md"), "Use when coding tasks.").unwrap();

    let role2_dir = roles_dir.join("reviewer");
    fs::create_dir_all(&role2_dir).unwrap();
    fs::write(role2_dir.join("systemPrompt.md"), "You are a code reviewer.").unwrap();
    fs::write(role2_dir.join("whenToUse.md"), "Use when reviewing code.").unwrap();

    fs::write(project_dir.join("mcp.yaml"), "mcp: []\n").unwrap();

    let project = load_project(&project_dir).unwrap();
    assert_eq!(project.roles.len(), 2);
    assert_eq!(project.roles[0].name, "developer");
    assert_eq!(project.roles[0].system_prompt, "You are a developer.");
    assert_eq!(project.roles[0].when_to_use, "Use when coding tasks.");
    assert_eq!(project.roles[1].name, "reviewer");
}

#[test]
fn test_load_project_without_roles() {
    let dir = tempfile::tempdir().unwrap();
    let project_dir = dir.path().join("no-roles");
    fs::create_dir_all(&project_dir).unwrap();
    fs::write(project_dir.join("mcp.yaml"), "mcp: []\n").unwrap();

    let project = load_project(&project_dir).unwrap();
    assert!(project.roles.is_empty());
}

#[test]
fn test_load_project_with_incomplete_role() {
    let dir = tempfile::tempdir().unwrap();
    let project_dir = dir.path().join("incomplete-roles");
    fs::create_dir_all(&project_dir).unwrap();

    let roles_dir = project_dir.join("roles");
    fs::create_dir_all(&roles_dir).unwrap();

    let role_dir = roles_dir.join("incomplete");
    fs::create_dir_all(&role_dir).unwrap();
    fs::write(role_dir.join("systemPrompt.md"), "Incomplete role.").unwrap();

    fs::write(project_dir.join("mcp.yaml"), "mcp: []\n").unwrap();

    let project = load_project(&project_dir).unwrap();
    assert!(project.roles.is_empty());
}

#[test]
fn test_load_roles_sorted_by_name() {
    let dir = tempfile::tempdir().unwrap();
    let project_dir = dir.path().join("sorted-roles");
    fs::create_dir_all(&project_dir).unwrap();

    let roles_dir = project_dir.join("roles");
    fs::create_dir_all(&roles_dir).unwrap();

    for name in &["zebra", "alpha", "mike"] {
        let role_dir = roles_dir.join(name);
        fs::create_dir_all(&role_dir).unwrap();
        fs::write(role_dir.join("systemPrompt.md"), format!("Role: {}", name)).unwrap();
        fs::write(role_dir.join("whenToUse.md"), format!("When to use: {}", name)).unwrap();
    }

    fs::write(project_dir.join("mcp.yaml"), "mcp: []\n").unwrap();

    let project = load_project(&project_dir).unwrap();
    assert_eq!(project.roles.len(), 3);
    assert_eq!(project.roles[0].name, "alpha");
    assert_eq!(project.roles[1].name, "mike");
    assert_eq!(project.roles[2].name, "zebra");
}

#[test]
fn test_project_info_has_roles() {
    use rhd_api::project::ProjectInfo;

    let dir = tempfile::tempdir().unwrap();
    let project_dir = dir.path().join("with-roles");
    fs::create_dir_all(&project_dir).unwrap();

    let roles_dir = project_dir.join("roles");
    fs::create_dir_all(&roles_dir).unwrap();

    let role_dir = roles_dir.join("test-role");
    fs::create_dir_all(&role_dir).unwrap();
    fs::write(role_dir.join("systemPrompt.md"), "Test prompt").unwrap();
    fs::write(role_dir.join("whenToUse.md"), "Test when to use").unwrap();

    fs::write(project_dir.join("mcp.yaml"), "mcp: []\n").unwrap();

    let project = load_project(&project_dir).unwrap();
    let info = ProjectInfo::from(&project);
    assert!(info.has_roles);
    assert_eq!(info.role_names, vec!["test-role"]);
}
