//! Template loading.

use include_dir::{include_dir, Dir};

// Embed templates directory at compile time
static TEMPLATES_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/../../templates");

/// Tool definition template for rhd_choice.
pub const TOOL_DEFINITION_TEMPLATE: &str = "mcp_internal/rhd_choice/tool_definition.json";

/// Loaded templates.
#[derive(Debug, Clone)]
pub struct Templates {
    tool_definition: String,
}

impl Templates {
    /// Load all templates from embedded files.
    pub fn load() -> Result<Self, TemplateError> {
        Ok(Self {
            tool_definition: Self::load_file(TOOL_DEFINITION_TEMPLATE)?,
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

    /// Get the tool definition JSON.
    pub fn tool_definition(&self) -> &str {
        &self.tool_definition
    }

    /// Parse tool definition as JSON value.
    pub fn tool_definition_json(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::from_str(&self.tool_definition)
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
