//! Assembling assistant messages from responses.
//!
//! Streaming (SSE) responses are stored as their verbatim concatenated bytes; this module
//! additionally merges the OpenAI chunk deltas back into a single assistant message for
//! easy inspection. Non-streaming JSON responses contribute `choices[0].message` directly.

use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

/// Assembles the assistant message from concatenated SSE bytes of a chat-completions
/// stream. Returns `None` when no chunk payload could be parsed.
///
/// Tolerates malformed chunks: unparsable `data:` payloads, chunks without `choices`,
/// and unknown delta fields are skipped. The merged message mirrors the API's own
/// message shape: `role`, `content` (null when only tool calls were streamed),
/// `tool_calls` (merged by index), and `finish_reason` when observed.
pub fn assemble_sse_message(raw: &[u8]) -> Option<Value> {
    let text = String::from_utf8_lossy(raw);
    let mut role: Option<String> = None;
    let mut content = String::new();
    let mut refusal = String::new();
    let mut tool_calls: BTreeMap<u64, ToolCallAccumulator> = BTreeMap::new();
    let mut finish_reason: Option<Value> = None;
    let mut parsed_any = false;

    for payload in sse_data_payloads(&text) {
        if payload == "[DONE]" {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(&payload) else {
            continue;
        };
        let Some(choice) = value
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first())
        else {
            continue;
        };
        parsed_any = true;
        if finish_reason.is_none() {
            if let Some(reason) = choice.get("finish_reason").filter(|r| !r.is_null()) {
                finish_reason = Some(reason.clone());
            }
        }
        let Some(delta) = choice.get("delta") else {
            continue;
        };
        if role.is_none() {
            if let Some(delta_role) = delta.get("role").and_then(Value::as_str) {
                role = Some(delta_role.to_string());
            }
        }
        if let Some(chunk) = delta.get("content").and_then(Value::as_str) {
            content.push_str(chunk);
        }
        if let Some(chunk) = delta.get("refusal").and_then(Value::as_str) {
            refusal.push_str(chunk);
        }
        if let Some(calls) = delta.get("tool_calls").and_then(Value::as_array) {
            for call in calls {
                merge_tool_call(&mut tool_calls, call);
            }
        }
    }

    if !parsed_any {
        return None;
    }

    let mut message = Map::new();
    message.insert("role".to_string(), json!(role.unwrap_or_else(|| "assistant".to_string())));
    if !refusal.is_empty() {
        message.insert("refusal".to_string(), json!(refusal));
    }
    let has_tool_calls = !tool_calls.is_empty();
    if content.is_empty() && has_tool_calls {
        message.insert("content".to_string(), Value::Null);
    } else {
        message.insert("content".to_string(), json!(content));
    }
    if has_tool_calls {
        let calls: Vec<Value> = tool_calls
            .into_values()
            .map(|accumulator| accumulator.into_value())
            .collect();
        message.insert("tool_calls".to_string(), json!(calls));
    }
    if let Some(reason) = finish_reason {
        message.insert("finish_reason".to_string(), reason);
    }
    Some(Value::Object(message))
}

/// Merges one `delta.tool_calls` fragment into the accumulators, keyed by `index`.
fn merge_tool_call(tool_calls: &mut BTreeMap<u64, ToolCallAccumulator>, call: &Value) {
    let index = call.get("index").and_then(Value::as_u64).unwrap_or(0);
    let accumulator = tool_calls.entry(index).or_default();
    if let Some(id) = call.get("id").and_then(Value::as_str) {
        accumulator.id.get_or_insert_with(|| id.to_string());
    }
    if let Some(kind) = call.get("type").and_then(Value::as_str) {
        accumulator.kind.get_or_insert_with(|| kind.to_string());
    }
    if let Some(function) = call.get("function").and_then(Value::as_object) {
        if let Some(name) = function.get("name").and_then(Value::as_str) {
            accumulator.name.get_or_insert_with(|| name.to_string());
        }
        if let Some(args) = function.get("arguments").and_then(Value::as_str) {
            accumulator.arguments.push_str(args);
        }
    }
}

#[derive(Default)]
struct ToolCallAccumulator {
    id: Option<String>,
    kind: Option<String>,
    name: Option<String>,
    arguments: String,
}

impl ToolCallAccumulator {
    fn into_value(self) -> Value {
        json!({
            "id": self.id,
            "type": self.kind.unwrap_or_else(|| "function".to_string()),
            "function": {
                "name": self.name,
                "arguments": self.arguments,
            }
        })
    }
}

