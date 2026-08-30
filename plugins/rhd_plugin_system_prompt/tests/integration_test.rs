//! Integration tests for the system prompt plugin.

use std::io::Write;
use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use rhd_chat_api::{AddMessageParams, CreateChatParams, GetChatParams, UpdateChatParams};
use rhd_chat_client::ChatClient;
use rhd_chat_server::config::Config;
use rhd_chat_server::connection::handle_connection;
use rhd_chat_server::plugins::new_shared_plugin_registry;
use rhd_chat_server::streams::StreamManager;
use rhd_chat_server::subscriptions::new_shared_subscription_manager;
use rhd_db::ChatDb;
use tempfile::NamedTempFile;

/// Start a test server and return the port.
async fn start_test_server() -> (u16, tokio::task::JoinHandle<()>) {
    // Find available port
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    // Create test config with in-memory database
    let config = Config {
        host: "127.0.0.1".to_string(),
        port,
        db_path: ":memory:".to_string(),
    };

    // Initialize database
    let db = Arc::new(ChatDb::new(&config.db_path).unwrap());

    // Create subscription manager
    let subscription_manager = new_shared_subscription_manager();

    // Create plugin registry
    let plugin_registry = new_shared_plugin_registry();

    // Create stream manager
    let stream_manager = Arc::new(StreamManager::new());

    // Bind TCP listener
    let listener = tokio::net::TcpListener::bind(&config.socket_addr())
        .await
        .unwrap();

    // Start server in background
    let handle = tokio::spawn(async move {
        loop {
            let (stream, _addr) = match listener.accept().await {
                Ok(result) => result,
                Err(_) => break,
            };

            let db = Arc::clone(&db);
            let subscription_manager = subscription_manager.clone();
            let plugin_registry = plugin_registry.clone();
            let stream_manager = stream_manager.clone();

            tokio::spawn(async move {
                match tokio_tungstenite::accept_async(stream).await {
                    Ok(ws_stream) => {
                        let (write, read) = ws_stream.split();
                        let _ = handle_connection(
                            read,
                            write,
                            db,
                            subscription_manager,
                            plugin_registry,
                            stream_manager,
                        )
                        .await;
                    }
                    Err(_) => {}
                }
            });
        }
    });

    // Wait for server to start
    tokio::time::sleep(Duration::from_millis(50)).await;

    (port, handle)
}

/// Connect a client to the test server.
async fn connect_client(port: u16) -> ChatClient {
    let url = format!("ws://127.0.0.1:{}/", port);
    ChatClient::connect(&url).await.unwrap()
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
    let (_port, _handle) = start_test_server().await;
    let (config_file, _prompt_files) = create_test_config(&[
        ("warhammer", "Warhammer 40k system prompt"),
        ("jokeTeller", "Joke teller system prompt"),
    ]);

    // Load config to verify it works
    let (config, cached) =
        rhd_plugin_system_prompt::config::load_config(config_file.path().to_str().unwrap())
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

    let result =
        rhd_plugin_system_prompt::config::load_config(config_file.path().to_str().unwrap());

    assert!(result.is_err());
}

