use std::path::Path;

use thiserror::Error;

use rhd_api::project::{McpRef, Project, Role};

#[derive(Debug, Error)]
pub enum ProjectLoadError {
    #[error("failed to read project file '{path}': {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to parse project YAML at {path}:{line}:{column}: {message}")]
    Parse {
        path: String,
        line: usize,
        column: usize,
        message: String,
    },
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
struct McpYaml {
    #[serde(default)]
    mcp: Vec<McpRef>,
}

pub fn load_projects(projects_dir: &Path) -> Result<Vec<Project>, ProjectLoadError> {
    let mut projects = Vec::new();

    if !projects_dir.exists() {
        return Ok(projects);
    }

    let entries = projects_dir.read_dir().map_err(|source| ProjectLoadError::Io {
        path: projects_dir.display().to_string(),
        source,
    })?;

    for entry in entries {
        let entry = entry.map_err(|source| ProjectLoadError::Io {
            path: projects_dir.display().to_string(),
            source,
        })?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let mcp_file = path.join("mcp.yaml");
        let system_prompt_file = path.join("systemPrompt.md");

        if !mcp_file.exists() && !system_prompt_file.exists() {
            continue;
        }

        let project = load_project(&path)?;
        projects.push(project);
    }

    projects.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(projects)
}

pub fn load_project(project_dir: &Path) -> Result<Project, ProjectLoadError> {
    let dir_name = project_dir
        .file_name()
        .expect("directory must have a name")
        .to_string_lossy()
        .into_owned();

    let mcp_file = project_dir.join("mcp.yaml");
    let mcp_configs = if mcp_file.exists() {
        let path_str = mcp_file.display().to_string();
        let contents = std::fs::read_to_string(&mcp_file).map_err(|source| ProjectLoadError::Io {
            path: path_str.clone(),
            source,
        })?;

        let mut mcp_yaml: McpYaml =
            serde_yaml::from_str(&contents).map_err(|e| ProjectLoadError::Parse {
                path: path_str.clone(),
                line: e.location().map(|l| l.line()).unwrap_or(0),
                column: e.location().map(|l| l.column()).unwrap_or(0),
                message: e.to_string(),
            })?;

        for mcp_ref in &mut mcp_yaml.mcp {
            if let Some(args) = &mut mcp_ref.args {
                for arg in args.iter_mut() {
                    *arg = rhd_util::substitute_env_vars(arg);
                }
            }
            if let Some(env) = &mut mcp_ref.env {
                for (_key, value) in env.iter_mut() {
                    *value = rhd_util::substitute_env_vars(value);
                }
            }
        }

        let mut seen_ids = std::collections::HashSet::new();
        for mcp_ref in &mcp_yaml.mcp {
            let eid = mcp_ref.effective_id().to_string();
            if !seen_ids.insert(eid.clone()) {
                return Err(ProjectLoadError::Parse {
                    path: mcp_file.display().to_string(),
                    line: 0,
                    column: 0,
                    message: format!("duplicate MCP id: '{}'", eid),
                });
            }
        }

        mcp_yaml.mcp
    } else {
        Vec::new()
    };

    let system_prompt_file = project_dir.join("systemPrompt.md");
    let system_prompt = if system_prompt_file.exists() {
        let path_str = system_prompt_file.display().to_string();
        let content =
            std::fs::read_to_string(&system_prompt_file).map_err(|source| ProjectLoadError::Io {
                path: path_str,
                source,
            })?;
        Some(content)
    } else {
        None
    };

    let roles = load_roles(project_dir)?;

    Ok(Project {
        name: dir_name,
        path: project_dir.to_path_buf(),
        mcp_configs,
        system_prompt,
        roles,
    })
}

pub fn load_roles(project_dir: &Path) -> Result<Vec<Role>, ProjectLoadError> {
    let roles_dir = project_dir.join("roles");

    if !roles_dir.exists() || !roles_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut roles = Vec::new();
    let entries = roles_dir.read_dir().map_err(|source| ProjectLoadError::Io {
        path: roles_dir.display().to_string(),
        source,
    })?;

    for entry in entries {
        let entry = entry.map_err(|source| ProjectLoadError::Io {
            path: roles_dir.display().to_string(),
            source,
        })?;
        let role_dir = entry.path();

        if !role_dir.is_dir() {
            continue;
        }

        let role_name = role_dir
            .file_name()
            .expect("directory must have a name")
            .to_string_lossy()
            .into_owned();

        let system_prompt_file = role_dir.join("systemPrompt.md");
        let when_to_use_file = role_dir.join("whenToUse.md");

        if !system_prompt_file.exists() || !when_to_use_file.exists() {
            continue;
        }

        let system_prompt_path = system_prompt_file.display().to_string();
        let system_prompt = std::fs::read_to_string(&system_prompt_file)
            .map_err(|source| ProjectLoadError::Io {
                path: system_prompt_path,
                source,
            })?;

        let when_to_use_path = when_to_use_file.display().to_string();
        let when_to_use = std::fs::read_to_string(&when_to_use_file)
            .map_err(|source| ProjectLoadError::Io {
                path: when_to_use_path,
                source,
            })?;

        roles.push(Role {
            name: role_name,
            system_prompt,
            when_to_use,
        });
    }

    roles.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(roles)
}

#[cfg(test)]
mod tests {
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
}
