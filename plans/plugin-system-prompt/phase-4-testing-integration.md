# Phase 4: Testing & Integration

## Overview

This phase writes comprehensive tests and ensures the plugin integrates correctly with the workspace. This validates the implementation and prevents regressions.

**Scope:**
- Run and verify unit tests from previous phases
- Create integration tests for full plugin lifecycle
- Create README documentation
- Verify workspace integration

**Out of Scope:**
- Performance testing
- Load testing

## Files to Create/Modify

### 1. `plugins/rhd_plugin_system_prompt/tests/integration_test.rs`

**Create new file:**

```rust
//! Integration tests for the system prompt plugin.

use std::io::Write;
use std::time::Duration;

use rhd_chat_api::{
    AddMessageParams, CreateChatParams, GetChatParams, RegisterPluginParams,
    SendCustomEventParams,
};
use rhd_chat_client::ChatClient;
use rhd_chat_server::ChatServer;
use tempfile::NamedTempFile;

/// Test helper to create a test server and client.
async fn setup_test_server() -> (ChatServer, ChatClient, String) {
    let server = ChatServer::start_test_server().await;
    let client = ChatClient::connect(&server.url())
        .await
        .expect("Failed to connect to test server");
    (server, client, server.url())
}

/// Test helper to create a config file with prompt files.
fn create_test_config(prompts: &[(&str, &str)]) -> (NamedTempFile, Vec<NamedTempFile>) {
    let mut config_file = NamedTempFile::new().unwrap();
    let config_dir = config_file.path().parent().unwrap();

    let mut prompt_files = Vec::new();
    let mut config_content = String::from("systemPrompts:\n");

    for (name, content) in prompts {
        let mut prompt_file = NamedTempFile::new().unwrap();
        write!(prompt_file, "{}", content).unwrap();
        let prompt_path = prompt_file.path().to_str().unwrap();
        config_content.push_str(&format!("  {}: {}\n", name, prompt_path));
        prompt_files.push(prompt_file);
    }

    write!(config_file, "{}", config_content).unwrap();
    (config_file, prompt_files)
}

#[tokio::test]
async fn test_plugin_startup_with_valid_config() {
    let (_server, _client, server_url) = setup_test_server().await;
    let (config_file, _prompt_files) = create_test_config(&[
        ("warhammer", "Warhammer 40k system prompt"),
        ("jokeTeller", "Joke teller system prompt"),
    ]);

    // Load config to verify it works
    let (config, cached) = rhd_plugin_system_prompt::config::load_config(
        config_file.path().to_str().unwrap(),
    )
    .expect("Failed to load config");

    assert_eq!(config.system_prompts.len(), 2);
    assert_eq!(cached.len(), 2);
}

#[tokio::test]
async fn test_plugin_startup_with_missing_prompt_file() {
    let mut config_file = NamedTempFile::new().unwrap();
    writeln!(
        config_file,
        r#"
systemPrompts:
  warhammer: /nonexistent/path/prompt.md
"#
    )
    .unwrap();

    let result = rhd_plugin_system_prompt::config::load_config(
        config_file.path().to_str().unwrap(),
    );

    assert!(result.is_err());
}

#[tokio::test]
async fn test_system_prompt_injection_on_chat_with_tag() {
    let (_server, client, _server_url) = setup_test_server().await;
    let (config_file, _prompt_files) = create_test_config(&[
        ("warhammer", "Warhammer 40k system prompt"),
    ]);

    let (_config, cached_prompts) = rhd_plugin_system_prompt::config::load_config(
        config_file.path().to_str().unwrap(),
    )
    .unwrap();

    // Create a chat with the systemPrompt:warhammer tag
    let chat_result = client
        .create_chat(CreateChatParams {
            title: Some("Test Chat".to_string()),
            tags: vec!["systemPrompt:warhammer".to_string()],
        })
        .await
        .expect("Failed to create chat");

    let chat_id = chat_result.chat_id;

    // Get chat state
    let chat = client
        .get_chat(GetChatParams {
            chat_id,
            if_version_higher_than: None,
        })
        .await
        .expect("Failed to get chat");

    // Process the chat to inject system prompt
    let chat_state = rhd_chat_client::ChatState {
        chat_id,
        messages: chat.messages,
        queued_messages_count: chat.queued_messages_count,
        tags: chat.chat.tags,
        version: chat.chat.version,
    };

    let injected = rhd_plugin_system_prompt::system_prompt::process_chat(
        &client,
        &chat_state,
        &cached_prompts,
    )
    .await
    .expect("Failed to process chat");

    assert_eq!(injected, 1);

    // Verify the system prompt was added
    let updated_chat = client
        .get_chat(GetChatParams {
            chat_id,
            if_version_higher_than: None,
        })
        .await
        .expect("Failed to get updated chat");

    let system_messages: Vec<_> = updated_chat
        .messages
        .iter()
        .filter(|m| m.role == "system")
        .collect();

    assert_eq!(system_messages.len(), 1);
    assert_eq!(system_messages[0].content, "Warhammer 40k system prompt");
    assert!(system_messages[0].tags.contains(&"systemPrompt:warhammer".to_string()));
}

#[tokio::test]
async fn test_duplicate_prevention() {
    let (_server, client, _server_url) = setup_test_server().await;
    let (config_file, _prompt_files) = create_test_config(&[
        ("warhammer", "Warhammer 40k system prompt"),
    ]);

    let (_config, cached_prompts) = rhd_plugin_system_prompt::config::load_config(
        config_file.path().to_str().unwrap(),
    )
    .unwrap();

    // Create a chat with the tag
    let chat_result = client
        .create_chat(CreateChatParams {
            title: Some("Test Chat".to_string()),
            tags: vec!["systemPrompt:warhammer".to_string()],
        })
        .await
        .expect("Failed to create chat");

    let chat_id = chat_result.chat_id;

    // Get initial chat state
    let chat = client
        .get_chat(GetChatParams {
            chat_id,
            if_version_higher_than: None,
        })
        .await
        .expect("Failed to get chat");

    let chat_state = rhd_chat_client::ChatState {
        chat_id,
        messages: chat.messages,
        queued_messages_count: chat.queued_messages_count,
        tags: chat.chat.tags,
        version: chat.chat.version,
    };

    // First injection
    let injected1 = rhd_plugin_system_prompt::system_prompt::process_chat(
        &client,
        &chat_state,
        &cached_prompts,
    )
    .await
    .expect("Failed to process chat");

    assert_eq!(injected1, 1);

    // Get updated chat state
    let updated_chat = client
        .get_chat(GetChatParams {
            chat_id,
            if_version_higher_than: None,
        })
        .await
        .expect("Failed to get updated chat");

    let updated_state = rhd_chat_client::ChatState {
        chat_id,
        messages: updated_chat.messages,
        queued_messages_count: updated_chat.queued_messages_count,
        tags: updated_chat.chat.tags,
        version: updated_chat.chat.version,
    };

    // Second injection should not add duplicate
    let injected2 = rhd_plugin_system_prompt::system_prompt::process_chat(
        &client,
        &updated_state,
        &cached_prompts,
    )
    .await
    .expect("Failed to process chat");

    assert_eq!(injected2, 0);
}

#[tokio::test]
async fn test_unfinished_message_blocks_injection() {
    let (_server, client, _server_url) = setup_test_server().await;
    let (config_file, _prompt_files) = create_test_config(&[
        ("warhammer", "Warhammer 40k system prompt"),
    ]);

    let (_config, cached_prompts) = rhd_plugin_system_prompt::config::load_config(
        config_file.path().to_str().unwrap(),
    )
    .unwrap();

    // Create a chat with the tag
    let chat_result = client
        .create_chat(CreateChatParams {
            title: Some("Test Chat".to_string()),
            tags: vec!["systemPrompt:warhammer".to_string()],
        })
        .await
        .expect("Failed to create chat");

    let chat_id = chat_result.chat_id;

    // Add an unfinished assistant message
    client
        .add_message(AddMessageParams {
            chat_id,
            role: "assistant".to_string(),
            content: "Partial response".to_string(),
            tool_call_id: None,
            reasoning_content: None,
            tags: vec![],
            is_finished: false,
            is_streaming: true,
        })
        .await
        .expect("Failed to add message");

    // Get chat state
    let chat = client
        .get_chat(GetChatParams {
            chat_id,
            if_version_higher_than: None,
        })
        .await
        .expect("Failed to get chat");

    let chat_state = rhd_chat_client::ChatState {
        chat_id,
        messages: chat.messages,
        queued_messages_count: chat.queued_messages_count,
        tags: chat.chat.tags,
        version: chat.chat.version,
    };

    // Process should not inject due to unfinished message
    let injected = rhd_plugin_system_prompt::system_prompt::process_chat(
        &client,
        &chat_state,
        &cached_prompts,
    )
    .await
    .expect("Failed to process chat");

    assert_eq!(injected, 0);
}

#[tokio::test]
async fn test_multiple_system_prompts_in_same_chat() {
    let (_server, client, _server_url) = setup_test_server().await;
    let (config_file, _prompt_files) = create_test_config(&[
        ("warhammer", "Warhammer 40k system prompt"),
        ("jokeTeller", "Joke teller system prompt"),
    ]);

    let (_config, cached_prompts) = rhd_plugin_system_prompt::config::load_config(
        config_file.path().to_str().unwrap(),
    )
    .unwrap();

    // Create a chat with both tags
    let chat_result = client
        .create_chat(CreateChatParams {
            title: Some("Test Chat".to_string()),
            tags: vec![
                "systemPrompt:warhammer".to_string(),
                "systemPrompt:jokeTeller".to_string(),
            ],
        })
        .await
        .expect("Failed to create chat");

    let chat_id = chat_result.chat_id;

    // Get chat state
    let chat = client
        .get_chat(GetChatParams {
            chat_id,
            if_version_higher_than: None,
        })
        .await
        .expect("Failed to get chat");

    let chat_state = rhd_chat_client::ChatState {
        chat_id,
        messages: chat.messages,
        queued_messages_count: chat.queued_messages_count,
        tags: chat.chat.tags,
        version: chat.chat.version,
    };

    // Process should inject both prompts
    let injected = rhd_plugin_system_prompt::system_prompt::process_chat(
        &client,
        &chat_state,
        &cached_prompts,
    )
    .await
    .expect("Failed to process chat");

    assert_eq!(injected, 2);

    // Verify both system prompts were added
    let updated_chat = client
        .get_chat(GetChatParams {
            chat_id,
            if_version_higher_than: None,
        })
        .await
        .expect("Failed to get updated chat");

    let system_messages: Vec<_> = updated_chat
        .messages
        .iter()
        .filter(|m| m.role == "system")
        .collect();

    assert_eq!(system_messages.len(), 2);
}
```

