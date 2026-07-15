use std::collections::HashMap;
use crate::templates::TemplateRegistry;

pub struct TemplateLoader;

impl TemplateLoader {
    pub fn new() -> Self {
        Self
    }

    pub fn get_template(&self, name: &str) -> Option<&'static str> {
        TemplateRegistry::get(name)
    }

    pub fn render_template(
        &self,
        name: &str,
        replacements: &HashMap<String, String>,
    ) -> Option<String> {
        TemplateRegistry::get(name).map(|template| {
            let mut result = template.to_string();
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

    #[test]
    fn test_get_existing_template() {
        let loader = TemplateLoader::new();
        assert!(loader.get_template("todo_list_empty").is_some());
        assert!(loader.get_template("todo_list_with_items").is_some());
        assert!(loader.get_template("environment_details_no_role").is_some());
        assert!(loader.get_template("environment_details_with_role").is_some());
        assert!(loader.get_template("rhd_set_todo_list_contract").is_some());
    }

    #[test]
    fn test_get_nonexistent_template() {
        let loader = TemplateLoader::new();
        assert!(loader.get_template("nonexistent_template").is_none());
    }

    #[test]
    fn test_list_templates() {
        let templates = TemplateRegistry::list_templates();
        assert_eq!(templates.len(), 5);
        assert!(templates.contains(&"todo_list_empty"));
        assert!(templates.contains(&"todo_list_with_items"));
        assert!(templates.contains(&"environment_details_no_role"));
        assert!(templates.contains(&"environment_details_with_role"));
        assert!(templates.contains(&"rhd_set_todo_list_contract"));
    }

    #[test]
    fn test_render_template_with_replacements() {
        let loader = TemplateLoader::new();
        let mut replacements = HashMap::new();
        replacements.insert("todoItems".to_string(), "| 1 | Test task | Pending |\n".to_string());

        let rendered = loader.render_template("todo_list_with_items", &replacements);
        assert!(rendered.is_some());
        let rendered_str = rendered.unwrap();
        assert!(rendered_str.contains("| 1 | Test task | Pending |"));
    }

    #[test]
    fn test_render_template_missing_placeholder() {
        let loader = TemplateLoader::new();
        let mut replacements = HashMap::new();
        replacements.insert("todoItems".to_string(), "Test content".to_string());

        let rendered = loader.render_template("todo_list_with_items", &replacements);
        assert!(rendered.is_some());
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
