use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TemplateLoadError {
    #[error("failed to read template file '{path}': {source}")]
    Io {
        path: String,
        source: std::io::Error,
    },
}

pub struct TemplateLoader {
    templates: HashMap<String, String>,
}

impl TemplateLoader {
    pub fn new(templates_dir: &Path) -> Result<Self, TemplateLoadError> {
        let mut templates = HashMap::new();

        if !templates_dir.exists() {
            return Ok(Self { templates });
        }

        let entries = templates_dir.read_dir().map_err(|source| TemplateLoadError::Io {
            path: templates_dir.display().to_string(),
            source,
        })?;

        for entry in entries {
            let entry = entry.map_err(|source| TemplateLoadError::Io {
                path: templates_dir.display().to_string(),
                source,
            })?;
            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            if let Some(ext) = path.extension() {
                if ext == "md" {
                    let template_name = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_string();

                    let content = std::fs::read_to_string(&path).map_err(|source| {
                        TemplateLoadError::Io {
                            path: path.display().to_string(),
                            source,
                        }
                    })?;

                    templates.insert(template_name, content);
                }
            }
        }

        Ok(Self { templates })
    }

    pub fn get_template(&self, name: &str) -> Option<&String> {
        self.templates.get(name)
    }

    pub fn render_template(
        &self,
        name: &str,
        replacements: &HashMap<String, String>,
    ) -> Option<String> {
        self.templates.get(name).map(|template| {
            let mut result = template.clone();
            for (key, value) in replacements {
                result = result.replace(&format!("{{{}}}", key), value);
            }
            result
        })
    }
}

pub fn render_todo_items(items: &[rhd_chat::TodoItem]) -> String {
    let mut output = String::new();

    for (idx, item) in items.iter().enumerate() {
        let status_str = match item.status {
            rhd_chat::TodoStatus::Pending => "Pending",
            rhd_chat::TodoStatus::InProgress => "In Progress",
            rhd_chat::TodoStatus::Completed => "Completed",
            rhd_chat::TodoStatus::Discarded => "Discarded",
        };

        output.push_str(&format!("| {} | {} | {} |\n", idx + 1, item.content, status_str));
    }

    output
}

