pub fn extract_mcp_id_from_tool_name(tool_name: &str) -> String {
    tool_name.split('/').next().unwrap_or("").to_string()
}

pub fn split_tool_name(tool_name: &str) -> (String, String) {
    if let Some((mcp_id, bare_name)) = tool_name.split_once('/') {
        (mcp_id.to_string(), bare_name.to_string())
    } else {
        (String::new(), tool_name.to_string())
    }
}