#[tokio::test]
async fn test_system_prompt_injection_on_chat_with_tag() {
    let (port, _handle) = start_test_server().await;
    let client = connect_client(port).await;
    let (config_file, _prompt_files) =
        create_test_config(&[("warhammer", "Warhammer 40k system prompt")]);

    let (_config, cached_prompts) =
        rhd_plugin_system_prompt::config::load_config(config_file.path().to_str().unwrap())
            .unwrap();

    // Create a chat with the systemPrompt:warhammer tag
    let chat_result = client
        .create_chat(CreateChatParams {
            title: "Test Chat".to_string(),
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
    assert!(system_messages[0]
        .tags
        .contains(&"systemPrompt:warhammer".to_string()));
}

#[tokio::test]
async fn test_duplicate_prevention() {
    let (port, _handle) = start_test_server().await;
    let client = connect_client(port).await;
    let (config_file, _prompt_files) =
        create_test_config(&[("warhammer", "Warhammer 40k system prompt")]);

    let (_config, cached_prompts) =
        rhd_plugin_system_prompt::config::load_config(config_file.path().to_str().unwrap())
            .unwrap();

    // Create a chat with the tag
    let chat_result = client
        .create_chat(CreateChatParams {
            title: "Test Chat".to_string(),
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
    let (port, _handle) = start_test_server().await;
    let client = connect_client(port).await;
    let (config_file, _prompt_files) =
        create_test_config(&[("warhammer", "Warhammer 40k system prompt")]);

    let (_config, cached_prompts) =
        rhd_plugin_system_prompt::config::load_config(config_file.path().to_str().unwrap())
            .unwrap();

    // Create a chat with the tag
    let chat_result = client
        .create_chat(CreateChatParams {
            title: "Test Chat".to_string(),
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
    let (port, _handle) = start_test_server().await;
    let client = connect_client(port).await;
    let (config_file, _prompt_files) = create_test_config(&[
        ("warhammer", "Warhammer 40k system prompt"),
        ("jokeTeller", "Joke teller system prompt"),
    ]);

    let (_config, cached_prompts) =
        rhd_plugin_system_prompt::config::load_config(config_file.path().to_str().unwrap())
            .unwrap();

    // Create a chat with both tags
    let chat_result = client
        .create_chat(CreateChatParams {
            title: "Test Chat".to_string(),
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

/// Build a [`rhd_chat_client::ChatState`] from a fetched chat response.
fn build_chat_state(
    chat_id: i64,
    chat: &rhd_chat_api::GetChatResult,
) -> rhd_chat_client::ChatState {
    rhd_chat_client::ChatState {
        chat_id,
        messages: chat.messages.clone(),
        queued_messages_count: chat.queued_messages_count,
        tags: chat.chat.tags.clone(),
        version: chat.chat.version,
    }
}

/// Create a chat with the given tags on the test server.
async fn create_chat_with_tags(client: &ChatClient, tags: Vec<String>) -> i64 {
    let chat_result = client
        .create_chat(CreateChatParams {
            title: "Test Chat".to_string(),
            tags,
        })
        .await
        .expect("Failed to create chat");
    chat_result.chat_id
}

/// Fetch the current chat state from the server.
async fn fetch_chat_state(client: &ChatClient, chat_id: i64) -> rhd_chat_client::ChatState {
    let chat = client
        .get_chat(GetChatParams {
            chat_id,
            if_version_higher_than: None,
        })
        .await
        .expect("Failed to get chat");
    build_chat_state(chat_id, &chat)
}

/// Count system prompt messages with the given tag in a fetched chat.
async fn count_system_prompt_messages(
    client: &ChatClient,
    chat_id: i64,
    prompt_tag: &str,
) -> usize {
    let chat = client
        .get_chat(GetChatParams {
            chat_id,
            if_version_higher_than: None,
        })
        .await
        .expect("Failed to get chat");
    chat.messages
        .iter()
        .filter(|m| m.tags.iter().any(|t| t == prompt_tag))
        .count()
}

#[tokio::test]
async fn test_running_tag_blocks_injection() {
    let (port, _handle) = start_test_server().await;
    let client = connect_client(port).await;
    let (config_file, _prompt_files) =
        create_test_config(&[("warhammer", "Warhammer 40k system prompt")]);

    let (_config, cached_prompts) =
        rhd_plugin_system_prompt::config::load_config(config_file.path().to_str().unwrap())
            .unwrap();

    let chat_id = create_chat_with_tags(
        &client,
        vec![
            "systemPrompt:warhammer".to_string(),
            "ai_completions:running".to_string(),
        ],
    )
    .await;

    let chat_state = fetch_chat_state(&client, chat_id).await;

    let injected = rhd_plugin_system_prompt::system_prompt::process_chat(
        &client,
        &chat_state,
        &cached_prompts,
    )
    .await
    .expect("Failed to process chat");

    assert_eq!(injected, 0, "must not inject while running tag is present");
    assert_eq!(
        count_system_prompt_messages(&client, chat_id, "systemPrompt:warhammer").await,
        0,
        "no system prompt message should have been added"
    );
}

#[tokio::test]
async fn test_error_tag_blocks_injection() {
    let (port, _handle) = start_test_server().await;
    let client = connect_client(port).await;
    let (config_file, _prompt_files) =
        create_test_config(&[("warhammer", "Warhammer 40k system prompt")]);

    let (_config, cached_prompts) =
        rhd_plugin_system_prompt::config::load_config(config_file.path().to_str().unwrap())
            .unwrap();

    let chat_id = create_chat_with_tags(
        &client,
        vec![
            "systemPrompt:warhammer".to_string(),
            "ai_completions:error".to_string(),
        ],
    )
    .await;

    let chat_state = fetch_chat_state(&client, chat_id).await;

    let injected = rhd_plugin_system_prompt::system_prompt::process_chat(
        &client,
        &chat_state,
        &cached_prompts,
    )
    .await
    .expect("Failed to process chat");

    assert_eq!(injected, 0, "must not inject while error tag is present");
    assert_eq!(
        count_system_prompt_messages(&client, chat_id, "systemPrompt:warhammer").await,
        0,
        "no system prompt message should have been added"
    );
}

#[tokio::test]
async fn test_injection_resumes_after_running_tag_removed() {
    let (port, _handle) = start_test_server().await;
    let client = connect_client(port).await;
    let (config_file, _prompt_files) =
        create_test_config(&[("warhammer", "Warhammer 40k system prompt")]);

    let (_config, cached_prompts) =
        rhd_plugin_system_prompt::config::load_config(config_file.path().to_str().unwrap())
            .unwrap();

    let chat_id = create_chat_with_tags(
        &client,
        vec![
            "systemPrompt:warhammer".to_string(),
            "ai_completions:running".to_string(),
        ],
    )
    .await;

    // Locked: no injection while running tag is present
    let locked_state = fetch_chat_state(&client, chat_id).await;
    let injected = rhd_plugin_system_prompt::system_prompt::process_chat(
        &client,
        &locked_state,
        &cached_prompts,
    )
    .await
    .expect("Failed to process chat");
    assert_eq!(injected, 0);

    // Operator/AI plugin removes the running tag → chat is idle again
    client
        .update_chat(UpdateChatParams {
            chat_id,
            title: None,
            add_tags: vec![],
            remove_tags: vec!["ai_completions:running".to_string()],
        })
        .await
        .expect("Failed to remove running tag");

    let idle_state = fetch_chat_state(&client, chat_id).await;
    let injected = rhd_plugin_system_prompt::system_prompt::process_chat(
        &client,
        &idle_state,
        &cached_prompts,
    )
    .await
    .expect("Failed to process chat");

    assert_eq!(injected, 1, "should inject once running tag is removed");
    assert_eq!(
        count_system_prompt_messages(&client, chat_id, "systemPrompt:warhammer").await,
        1,
        "system prompt message should have been added"
    );
}
