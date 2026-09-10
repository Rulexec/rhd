//! Integration tests for the choice plugin.

use rhd_plugin_choice::templates::Templates;

#[test]
fn test_templates_load() {
    let templates = Templates::load();
    assert!(templates.is_ok(), "Failed to load templates: {:?}", templates.err());
}

#[test]
fn test_tool_definition_valid_json() {
    let templates = Templates::load().unwrap();
    let json = templates.tool_definition_json();
    assert!(json.is_ok(), "Tool definition is not valid JSON: {:?}", json.err());

    let json = json.unwrap();
    assert_eq!(json["type"], "function");
    assert_eq!(json["function"]["name"], "rhd_choice");
    assert!(json["function"]["parameters"]["properties"]["question"].is_object());
    assert!(json["function"]["parameters"]["properties"]["options"].is_object());
    let required = json["function"]["parameters"]["required"].as_array().unwrap();
    assert!(required.contains(&serde_json::json!("question")));
    assert!(required.contains(&serde_json::json!("options")));
}
