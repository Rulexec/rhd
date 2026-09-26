//! Chat identity for a stateless API: canonical message forms and rolling prefix hashes.
//!
//! The OpenAI-compatible completions API carries no chat identifier — every request
//! contains the full `messages` history. Continuations are detected by hashing prefixes
//! of that history: `h_i = blake3(h_{i-1} || canonical(messages[i]))`. A request whose
//! history starts with a previously registered prefix continues that chat; the longest
//! matching prefix wins.

use serde_json::Value;

/// Seed for the hash chain, so the empty prefix hash is fixed and distinct from any
/// message-derived hash.
const CHAIN_SEED: &[u8] = b"rhd_ai_proxy:chat-chain:v1";

/// Maximum characters kept in a chat title.
pub const CHAT_TITLE_MAX_CHARS: usize = 100;

/// A loggable chat-completions request extracted from a raw body.
#[derive(Debug)]
pub struct ChatCandidate {
    pub messages: Vec<Value>,
    pub model: Option<String>,
    pub stream: bool,
}

/// Extracts the logging candidate from a completions request body.
///
/// Returns `None` unless the body is a JSON object with a non-empty `messages` array —
/// non-completions traffic is not logged at all.
pub fn extract_candidate(body: &[u8]) -> Option<ChatCandidate> {
    let value: Value = serde_json::from_slice(body).ok()?;
    let object = value.as_object()?;
    let messages = object.get("messages")?.as_array()?;
    if messages.is_empty() {
        return None;
    }
    Some(ChatCandidate {
        messages: messages.clone(),
        model: object
            .get("model")
            .and_then(Value::as_str)
            .map(str::to_string),
        stream: object
            .get("stream")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

/// Canonical serialization of one message.
///
/// The workspace `serde_json` keeps object keys sorted (`BTreeMap`), so serialization
/// is deterministic for identical semantic content.
fn canonical_message(message: &Value) -> String {
    serde_json::to_string(message).unwrap_or_default()
}

/// Computes the prefix hash chain for a message history.
///
/// Element `i` (0-based) hashes messages `0..=i`; `hashes[i]` of a longer history equals
/// `hashes[i]` of any history with the same first `i + 1` canonicalized messages.
pub fn prefix_hashes(messages: &[Value]) -> Vec<[u8; 32]> {
    let mut hashes = Vec::with_capacity(messages.len());
    let mut chain = blake3::hash(CHAIN_SEED);
    for message in messages {
        let mut hasher = blake3::Hasher::new();
        hasher.update(chain.as_bytes());
        hasher.update(canonical_message(message).as_bytes());
        chain = hasher.finalize();
        hashes.push(*chain.as_bytes());
    }
    hashes
}

/// Derives a short human-readable chat title from the first user message.
pub fn chat_title(messages: &[Value], max_chars: usize) -> String {
    let content = messages
        .iter()
        .find(|message| message.get("role").and_then(Value::as_str) == Some("user"))
        .map(|message| message.get("content").cloned().unwrap_or(Value::Null));
    let Some(content) = content else {
        return "<no user message>".to_string();
    };
    let text = squash_whitespace(&content_as_text(&content));
    if text.is_empty() {
        return "<empty user message>".to_string();
    }
    text.chars().take(max_chars).collect()
}

/// Extracts plain text from a message `content`: a string, or the concatenation of
/// `text` parts in the array-of-parts form. Non-text parts are ignored.
fn content_as_text(content: &Value) -> String {
    match content {
        Value::String(text) => text.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter(|part| {
                part.get("type").and_then(Value::as_str) == Some("text")
            })
            .filter_map(|part| part.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join(""),
        _ => String::new(),
    }
}

/// Collapses whitespace runs into single spaces and trims the ends.
fn squash_whitespace(text: &str) -> String {
    let mut squashed = String::with_capacity(text.len());
    let mut in_whitespace = false;
    for c in text.chars() {
        if c.is_whitespace() {
            if !in_whitespace {
                squashed.push(' ');
                in_whitespace = true;
            }
        } else {
            squashed.push(c);
            in_whitespace = false;
        }
    }
    squashed.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn prefix_hashes_are_deterministic_and_extend_stably() {
        let messages = vec![
            json!({"role": "system", "content": "sys"}),
            json!({"role": "user", "content": "hi"}),
        ];
        let short = prefix_hashes(&messages);
        let mut extended = messages.clone();
        extended.push(json!({"role": "assistant", "content": "hello"}));
        let long = prefix_hashes(&extended);

        assert_eq!(short, long[..short.len()]);
        assert_eq!(prefix_hashes(&messages), short);
        assert_ne!(long[2], short[1]);
    }

    #[test]
    fn edited_message_diverges_the_chain_from_that_point() {
        let original = prefix_hashes(&vec![
            json!({"role": "system", "content": "sys"}),
            json!({"role": "user", "content": "hi"}),
        ]);
        let edited = prefix_hashes(&vec![
            json!({"role": "system", "content": "different sys"}),
            json!({"role": "user", "content": "hi"}),
        ]);
        assert_ne!(original[0], edited[0]);
        assert_ne!(original[1], edited[1]);
    }

    #[test]
    fn key_order_in_messages_does_not_change_hashes() {
        // Same semantic message with keys written in different insertion order.
        let a = prefix_hashes(&vec![json!({"content": "hi", "role": "user"})]);
        let b = prefix_hashes(&vec![json!({"role": "user", "content": "hi"})]);
        assert_eq!(a, b);
    }

    #[test]
    fn extract_candidate_requires_object_with_messages() {
        assert!(extract_candidate(br#"not json"#).is_none());
        assert!(extract_candidate(b"[1,2,3]").is_none());
        assert!(extract_candidate(br#"{"model": "gpt-4o"}"#).is_none());
        assert!(extract_candidate(br#"{"messages": []}"#).is_none());
        assert!(extract_candidate(br#"{"messages": "no"}"#).is_none());

        let candidate =
            extract_candidate(br#"{"model": "gpt-4o", "stream": true, "messages": [{"role": "user", "content": "hi"}]}"#)
                .unwrap();
        assert_eq!(candidate.model.as_deref(), Some("gpt-4o"));
        assert!(candidate.stream);

        let candidate = extract_candidate(br#"{"messages": [{"role": "user", "content": "hi"}]}"#)
            .unwrap();
        assert_eq!(candidate.model, None);
        assert!(!candidate.stream);
    }

    #[test]
    fn chat_title_uses_first_user_message_string_content() {
        let messages = vec![
            json!({"role": "system", "content": "be brief"}),
            json!({"role": "user", "content": "  hello \n world  "}),
        ];
        assert_eq!(chat_title(&messages, 100), "hello world");
    }

    #[test]
    fn chat_title_joins_text_parts_and_truncates() {
        let messages = vec![json!({
            "role": "user",
            "content": [
                {"type": "text", "text": "part one "},
                {"type": "image_url", "image_url": {"url": "http://x"}},
                {"type": "text", "text": "part two"}
            ]
        })];
        assert_eq!(chat_title(&messages, 100), "part one part two");

        let long = vec![json!({"role": "user", "content": "x".repeat(300)})];
        let title = chat_title(&long, 100);
        assert_eq!(title.chars().count(), 100);
    }

    #[test]
    fn chat_title_falls_back_without_user_message() {
        let messages = vec![json!({"role": "system", "content": "sys"})];
        assert_eq!(chat_title(&messages, 100), "<no user message>");

        let empty = vec![json!({"role": "user", "content": "   "})];
        assert_eq!(chat_title(&empty, 100), "<empty user message>");
    }
}