pub fn render_environment_details(
    template_loader: &TemplateLoader,
    todo_items: &[rhd_chat::TodoItem],
    active_role: Option<&(String, String)>,
) -> String {
    let todo_items_str = if todo_items.is_empty() {
        template_loader
            .get_template("todo_list_empty")
            .map(|s| s.as_str())
            .unwrap_or("No todo list created yet.")
            .to_string()
    } else {
        let rendered_items = render_todo_items(todo_items);
        let mut replacements = HashMap::new();
        replacements.insert("todoItems".to_string(), rendered_items);

        template_loader
            .render_template("todo_list_with_items", &replacements)
            .unwrap_or_else(|| "Failed to render todo list.".to_string())
    };

    let template_name = if active_role.is_some() {
        "environment_details_with_role"
    } else {
        "environment_details_no_role"
    };

    let mut replacements = HashMap::new();
    replacements.insert("todoItems".to_string(), todo_items_str);

    if let Some((project_name, role_name)) = active_role {
        replacements.insert(
            "currentRoleName".to_string(),
            format!("{} ({})", role_name, project_name),
        );
    }

    template_loader
        .render_template(template_name, &replacements)
        .unwrap_or_else(|| "Failed to render environment details.".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_load_templates_from_directory() {
        let dir = tempdir().unwrap();
        let templates_dir = dir.path();

        fs::write(templates_dir.join("test_template.md"), "Hello {name}!").unwrap();

        let loader = TemplateLoader::new(templates_dir).unwrap();
        assert!(loader.get_template("test_template").is_some());
        assert_eq!(loader.get_template("test_template").unwrap(), "Hello {name}!");
    }

    #[test]
    fn test_render_template_with_replacements() {
        let dir = tempdir().unwrap();
        let templates_dir = dir.path();

        fs::write(
            templates_dir.join("greeting.md"),
            "Hello {name}, welcome to {place}!",
        )
        .unwrap();

        let loader = TemplateLoader::new(templates_dir).unwrap();

        let mut replacements = HashMap::new();
        replacements.insert("name".to_string(), "Alice".to_string());
        replacements.insert("place".to_string(), "Wonderland".to_string());

        let rendered = loader.render_template("greeting", &replacements).unwrap();
        assert_eq!(rendered, "Hello Alice, welcome to Wonderland!");
    }

    #[test]
    fn test_render_template_missing_placeholder() {
        let dir = tempdir().unwrap();
        let templates_dir = dir.path();

        fs::write(templates_dir.join("partial.md"), "Hello {name}!").unwrap();

        let loader = TemplateLoader::new(templates_dir).unwrap();

        let mut replacements = HashMap::new();
        replacements.insert("name".to_string(), "Bob".to_string());

        let rendered = loader.render_template("partial", &replacements).unwrap();
        assert_eq!(rendered, "Hello Bob!");
    }

    #[test]
    fn test_load_templates_nonexistent_directory() {
        let loader = TemplateLoader::new(Path::new("/nonexistent/path")).unwrap();
        assert!(loader.get_template("any_template").is_none());
    }

    #[test]
    fn test_load_templates_skips_non_md_files() {
        let dir = tempdir().unwrap();
        let templates_dir = dir.path();

        fs::write(templates_dir.join("valid.md"), "Valid template").unwrap();
        fs::write(templates_dir.join("invalid.txt"), "Invalid file").unwrap();
        fs::write(templates_dir.join("also_invalid.json"), "{}").unwrap();

        let loader = TemplateLoader::new(templates_dir).unwrap();
        assert!(loader.get_template("valid").is_some());
        assert!(loader.get_template("invalid").is_none());
        assert!(loader.get_template("also_invalid").is_none());
    }

    #[test]
    fn test_render_todo_items_empty() {
        let items: Vec<rhd_chat::TodoItem> = vec![];
        let rendered = render_todo_items(&items);
        assert_eq!(rendered, "");
    }

    #[test]
    fn test_render_todo_items_single() {
        let items = vec![rhd_chat::TodoItem {
            content: "Test task".to_string(),
            status: rhd_chat::TodoStatus::Pending,
        }];
        let rendered = render_todo_items(&items);
        assert!(rendered.contains("| 1 | Test task | Pending |"));
    }

    #[test]
    fn test_render_todo_items_multiple() {
        let items = vec![
            rhd_chat::TodoItem {
                content: "Task 1".to_string(),
                status: rhd_chat::TodoStatus::Completed,
            },
            rhd_chat::TodoItem {
                content: "Task 2".to_string(),
                status: rhd_chat::TodoStatus::InProgress,
            },
            rhd_chat::TodoItem {
                content: "Task 3".to_string(),
                status: rhd_chat::TodoStatus::Pending,
            },
        ];
        let rendered = render_todo_items(&items);
        assert!(rendered.contains("| 1 | Task 1 | Completed |"));
        assert!(rendered.contains("| 2 | Task 2 | In Progress |"));
        assert!(rendered.contains("| 3 | Task 3 | Pending |"));
    }

    #[test]
    fn test_render_todo_items_with_discarded() {
        let items = vec![
            rhd_chat::TodoItem {
                content: "Active task".to_string(),
                status: rhd_chat::TodoStatus::InProgress,
            },
            rhd_chat::TodoItem {
                content: "Discarded task".to_string(),
                status: rhd_chat::TodoStatus::Discarded,
            },
        ];
        let rendered = render_todo_items(&items);
        assert!(rendered.contains("| 1 | Active task | In Progress |"));
        assert!(rendered.contains("| 2 | Discarded task | Discarded |"));
    }
}
