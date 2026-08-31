//! Markdown checklist parser for todo lists.

use regex::Regex;
use std::fmt;

/// Status of a todo item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TodoStatus {
    /// `[ ]` - Task not yet started
    Pending,
    /// `[-]` - Task currently being worked on
    InProgress,
    /// `[x]` - Task finished
    Completed,
    /// `[!]` - Task no longer needed
    Discarded,
}

impl TodoStatus {
    /// Parse status from checkbox marker.
    pub fn from_marker(marker: &str) -> Option<Self> {
        match marker {
            " " => Some(TodoStatus::Pending),
            "-" => Some(TodoStatus::InProgress),
            "x" | "X" => Some(TodoStatus::Completed),
            "!" => Some(TodoStatus::Discarded),
            _ => None,
        }
    }

    /// Get the checkbox marker for this status.
    pub fn to_marker(&self) -> &'static str {
        match self {
            TodoStatus::Pending => " ",
            TodoStatus::InProgress => "-",
            TodoStatus::Completed => "x",
            TodoStatus::Discarded => "!",
        }
    }

    /// Get the display name for this status.
    pub fn display_name(&self) -> &'static str {
        match self {
            TodoStatus::Pending => "Pending",
            TodoStatus::InProgress => "In Progress",
            TodoStatus::Completed => "Completed",
            TodoStatus::Discarded => "Discarded",
        }
    }
}

impl fmt::Display for TodoStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// A single todo item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TodoItem {
    /// The content/description of the task.
    pub content: String,
    /// The status of the task.
    pub status: TodoStatus,
}

impl TodoItem {
    /// Create a new todo item.
    pub fn new(content: String, status: TodoStatus) -> Self {
        Self { content, status }
    }

    /// Format as markdown checklist line.
    pub fn to_markdown(&self) -> String {
        format!("[{}] {}", self.status.to_marker(), self.content)
    }
}

/// Error when parsing todo list.
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("invalid checkbox format at line {line}: '{content}'")]
    InvalidFormat { line: usize, content: String },
    #[error("empty todo list")]
    EmptyList,
}

/// Parse a markdown checklist string into todo items.
///
/// Expected format:
/// ```text
/// [ ] Pending task
/// [-] In progress task
/// [x] Completed task
/// [!] Discarded task
/// ```
pub fn parse_todo_list(input: &str) -> Result<Vec<TodoItem>, ParseError> {
    let mut items = Vec::new();
    
    for (line_num, line) in input.lines().enumerate() {
        let trimmed = line.trim();
        
        // Skip empty lines
        if trimmed.is_empty() {
            continue;
        }
        
        // Check if line starts with [ and has ] followed by space
        if trimmed.starts_with('[') {
            if let Some(bracket_end) = trimmed.find(']') {
                let marker = &trimmed[1..bracket_end];
                if marker.len() == 1 {
                    let marker_char = marker.chars().next().unwrap();
                    let rest = trimmed[bracket_end + 1..].trim();
                    
                    if let Some(status) = TodoStatus::from_marker(&marker_char.to_string()) {
                        items.push(TodoItem::new(rest.to_string(), status));
                        continue;
                    }
                }
            }
        }
        
        return Err(ParseError::InvalidFormat {
            line: line_num + 1,
            content: trimmed.to_string(),
        });
    }
    
    if items.is_empty() {
        return Err(ParseError::EmptyList);
    }
    
    Ok(items)
}

/// Format todo items as a markdown table for display.
pub fn format_todo_table(items: &[TodoItem]) -> String {
    let mut output = String::from("| # | Content | Status |\n|---|---------|--------|\n");
    
    for (idx, item) in items.iter().enumerate() {
        output.push_str(&format!("| {} | {} | {} |\n", idx + 1, item.content, item.status));
    }
    
    output
}

/// Format todo items as a markdown checklist.
pub fn format_todo_checklist(items: &[TodoItem]) -> String {
    items
        .iter()
        .map(|item| item.to_markdown())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_checklist() {
        let input = r#"[ ] Pending task
[-] In progress task
[x] Completed task
[!] Discarded task"#;
        
        let items = parse_todo_list(input).unwrap();
        assert_eq!(items.len(), 4);
        assert_eq!(items[0].status, TodoStatus::Pending);
        assert_eq!(items[0].content, "Pending task");
        assert_eq!(items[1].status, TodoStatus::InProgress);
        assert_eq!(items[2].status, TodoStatus::Completed);
        assert_eq!(items[3].status, TodoStatus::Discarded);
    }

    #[test]
    fn test_parse_with_whitespace() {
        let input = "  [ ] Task with leading spaces  \n\t[x] Task with tab";
        let items = parse_todo_list(input).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].content, "Task with leading spaces");
        assert_eq!(items[1].content, "Task with tab");
    }

    #[test]
    fn test_parse_empty_lines_skipped() {
        let input = "[ ] Task 1\n\n[x] Task 2\n\n";
        let items = parse_todo_list(input).unwrap();
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn test_parse_invalid_format() {
        let input = "[ ] Valid task\nInvalid line without checkbox";
        let result = parse_todo_list(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_empty_list() {
        let input = "\n\n";
        let result = parse_todo_list(input);
        assert!(matches!(result, Err(ParseError::EmptyList)));
    }

    #[test]
    fn test_format_todo_table() {
        let items = vec![
            TodoItem::new("Task 1".to_string(), TodoStatus::Pending),
            TodoItem::new("Task 2".to_string(), TodoStatus::Completed),
        ];
        
        let table = format_todo_table(&items);
        assert!(table.contains("| 1 | Task 1 | Pending |"));
        assert!(table.contains("| 2 | Task 2 | Completed |"));
    }

    #[test]
    fn test_format_todo_checklist() {
        let items = vec![
            TodoItem::new("Task 1".to_string(), TodoStatus::Pending),
            TodoItem::new("Task 2".to_string(), TodoStatus::InProgress),
        ];
        
        let checklist = format_todo_checklist(&items);
        assert!(checklist.contains("[ ] Task 1"));
        assert!(checklist.contains("[-] Task 2"));
    }

    #[test]
    fn test_todo_item_to_markdown() {
        let item = TodoItem::new("Test task".to_string(), TodoStatus::Completed);
        assert_eq!(item.to_markdown(), "[x] Test task");
    }

    #[test]
    fn test_parse_single_line_pending() {
        let input = "[ ] Task 1";
        let items = parse_todo_list(input).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].content, "Task 1");
        assert_eq!(items[0].status, TodoStatus::Pending);
    }

    #[test]
    fn test_parse_single_line_completed() {
        let input = "[x] Task 2";
        let items = parse_todo_list(input).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].content, "Task 2");
        assert_eq!(items[0].status, TodoStatus::Completed);
    }

    #[test]
    fn test_parse_single_line_in_progress() {
        let input = "[-] Task 3";
        let items = parse_todo_list(input).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].content, "Task 3");
        assert_eq!(items[0].status, TodoStatus::InProgress);
    }

    #[test]
    fn test_parse_single_line_discarded() {
        let input = "[!] Task 4";
        let items = parse_todo_list(input).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].content, "Task 4");
        assert_eq!(items[0].status, TodoStatus::Discarded);
    }
}
