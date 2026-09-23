//! Template loading.

use include_dir::{include_dir, Dir};

// Embed templates directory at compile time
static TEMPLATES_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/../../templates");

/// Tool definition template for rhd_sub_chat.
pub const SPAWN_TOOL_PATH: &str = "mcp_internal/rhd_sub_chat/tool_definition.json";
/// Tool definition template for rhd_sub_chat_status.
pub const STATUS_TOOL_PATH: &str = "mcp_internal/rhd_sub_chat_status/tool_definition.json";
/// Tool definition template for rhd_sub_chat_await.
pub const AWAIT_TOOL_PATH: &str = "mcp_internal/rhd_sub_chat_await/tool_definition.json";

/// Loaded templates.
#[derive(Debug, Clone)]
pub struct Templates {
    spawn: String,
    status: String,
    await_tool: String,
}

impl Templates {
    /// Load all templates from embedded files.
    pub fn load() -> Result<Self, TemplateError> {
        Ok(Self {
            spawn: Self::load_file(SPAWN_TOOL_PATH)?,
            status: Self::load_file(STATUS_TOOL_PATH)?,
            await_tool: Self::load_file(AWAIT_TOOL_PATH)?,
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

    /// Get the `rhd_sub_chat` tool definition JSON.
    pub fn spawn_definition(&self) -> &str {
        &self.spawn
    }

    /// Get the `rhd_sub_chat_status` tool definition JSON.
    pub fn status_definition(&self) -> &str {
        &self.status
    }

    /// Get the `rhd_sub_chat_await` tool definition JSON.
    pub fn await_definition(&self) -> &str {
        &self.await_tool
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

    #[test]
    fn templates_load() {
        assert!(Templates::load().is_ok());
    }

    #[test]
    fn each_definition_parses_as_tool_definition_with_expected_name() {
        let templates = Templates::load().unwrap();
        for (json, expected_name) in [
            (templates.spawn_definition(), "rhd_sub_chat"),
            (templates.status_definition(), "rhd_sub_chat_status"),
            (templates.await_definition(), "rhd_sub_chat_await"),
        ] {
            let def: rhd_chat_api::ToolDefinition = serde_json::from_str(json)
                .unwrap_or_else(|e| panic!("{expected_name} definition must parse: {e}"));
            assert_eq!(def.tool_type, "function", "{expected_name} type");
            assert_eq!(def.function.name, expected_name);
        }
    }
}
