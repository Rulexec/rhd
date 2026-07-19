use super::*;

#[test]
fn test_get_existing_template() {
    let loader = TemplateLoader::new();
    assert!(loader.get_template("environment/todo_list_empty").is_some());
    assert!(loader.get_template("environment/todo_list_with_items").is_some());
    assert!(loader.get_template("environment/details_no_role").is_some());
    assert!(loader.get_template("environment/details_with_role").is_some());
    assert!(loader.get_template("mcp_internal/rhd_set_todo_list/contract").is_some());
    assert!(loader.get_template("mcp_internal/rhd_set_todo_list/tool_definition").is_some());
    assert!(loader.get_template("mcp_internal/rhd_set_role/tool_definition").is_some());
    assert!(loader.get_template("mcp_internal/rhd_set_flag/tool_definition").is_some());
    assert!(loader.get_template("roles/roles_list_prompt").is_some());
    assert!(loader.get_template("roles/role_switch_prompt").is_some());
}

#[test]
fn test_get_nonexistent_template() {
    let loader = TemplateLoader::new();
    assert!(loader.get_template("nonexistent_template").is_none());
}