### 2. `plugins/rhd_plugin_system_prompt/README.md`

**Create new file:**

```markdown
# rhd_plugin_system_prompt

A plugin for the RHD chat system that automatically injects system prompts into chats based on chat tags.

## Overview

This plugin monitors chats for tags like `systemPrompt:warhammer` and adds corresponding system prompt messages from configured markdown files. It coordinates with the AI completions plugin to ensure system prompts are in place before AI requests proceed.

## Features

- **Automatic System Prompt Injection**: Automatically adds system prompts to chats based on tags
- **Multiple Prompts per Chat**: Supports multiple system prompts in a single chat
- **Duplicate Prevention**: Prevents adding the same system prompt twice
- **Race Condition Handling**: Safely handles chats with unfinished assistant messages
- **Event Coordination**: Coordinates with AI completions plugin via `ai_completions:preRequest` events

## Installation

Build the plugin:

```bash
cargo build --release -p rhd_plugin_system_prompt
```

The binary will be available at `target/release/rhd_plugin_system_prompt`.

## Configuration

Create a YAML configuration file:

```yaml
systemPrompts:
  warhammer: ./prompts/warhammer.md
  jokeTeller: ./prompts/jokes.md
```

### Configuration Options

- `systemPrompts`: A map of prompt names to file paths
  - Keys are prompt names (used in tags as `systemPrompt:<name>`)
  - Values are paths to markdown files containing the prompt content
  - Relative paths are resolved against the config file's directory
  - Absolute paths are used as-is

## Usage

Run the plugin:

```bash
./rhd_plugin_system_prompt \
  --server-url ws://localhost:8080/ \
  --config config.yaml \
  --plugin-id system_prompt
