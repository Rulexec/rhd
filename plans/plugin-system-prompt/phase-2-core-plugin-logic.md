# Phase 2: Core Plugin Logic

## Overview

This phase implements the main plugin lifecycle (connect, register, monitor setup) and the system prompt detection/injection logic. This establishes the plugin's ability to monitor chats and inject system prompts.

**Scope:**
- Implement CLI entry point with argument parsing
- Implement plugin lifecycle (connect, register, create monitors)
- Implement system prompt detection and injection logic
- Implement startup reconciliation
- Implement main loop for continuous monitoring

**Out of Scope:**
- Event coordination with `ai_completions:preRequest`
- Integration tests

## Files to Create/Modify

### 1. `plugins/rhd_plugin_system_prompt/src/main.rs`

**Create new file:**

```rust
//! Entry point for the system prompt plugin.

use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use rhd_plugin_system_prompt::{config, plugin};

#[derive(Parser, Debug)]
#[command(name = "rhd_plugin_system_prompt")]
#[command(about = "System prompt plugin for RHD chat system")]
struct Args {
    /// WebSocket URL of the chat server
    #[arg(long)]
    server_url: String,

    /// Path to configuration file
    #[arg(long)]
    config: String,

    /// Plugin ID (defaults to "system_prompt")
    #[arg(long, default_value = "system_prompt")]
    plugin_id: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env().add_directive("rhd_plugin_system_prompt=info".parse()?))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Parse arguments
    let args = Args::parse();

    // Load configuration
    let (config, cached_prompts) = config::load_config(&args.config)?;

    tracing::info!("Starting system prompt plugin");
    tracing::info!("Server URL: {}", args.server_url);
    tracing::info!("Plugin ID: {}", args.plugin_id);
    tracing::info!("Loaded {} system prompts", cached_prompts.len());

    // Run plugin
    plugin::run_plugin(&args.server_url, &args.plugin_id, config, cached_prompts).await?;

    Ok(())
}
```

**Rationale:** Matches the structure of `rhd_plugin_ai_completions/src/main.rs` for consistency.

### 2. `plugins/rhd_plugin_system_prompt/src/system_prompt.rs`

**Create new file:**

