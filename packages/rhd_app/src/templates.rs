/// Compile-time template registry
/// Templates are embedded into the binary at compile time using include_str!

pub struct TemplateRegistry;

impl TemplateRegistry {
    /// Get a template by name
    pub fn get(name: &str) -> Option<&'static str> {
        match name {
            "environment_details_no_role" => Some(include_str!("../../../templates/environment_details_no_role.md")),
            "environment_details_with_role" => Some(include_str!("../../../templates/environment_details_with_role.md")),
            "rhd_set_todo_list_contract" => Some(include_str!("../../../templates/rhd_set_todo_list_contract.md")),
            "todo_list_empty" => Some(include_str!("../../../templates/todo_list_empty.md")),
            "todo_list_with_items" => Some(include_str!("../../../templates/todo_list_with_items.md")),
            _ => None,
        }
    }
    
    /// Get all available template names
    pub fn list_templates() -> Vec<&'static str> {
        vec![
            "environment_details_no_role",
            "environment_details_with_role",
            "rhd_set_todo_list_contract",
            "todo_list_empty",
            "todo_list_with_items",
        ]
    }
}