```

### Command-Line Arguments

- `--server-url`: WebSocket URL of the chat server (required)
- `--config`: Path to configuration file (required)
- `--plugin-id`: Plugin ID (optional, defaults to `system_prompt`)

## How It Works

1. **Startup**:
   - Loads configuration and validates all prompt files exist
   - Connects to the chat server and registers as a plugin
   - Subscribes to all chats
   - Performs startup reconciliation to inject prompts for existing chats

2. **Main Loop**:
   - Periodically checks all monitored chats for missing system prompts
   - Injects any missing prompts

3. **Event Coordination**:
   - Listens for `ai_completions:preRequest` events
   - If a chat needs a system prompt, injects it before acknowledging the event
   - Ensures system prompts are in place before AI requests proceed

## Tag Format

Chats should be tagged with `systemPrompt:<name>` where `<name>` matches a key in the configuration.

Example tags:
- `systemPrompt:warhammer`
- `systemPrompt:jokeTeller`

## System Prompt Messages

When a system prompt is injected, a message is added to the chat with:
- `role: "system"`
- `content: <markdown file content>`
- `tags: ["systemPrompt:<name>"]`
- `is_finished: true`
- `is_streaming: false`

## Error Handling

- **Missing Prompt Files**: Plugin fails to start if any configured prompt file is missing
- **Injection Errors**: Logged but don't stop the main loop
- **Event Acknowledgment**: Always acknowledges events, even if injection fails

## Troubleshooting

### Plugin fails to start with "failed to read prompt file"

Ensure all prompt files exist and are readable. Check the paths in your configuration file.

### System prompts not being injected

1. Verify the chat has the correct tag (e.g., `systemPrompt:warhammer`)
2. Check that the prompt name matches a key in the configuration
3. Ensure the chat doesn't have an unfinished assistant message

### AI requests proceeding without system prompts

Check the plugin logs for errors during event handling. The plugin should log when it receives `ai_completions:preRequest` events and when it injects system prompts.

## Development

### Running Tests

```bash
cargo test -p rhd_plugin_system_prompt
```

### Code Structure

- `src/main.rs`: Entry point with CLI argument parsing
- `src/lib.rs`: Module exports
- `src/config.rs`: Configuration loading and validation
- `src/plugin.rs`: Core plugin lifecycle and main loop
- `src/system_prompt.rs`: System prompt detection and injection logic

## License

This project is part of the RHD workspace.
```

## Implementation Notes

1. **Integration Test Pattern**: The integration tests follow the same pattern as `rhd_plugin_ai_completions/tests/integration_test.rs`, using the test server infrastructure.

2. **Test Coverage**: The tests cover:
   - Plugin startup with valid config
   - Plugin startup with missing prompt file (should fail)
   - System prompt injection on chat with tag
   - Duplicate prevention (don't add twice)
   - Unfinished message blocking injection
   - Multiple system prompts in same chat

3. **README Documentation**: The README provides:
   - Overview of the plugin's purpose
   - Installation instructions
   - Configuration format and options
   - Usage examples
   - How it works explanation
   - Troubleshooting tips

4. **Test Helpers**: The test file includes helper functions for:
   - Setting up test server and client
   - Creating test config files with prompt files

## Dependencies

- **Phase 3** - Requires complete plugin implementation including event coordination
- This is the final phase

## Success Criteria

- [ ] All unit tests pass (from Phase 1 and Phase 2)
- [ ] All integration tests pass
- [ ] README provides clear usage instructions
- [ ] Plugin builds successfully with `cargo build`
- [ ] Plugin can be run alongside `rhd_plugin_ai_completions`
- [ ] Workspace integration is complete (plugin is in workspace members)