```rust
//! System prompt detection and injection logic.

use rhd_chat_api::{AddMessageParams, Message};
use rhd_chat_client::{ChatClient, ChatState};

use crate::config::CachedPrompt;

/// Tag prefix for system prompt tags.
pub const SYSTEM_PROMPT_TAG_PREFIX: &str = "systemPrompt:";

/// Check if a chat has a system prompt message for the given prompt name.
///
/// A system prompt message is identified by having a tag matching `systemPrompt:<name>`.
pub fn has_system_prompt_message(chat_state: &ChatState, prompt_name: &str) -> bool {
    let expected_tag = format!("{}{}", SYSTEM_PROMPT_TAG_PREFIX, prompt_name);
    chat_state.messages.iter().any(|msg| {
        msg.tags.iter().any(|tag| tag == &expected_tag)
    })
}

/// Check if a chat has an unfinished assistant message.
///
/// An unfinished message is one where:
/// - `role == "assistant"` AND
/// - (`is_streaming == true` OR `is_finished == false`)
pub fn has_unfinished_assistant_message(chat_state: &ChatState) -> bool {
    chat_state.messages.iter().any(|msg| {
        msg.role == "assistant" && (msg.is_streaming || !msg.is_finished)
    })
}

/// Parse a system prompt tag and extract the prompt name.
///
/// Returns `Some(name)` if the tag matches the pattern `systemPrompt:<name>`,
/// otherwise returns `None`.
pub fn parse_system_prompt_tag(tag: &str) -> Option<&str> {
    tag.strip_prefix(SYSTEM_PROMPT_TAG_PREFIX)
}

/// Get all system prompt names that a chat needs based on its tags.
///
/// Returns a list of prompt names extracted from tags matching `systemPrompt:<name>`.
pub fn get_required_prompt_names(chat_state: &ChatState) -> Vec<String> {
    chat_state
        .tags
        .iter()
        .filter_map(|tag| parse_system_prompt_tag(tag))
        .map(|name| name.to_string())
        .collect()
}

/// Inject a system prompt into a chat.
///
/// This adds a message with:
/// - `role: "system"`
/// - `content: <prompt content>`
/// - `tags: ["systemPrompt:<name>"]`
/// - `is_finished: true`
/// - `is_streaming: false`
pub async fn inject_system_prompt(
    client: &ChatClient,
    chat_id: i64,
    prompt: &CachedPrompt,
) -> Result<(), InjectError> {
    let tag = format!("{}{}", SYSTEM_PROMPT_TAG_PREFIX, prompt.name);

    client
        .add_message(AddMessageParams {
            chat_id,
            role: "system".to_string(),
            content: prompt.content.clone(),
            tool_call_id: None,
            reasoning_content: None,
            tags: vec![tag],
            is_finished: true,
            is_streaming: false,
        })
        .await
        .map_err(|e| InjectError::AddMessage(e.to_string()))?;

    tracing::info!(
        chat_id = chat_id,
        prompt_name = %prompt.name,
        "injected system prompt"
    );

    Ok(())
}

/// Process a chat and inject any missing system prompts.
///
/// This function:
/// 1. Checks if the chat has an unfinished assistant message (if so, skip)
/// 2. Gets the list of required prompt names from chat tags
/// 3. For each required prompt, checks if it already exists
/// 4. Injects any missing prompts
///
/// Returns the number of prompts injected.
pub async fn process_chat(
    client: &ChatClient,
    chat_state: &ChatState,
    cached_prompts: &[CachedPrompt],
) -> Result<usize, InjectError> {
    // Skip if chat has unfinished assistant message
    if has_unfinished_assistant_message(chat_state) {
        tracing::debug!(
            chat_id = chat_state.chat_id,
            "skipping chat with unfinished assistant message"
        );
        return Ok(0);
    }

    let required_names = get_required_prompt_names(chat_state);
    let mut injected_count = 0;

    for prompt_name in required_names {
        // Check if prompt already exists
        if has_system_prompt_message(chat_state, &prompt_name) {
            tracing::debug!(
                chat_id = chat_state.chat_id,
                prompt_name = %prompt_name,
                "system prompt already exists"
            );
            continue;
        }

        // Find the cached prompt
        let prompt = cached_prompts.iter().find(|p| p.name == prompt_name);
        let Some(prompt) = prompt else {
            tracing::warn!(
                chat_id = chat_state.chat_id,
                prompt_name = %prompt_name,
                "prompt not found in config"
            );
            continue;
        };

        // Inject the prompt
        inject_system_prompt(client, chat_state.chat_id, prompt).await?;
        injected_count += 1;
    }

    Ok(injected_count)
}

/// Errors that can occur during system prompt injection.
#[derive(Debug, thiserror::Error)]
pub enum InjectError {
    #[error("failed to add message: {0}")]
    AddMessage(String),
}
```

**Key Implementation Details:**

1. **Tag Parsing**: The `parse_system_prompt_tag` function uses `strip_prefix` to extract the prompt name from tags like `systemPrompt:warhammer`.

2. **Duplicate Prevention**: The `has_system_prompt_message` function checks if any message in the chat has a tag matching `systemPrompt:<name>`.

3. **Unfinished Message Check**: The `has_unfinished_assistant_message` function checks for assistant messages that are streaming or not finished.

4. **Injection**: The `inject_system_prompt` function adds a message with role "system" and the appropriate tag.

### 3. `plugins/rhd_plugin_system_prompt/src/plugin.rs`

**Create new file:**

