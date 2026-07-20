/// Compile-time template registry
/// Templates are embedded into the binary at compile time using include_str!

pub struct TemplateRegistry;

impl TemplateRegistry {
    /// Get a template by name
    pub fn get(name: &str) -> Option<&'static str> {
        match name {
            // Environment templates
            "environment/details_no_role" => Some(include_str!("../../../templates/environment/details_no_role.md")),
            "environment/details_with_role" => Some(include_str!("../../../templates/environment/details_with_role.md")),
            "environment/todo_list_empty" => Some(include_str!("../../../templates/environment/todo_list_empty.md")),
            "environment/todo_list_with_items" => Some(include_str!("../../../templates/environment/todo_list_with_items.md")),
            
            // MCP internal tool templates
            "mcp_internal/rhd_set_todo_list/tool_definition" => Some(include_str!("../../../templates/mcp_internal/rhd_set_todo_list/tool_definition.json")),
            "mcp_internal/rhd_set_todo_list/contract" => Some(include_str!("../../../templates/mcp_internal/rhd_set_todo_list/contract.md")),
            "mcp_internal/rhd_set_role/tool_definition" => Some(include_str!("../../../templates/mcp_internal/rhd_set_role/tool_definition.json")),
            "mcp_internal/rhd_set_flag/tool_definition" => Some(include_str!("../../../templates/mcp_internal/rhd_set_flag/tool_definition.json")),
            
            // Role templates
            "roles/roles_list_prompt" => Some(include_str!("../../../templates/roles/roles_list_prompt.md")),
            "roles/role_switch_prompt" => Some(include_str!("../../../templates/roles/role_switch_prompt.md")),
            
            _ => None,
        }
    }
}
