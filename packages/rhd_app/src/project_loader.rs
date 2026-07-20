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
mod tests;