/// Yields the joined `data:` payload of every SSE event in `text`.
///
/// Events are separated by blank lines; `data:` lines within one event are joined with
/// `\n` per the SSE spec; `data:` prefix and one optional leading space are stripped.
fn sse_data_payloads(text: &str) -> std::vec::IntoIter<String> {
    let normalized = text.replace("\r\n", "\n");
    normalized
        .split("\n\n")
        .filter_map(|event| {
            let data: Vec<&str> = event
                .lines()
                .filter_map(|line| line.strip_prefix("data:"))
                .map(|rest| rest.strip_prefix(' ').unwrap_or(rest))
                .collect();
            if data.is_empty() {
                None
            } else {
                Some(data.join("\n"))
            }
        })
        .collect::<Vec<_>>()
        .into_iter()
}

/// Extracts `choices[0].message` from a non-streaming chat-completions JSON response.
pub fn extract_message_json(raw: &[u8]) -> Option<Value> {
    let value: Value = serde_json::from_slice(raw).ok()?;
    value
        .get("choices")?
        .as_array()?
        .first()?
        .get("message")
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assembles_content_deltas_with_role_and_finish_reason() {
        let sse = concat!(
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\"},\"finish_reason\":null}]}\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hel\"}}]}\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"lo\"}}]}\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
            "data: [DONE]\n\n",
        );
        let message = assemble_sse_message(sse.as_bytes()).unwrap();
        assert_eq!(message["role"], "assistant");
        assert_eq!(message["content"], "Hello");
        assert_eq!(message["finish_reason"], "stop");
    }

    #[test]
    fn assembles_tool_calls_split_across_chunks() {
        let sse = concat!(
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"tool_calls\":[{\"index\":0,\"id\":\"call_1\",\"type\":\"function\",\"function\":{\"name\":\"get_weather\",\"arguments\":\"\"}}]}}]}\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"{\\\"city\\\":\"}}]}}]}\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":1,\"id\":\"call_2\",\"type\":\"function\",\"function\":{\"name\":\"get_time\",\"arguments\":\"{}\"}}]}}]}\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"function\":{\"arguments\":\"\\\"Oslo\\\"}\"}}]}}]}\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"tool_calls\"}]}\n\n",
            "data: [DONE]\n\n",
        );
        let message = assemble_sse_message(sse.as_bytes()).unwrap();
        assert_eq!(message["content"], Value::Null);
        let calls = message["tool_calls"].as_array().unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0]["id"], "call_1");
        assert_eq!(calls[0]["function"]["name"], "get_weather");
        assert_eq!(calls[0]["function"]["arguments"], r#"{"city":"Oslo"}"#);
        assert_eq!(calls[1]["id"], "call_2");
        assert_eq!(calls[1]["function"]["arguments"], "{}");
        assert_eq!(message["finish_reason"], "tool_calls");
    }

    #[test]
    fn skips_malformed_payloads_and_non_data_lines() {
        let sse = concat!(
            ": keep-alive comment\n\n",
            "event: ping\ndata: not json\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"ok\"}}]}\n\n",
            "data: {\"no_choices\": true}\n\n",
        );
        let message = assemble_sse_message(sse.as_bytes()).unwrap();
        assert_eq!(message["content"], "ok");
        assert!(message.get("finish_reason").is_none());
    }

    #[test]
    fn returns_none_for_unparseable_input() {
        assert!(assemble_sse_message(b"").is_none());
        assert!(assemble_sse_message(b"plain text body").is_none());
    }

    #[test]
    fn handles_crlf_line_endings_and_missing_trailing_blank_line() {
        let sse = "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"a\"}}]}\r\n\r\ndata: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"b\"}}]}";
        let message = assemble_sse_message(sse.as_bytes()).unwrap();
        assert_eq!(message["content"], "ab");
    }

    #[test]
    fn joins_multi_line_data_payloads() {
        let payloads: Vec<String> =
            sse_data_payloads("data: first\ndata: second\n\ndata: third\n\n").collect();
        assert_eq!(payloads, vec!["first\nsecond", "third"]);
    }

    #[test]
    fn extracts_message_from_non_stream_json() {
        let raw = br#"{"id":"1","choices":[{"index":0,"message":{"role":"assistant","content":"hi"},"finish_reason":"stop"}]}"#;
        let message = extract_message_json(raw).unwrap();
        assert_eq!(message["content"], "hi");
        assert_eq!(message["role"], "assistant");
    }

    #[test]
    fn extract_message_json_returns_none_for_bad_shapes() {
        assert!(extract_message_json(b"not json").is_none());
        assert!(extract_message_json(br#"{"choices":[]}"#).is_none());
        assert!(extract_message_json(br#"{"choices":[{"index":0}]}"#).is_none());
    }
}