```rust
//! Core plugin lifecycle and main loop.

use std::sync::Arc;
use std::time::Duration;

use rhd_chat_api::{GetPendingAcksParams, RegisterPluginParams};
use rhd_chat_client::{ChatClient, ChatMonitor};

use crate::config::{CachedPrompt, PluginConfig};
use crate::system_prompt;

/// Run the system prompt plugin.
///
/// This function implements the complete plugin lifecycle:
/// 1. Connect to chat server
/// 2. Register as plugin
/// 3. Process pending acks
/// 4. Create monitors
/// 5. Subscribe to all chats
/// 6. Startup reconciliation
/// 7. Main loop: check for missing system prompts
pub async fn run_plugin(
    server_url: &str,
    plugin_id: &str,
    _config: PluginConfig,
    cached_prompts: Vec<CachedPrompt>,
) -> Result<(), PluginError> {
    // Connect to chat server
    let client = Arc::new(
        ChatClient::connect_with_retry(server_url)
            .await
            .map_err(|e| PluginError::Connection(e.to_string()))?,
    );

    tracing::info!("Connected to chat server");

    // Register as plugin
    client
        .register_plugin(RegisterPluginParams {
            plugin_id: plugin_id.to_string(),
        })
        .await
        .map_err(|e| PluginError::Registration(e.to_string()))?;

    tracing::info!("Registered as plugin: {}", plugin_id);

    // Get pending acks
    let pending_acks = client
        .get_pending_acks(GetPendingAcksParams {})
        .await
        .map_err(|e| PluginError::PendingAcks(e.to_string()))?;

    tracing::info!("Found {} pending acks", pending_acks.pending_events.len());

    // Process pending acks
    for event in pending_acks.pending_events {
        tracing::info!("Processing pending event: {}", event.event_name);
        client
            .ack_custom_event(rhd_chat_api::AckCustomEventParams {
                event_id: event.event_id,
                is_rejected: None,
            })
            .await
            .map_err(|e| PluginError::PendingAcks(e.to_string()))?;
    }

    // Create chat monitor
    let chat_monitor = Arc::new(
        client
            .create_chat_monitor()
            .await
            .map_err(|e| PluginError::MonitorCreate(e.to_string()))?,
    );

    tracing::info!("Created chat monitor");

    // Subscribe to all chats
    chat_monitor
        .subscribe_to_all_chats()
        .await
        .map_err(|e| PluginError::Subscription(e.to_string()))?;

    tracing::info!("Subscribed to all chats");

    // Startup reconciliation
    let injected_count = startup_reconciliation(&client, &chat_monitor, &cached_prompts).await?;
    tracing::info!("Startup reconciliation: injected {} system prompts", injected_count);

    // Main loop
    loop {
        let chat_ids = chat_monitor.get_chat_ids().await;
        tracing::debug!(
            monitored_chats = chat_ids.len(),
            "main loop iteration"
        );

        for chat_id in chat_ids {
            if let Some(chat_state) = chat_monitor.get_chat_state(chat_id).await {
                match system_prompt::process_chat(&client, &chat_state, &cached_prompts).await {
                    Ok(count) if count > 0 => {
                        tracing::info!(
                            chat_id = chat_id,
                            injected_count = count,
                            "injected system prompts in main loop"
                        );
                    }
                    Ok(_) => {}
                    Err(e) => {
                        tracing::error!(
                            chat_id = chat_id,
                            error = %e,
                            "failed to process chat"
                        );
                    }
                }
            }
        }

        // Wait before next check
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

/// Perform startup reconciliation.
///
/// This checks all monitored chats and injects any missing system prompts.
/// This handles chats that existed before the plugin started.
async fn startup_reconciliation(
    client: &ChatClient,
    chat_monitor: &ChatMonitor,
    cached_prompts: &[CachedPrompt],
) -> Result<usize, PluginError> {
    let mut total_injected = 0;

    for chat_id in chat_monitor.get_chat_ids().await {
        if let Some(chat_state) = chat_monitor.get_chat_state(chat_id).await {
            match system_prompt::process_chat(client, &chat_state, cached_prompts).await {
                Ok(count) => {
                    total_injected += count;
                }
                Err(e) => {
                    tracing::error!(
                        chat_id = chat_id,
                        error = %e,
                        "failed to process chat during startup reconciliation"
                    );
                }
            }
        }
    }

    Ok(total_injected)
}

/// Errors that can occur during plugin execution.
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("failed to connect to server: {0}")]
    Connection(String),
    #[error("failed to register plugin: {0}")]
    Registration(String),
    #[error("failed to get pending acks: {0}")]
    PendingAcks(String),
    #[error("failed to create monitor: {0}")]
    MonitorCreate(String),
    #[error("failed to subscribe: {0}")]
    Subscription(String),
    #[error("failed to inject system prompt: {0}")]
    Inject(#[from] system_prompt::InjectError),
}
```

**Key Implementation Details:**

1. **Plugin Lifecycle**: Follows the same pattern as `rhd_plugin_ai_completions`:
   - Connect with retry
   - Register as plugin
   - Process pending acks
   - Create monitors
   - Subscribe to all chats
   - Startup reconciliation
   - Main loop

2. **Startup Reconciliation**: Iterates through all monitored chats and injects missing system prompts. This handles chats that existed before the plugin started.

3. **Main Loop**: Periodically checks all monitored chats for missing system prompts. This handles tags added after the plugin started.

4. **Error Handling**: Errors during chat processing are logged but don't stop the main loop. This ensures the plugin continues to function even if individual chats fail.

## Tests

