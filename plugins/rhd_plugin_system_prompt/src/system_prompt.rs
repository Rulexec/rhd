//! System prompt detection and injection logic.

use rhd_chat_api::AddMessageParams;
use rhd_chat_client::{ChatClient, ChatState};

use crate::config::CachedPrompt;

/// Tag prefix for system prompt tags.
pub const SYSTEM_PROMPT_TAG_PREFIX: &str = "systemPrompt:";

/// Chat tag set by the AI completions plugin while a request is in flight
/// (including tool-loop iterations). While present, system prompt injection
/// must not touch the chat.
pub const AI_COMPLETIONS_RUNNING_TAG: &str = "ai_completions:running";

/// Chat tag set by the AI completions plugin when the chat is parked after a
/// failure. While present, system prompt injection must not touch the chat.
pub const AI_COMPLETIONS_ERROR_TAG: &str = "ai_completions:error";

/// Check whether a chat is locked by the AI completions plugin.
///
/// A chat is locked while it carries `ai_completions:running` (request
/// actively in progress) or `ai_completions:error` (chat parked after a
/// failure). No system prompts may be injected until the tag is removed.
pub fn has_blocking_tag(chat_state: &ChatState) -> bool {
    chat_state
        .tags
        .iter()
        .any(|tag| tag == AI_COMPLETIONS_RUNNING_TAG || tag == AI_COMPLETIONS_ERROR_TAG)
}

/// Check if a chat has a system prompt message for the given prompt name.
///
/// A system prompt message is identified by having a tag matching `systemPrompt:<name>`.
pub fn has_system_prompt_message(chat_state: &ChatState, prompt_name: &str) -> bool {
    let expected_tag = format!("{}{}", SYSTEM_PROMPT_TAG_PREFIX, prompt_name);
    chat_state
        .messages
        .iter()
        .any(|msg| msg.tags.iter().any(|tag| tag == &expected_tag))
}

/// Check if a chat has an unfinished assistant message.
///
/// An unfinished message is one where:
/// - `role == "assistant"` AND
/// - (`is_streaming == true` OR `is_finished == false`)
pub fn has_unfinished_assistant_message(chat_state: &ChatState) -> bool {
    chat_state
        .messages
        .iter()
        .any(|msg| msg.role == "assistant" && (msg.is_streaming || !msg.is_finished))
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
/// 1. Checks if the chat is locked by the AI completions plugin
///    (`ai_completions:running` or `ai_completions:error` tag; if so, skip)
/// 2. Checks if the chat has an unfinished assistant message (if so, skip)
/// 3. Gets the list of required prompt names from chat tags
/// 4. For each required prompt, checks if it already exists
/// 5. Injects any missing prompts
///
/// Returns the number of prompts injected.
pub async fn process_chat(
    client: &ChatClient,
    chat_state: &ChatState,
    cached_prompts: &[CachedPrompt],
) -> Result<usize, InjectError> {
    // Skip while the chat is locked by the AI completions plugin
    // (request in flight, or chat parked with error).
    if has_blocking_tag(chat_state) {
        tracing::debug!(
            chat_id = chat_state.chat_id,
            "skipping chat with ai_completions running/error tag"
        );
        return Ok(0);
    }

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
        assert_eq!(
            parse_system_prompt_tag("systemPrompt:warhammer"),
            Some("warhammer")
        );
        assert_eq!(
            parse_system_prompt_tag("systemPrompt:jokeTeller"),
            Some("jokeTeller")
        );
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

    #[test]
    fn test_has_blocking_tag_running() {
        let state = create_chat_state(vec![], vec!["ai_completions:running".to_string()]);
        assert!(has_blocking_tag(&state));
    }

    #[test]
    fn test_has_blocking_tag_error() {
        let state = create_chat_state(vec![], vec!["ai_completions:error".to_string()]);
        assert!(has_blocking_tag(&state));
    }

    #[test]
    fn test_has_blocking_tag_both() {
        let state = create_chat_state(
            vec![],
            vec![
                "ai_completions:running".to_string(),
                "ai_completions:error".to_string(),
            ],
        );
        assert!(has_blocking_tag(&state));
    }

    #[test]
    fn test_has_blocking_tag_none() {
        let state = create_chat_state(
            vec![],
            vec!["systemPrompt:warhammer".to_string(), "other".to_string()],
        );
        assert!(!has_blocking_tag(&state));
    }

    #[test]
    fn test_has_blocking_tag_empty() {
        let state = create_chat_state(vec![], vec![]);
        assert!(!has_blocking_tag(&state));
    }
}
