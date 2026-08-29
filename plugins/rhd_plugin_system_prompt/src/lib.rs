//! System prompt plugin for RHD chat system.
//!
//! This plugin automatically injects system prompts into chats based on chat tags.
//! It monitors chats for tags like `systemPrompt:warhammer` and adds corresponding
//! system prompt messages from configured markdown files.

pub mod config;
pub mod plugin;
pub mod system_prompt;
