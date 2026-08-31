//! Template loading and rendering.

use include_dir::{include_dir, Dir};

use crate::parser::TodoItem;

// Embed templates directory at compile time
static TEMPLATES_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/../../templates");

/// Template names.
pub const CONTRACT_TEMPLATE: &str = "mcp_internal/rhd_set_todo_list/contract.md";
pub const TOOL_DEFINITION_TEMPLATE: &str = "mcp_internal/rhd_set_todo_list/tool_definition.json";
pub const TODO_LIST_WITH_ITEMS_TEMPLATE: &str = "environment/todo_list_with_items.md";
pub const TODO_LIST_EMPTY_TEMPLATE: &str = "environment/todo_list_empty.md";
pub const TOOL_ERROR_INVALID_FORMAT_TEMPLATE: &str = "mcp_internal/rhd_set_todo_list/tool_error_invalid_format.md";

/// Tag for the contract system message.
pub const CONTRACT_TAG: &str = "todo_list:contract";

/// Loaded templates.
#[derive(Debug, Clone)]
pub struct Templates {
    contract: String,
    tool_definition: String,
    todo_list_with_items: String,
    todo_list_empty: String,
    tool_error_invalid_format: String,
}

impl Templates {
    /// Load all templates from embedded files.
    pub fn load() -> Result<Self, TemplateError> {
        let contract = Self::load_file(CONTRACT_TEMPLATE)?;
        let tool_definition = Self::load_file(TOOL_DEFINITION_TEMPLATE)?;
        let todo_list_with_items = Self::load_file(TODO_LIST_WITH_ITEMS_TEMPLATE)?;
        let todo_list_empty = Self::load_file(TODO_LIST_EMPTY_TEMPLATE)?;
        let tool_error_invalid_format = Self::load_file(TOOL_ERROR_INVALID_FORMAT_TEMPLATE)?;

        Ok(Self {
            contract,
            tool_definition,
            todo_list_with_items,
            todo_list_empty,
            tool_error_invalid_format,
        })
    }

    fn load_file(path: &str) -> Result<String, TemplateError> {
        TEMPLATES_DIR
            .get_file(path)
            .ok_or_else(|| TemplateError::NotFound(path.to_string()))?
            .contents_utf8()
            .map(|s| s.to_string())
            .ok_or_else(|| TemplateError::InvalidEncoding(path.to_string()))
    }

    /// Get the contract template content.
    pub fn contract(&self) -> &str {
        &self.contract
    }

    /// Get the tool definition JSON.
    pub fn tool_definition(&self) -> &str {
        &self.tool_definition
    }

    /// Parse tool definition as JSON value.
    pub fn tool_definition_json(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::from_str(&self.tool_definition)
    }

    /// Render the todo list with items template.
    pub fn render_todo_list_with_items(&self, items: &[TodoItem]) -> String {
        let table_rows: Vec<String> = items
            .iter()
            .enumerate()
            .map(|(idx, item)| {
                format!("| {} | {} | {} |", idx + 1, item.content, item.status)
            })
            .collect();
        
        let table_content = table_rows.join("\n");
        self.todo_list_with_items.replace("{todoItems}", &table_content)
    }

    /// Get the empty todo list template.
    pub fn todo_list_empty(&self) -> &str {
        &self.todo_list_empty
    }

    /// Get the tool error template for invalid format.
    pub fn tool_error_invalid_format(&self) -> &str {
        &self.tool_error_invalid_format
    }
}

/// Errors that can occur during template loading.
#[derive(Debug, thiserror::Error)]
pub enum TemplateError {
    #[error("template not found: {0}")]
    NotFound(String),
    #[error("invalid encoding in template: {0}")]
    InvalidEncoding(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::TodoStatus;

    #[test]
    fn test_load_templates() {
        let templates = Templates::load();
        assert!(templates.is_ok(), "Failed to load templates: {:?}", templates.err());
    }

    #[test]
    fn test_render_todo_list_with_items() {
        let templates = Templates::load().unwrap();
        let items = vec![
            TodoItem::new("Task 1".to_string(), TodoStatus::Pending),
            TodoItem::new("Task 2".to_string(), TodoStatus::Completed),
        ];
        
        let rendered = templates.render_todo_list_with_items(&items);
        assert!(rendered.contains("| 1 | Task 1 | Pending |"));
        assert!(rendered.contains("| 2 | Task 2 | Completed |"));
    }

    #[test]
    fn test_tool_definition_is_valid_json() {
        let templates = Templates::load().unwrap();
        let json = templates.tool_definition_json();
        assert!(json.is_ok(), "Tool definition is not valid JSON: {:?}", json.err());
    }
}
