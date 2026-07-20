use crate::tools::{extract_mcp_id_from_tool_name, split_tool_name};

#[test]
fn test_extract_mcp_id_from_tool_name_with_prefix() {
    assert_eq!(extract_mcp_id_from_tool_name("mock1/echo"), "mock1");
    assert_eq!(extract_mcp_id_from_tool_name("server/tool"), "server");
}

#[test]
fn test_extract_mcp_id_from_tool_name_without_prefix() {
    assert_eq!(extract_mcp_id_from_tool_name("echo"), "echo");
    assert_eq!(extract_mcp_id_from_tool_name("rhd_set_flag"), "rhd_set_flag");
}

#[test]
fn test_split_tool_name_with_prefix() {
    let (mcp_id, bare_name) = split_tool_name("mock1/echo");
    assert_eq!(mcp_id, "mock1");
    assert_eq!(bare_name, "echo");
}

#[test]
fn test_split_tool_name_without_prefix() {
    let (mcp_id, bare_name) = split_tool_name("echo");
    assert_eq!(mcp_id, "");
    assert_eq!(bare_name, "echo");
}