### Unit Tests for `system_prompt.rs`

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rhd_chat_api::Message;

    fn create_message(id: i64, role: &str, tags: Vec<String>) -> Message {
        Message {
            id,
            chat_id: 1,
            role: role.to_string(),
            content: "test".to_string(),
            tool_call_id: None,
            created_at: Utc::now(),
            reasoning_content: None,
            tags,
            is_finished: true,
            is_streaming: false,
            tool_calls: vec![],
        }
    }

    fn create_chat_state(messages: Vec<Message>, tags: Vec<String>) -> ChatState {
        ChatState {
            chat_id: 1,
            messages,
            queued_messages_count: 0,
            tags,
            version: 1,
        }
    }

    #[test]
    fn test_parse_system_prompt_tag_valid() {
        assert_eq!(parse_system_prompt_tag("systemPrompt:warhammer"), Some("warhammer"));
        assert_eq!(parse_system_prompt_tag("systemPrompt:jokeTeller"), Some("jokeTeller"));
    }

    #[test]
    fn test_parse_system_prompt_tag_invalid() {
        assert_eq!(parse_system_prompt_tag("other:tag"), None);
        assert_eq!(parse_system_prompt_tag("systemPrompt"), None);
        assert_eq!(parse_system_prompt_tag(""), None);
    }

    #[test]
    fn test_has_system_prompt_message_true() {
        let messages = vec![
            create_message(1, "user", vec![]),
            create_message(2, "system", vec!["systemPrompt:warhammer".to_string()]),
        ];
        let state = create_chat_state(messages, vec![]);
        assert!(has_system_prompt_message(&state, "warhammer"));
    }

    #[test]
    fn test_has_system_prompt_message_false() {
        let messages = vec![create_message(1, "user", vec![])];
        let state = create_chat_state(messages, vec![]);
        assert!(!has_system_prompt_message(&state, "warhammer"));
    }

    #[test]
    fn test_has_unfinished_assistant_message_streaming() {
        let mut msg = create_message(1, "assistant", vec![]);
        msg.is_streaming = true;
        let messages = vec![msg];
        let state = create_chat_state(messages, vec![]);
        assert!(has_unfinished_assistant_message(&state));
    }

    #[test]
    fn test_has_unfinished_assistant_message_not_finished() {
        let mut msg = create_message(1, "assistant", vec![]);
        msg.is_finished = false;
        let messages = vec![msg];
        let state = create_chat_state(messages, vec![]);
        assert!(has_unfinished_assistant_message(&state));
    }

    #[test]
    fn test_has_unfinished_assistant_message_all_finished() {
        let messages = vec![create_message(1, "assistant", vec![])];
        let state = create_chat_state(messages, vec![]);
        assert!(!has_unfinished_assistant_message(&state));
    }

    #[test]
    fn test_get_required_prompt_names() {
        let tags = vec![
            "systemPrompt:warhammer".to_string(),
            "systemPrompt:jokeTeller".to_string(),
            "other:tag".to_string(),
        ];
        let state = create_chat_state(vec![], tags);
        let names = get_required_prompt_names(&state);
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"warhammer".to_string()));
        assert!(names.contains(&"jokeTeller".to_string()));
    }
}
```

## Implementation Notes

1. **Plugin ID**: Default to `"system_prompt"` but allow override via `--plugin-id` argument.

2. **Tag Format**: Use `systemPrompt:<name>` format where `<name>` matches the config key. This allows multiple system prompts per chat.

3. **Message Role**: Use `"system"` role for system prompt messages. This is the standard OpenAI format.

4. **Message Tagging**: System prompt messages receive the same tag as the chat (`systemPrompt:<name>`). This enables easy detection of existing prompts.

5. **Startup Reconciliation**: This is critical for handling chats that existed before the plugin started. Without this, existing chats with tags would never get system prompts.

6. **Main Loop Interval**: Use 1 second sleep between iterations. This balances responsiveness with resource usage.

7. **Error Handling**: Errors during chat processing are logged but don't stop the main loop. This ensures the plugin continues to function even if individual chats fail.

## Dependencies

- **Phase 1** - Requires configuration loading and prompt caching
- This phase must be completed before Phase 3

## Success Criteria

- [ ] Plugin connects to chat server and registers successfully
- [ ] Plugin subscribes to all chats
- [ ] System prompts are injected for chats with matching tags
- [ ] Duplicate system prompts are prevented
- [ ] Unfinished assistant messages block system prompt injection
- [ ] Startup reconciliation injects prompts for existing chats
- [ ] Main loop continues to inject prompts for newly tagged chats
- [ ] All unit tests pass
