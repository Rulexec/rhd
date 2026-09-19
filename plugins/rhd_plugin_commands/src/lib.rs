//! Slash-command plugin for RHD queued messages.
//!
//! Reacts to `ai_completions:preDrainQueue` (emitted by the AI completions
//! plugin right before queued messages are promoted into the conversation),
//! executes commands configured under leading `/name` tokens of queued user
//! messages, and rewrites the queue accordingly.
//!
//! Contains the config model, the pure command parser, the queue executor,
//! and the plugin lifecycle.

pub mod config;
pub mod executor;
pub mod parser;
pub mod plugin;

/// Custom event handled by this plugin (emitted by rhd_plugin_ai_completions).
pub const PRE_DRAIN_QUEUE_EVENT: &str = "ai_completions:preDrainQueue";

/// Tag prefix added to queued prompt messages for observability. The tag
/// survives the drain into regular messages.
pub fn prompt_tag(name: &str) -> String {
    format!("commands:prompt:{}", name)
}
