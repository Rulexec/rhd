use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ============================================================================
// Execution tracking types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionEvent {
    pub event: EventType,
    pub data: EventData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EventType {
    ScenarioStarted,
    StepStarted,
    ScenarioFinished,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum EventData {
    ScenarioStarted {
        id: u64,
        name: String,
        #[serde(rename = "daemonTime")]
        daemon_time: DateTime<Utc>,
        #[serde(rename = "startedAt")]
        started_at: DateTime<Utc>,
    },
    StepStarted {
        #[serde(rename = "executionId")]
        execution_id: u64,
        #[serde(rename = "stepName")]
        step_name: String,
        #[serde(rename = "startedAt")]
        started_at: DateTime<Utc>,
    },
    ScenarioFinished(ScenarioMeta),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum StepType {
    RunCommand,
    AiChat,
    Output,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepTiming {
    pub name: String,
    pub step_type: StepType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens: Option<TokenUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<f64>,
    pub sections: Vec<LogSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogSection {
    pub kind: LogSectionKind,
    pub start_line: u64,
    pub end_line: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LogSectionKind {
    RunningCommand,
    ExitCode,
    CommandOutput,
    AiRequest,
    SystemPrompt,
    Message,
    AiResponse,
    ToolCall,
    ToolResult,
    Skipped,
    OutputStep,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

impl TokenUsage {
    pub fn add(&mut self, other: &TokenUsage) {
        self.prompt_tokens += other.prompt_tokens;
        self.completion_tokens += other.completion_tokens;
        self.total_tokens += other.total_tokens;
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ScenarioStatus {
    Executing,
    Success,
    Error,
    Aborted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioMeta {
    pub id: u64,
    pub scenario: String,
    pub status: ScenarioStatus,
    pub started: DateTime<Utc>,
    pub finished: DateTime<Utc>,
    pub duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens: Option<TokenUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<f64>,
    pub steps: Vec<StepTiming>,
}

// ============================================================================
// WebSocket protocol types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type")]
pub enum WsRequest {
    #[serde(rename = "runScenario", rename_all = "camelCase")]
    RunScenario {
        id: String,
        name: String,
        cwd: String,
        #[serde(default)]
        model_aliases: Vec<(String, String)>,
    },
    #[serde(rename = "subscribe")]
    Subscribe { id: String },
    #[serde(rename = "getFinishedScenarios", rename_all = "camelCase")]
    GetFinishedScenarios { id: String, last_id: Option<u64> },
    #[serde(rename = "abortScenario")]
    AbortScenario { id: String, #[serde(rename = "executionId")] execution_id: u64 },

    // Chat operations
    #[serde(rename = "createChat")]
    CreateChat { id: String, title: String },
    #[serde(rename = "listChats")]
    ListChats { id: String },
    #[serde(rename = "getChat", rename_all = "camelCase")]
    GetChat { id: String, chat_id: i64 },
    #[serde(rename = "deleteChat", rename_all = "camelCase")]
    DeleteChat { id: String, chat_id: i64 },
    #[serde(rename = "sendMessage", rename_all = "camelCase")]
    SendMessage {
        id: String,
        chat_id: i64,
        content: String,
        model: String,
    },
    #[serde(rename = "editMessage", rename_all = "camelCase")]
    EditMessage {
        id: String,
        message_id: i64,
        content: String,
        model: String,
    },
    #[serde(rename = "abortChat", rename_all = "camelCase")]
    AbortChat { id: String, chat_id: i64 },
    #[serde(rename = "getAvailableModels")]
    GetAvailableModels { id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WsResponse {
    pub r#type: String,
    pub id: String,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<ErrorCode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl WsResponse {
    pub fn success(id: String, data: serde_json::Value) -> Self {
        Self {
            r#type: "response".to_string(),
            id,
            success: true,
            data: Some(data),
            error_code: None,
            error: None,
        }
    }

    pub fn error(id: String, error_code: ErrorCode, error: String) -> Self {
        Self {
            r#type: "response".to_string(),
            id,
            success: false,
            data: None,
            error_code: Some(error_code),
            error: Some(error),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WsEvent {
    pub r#type: String,
    pub event: String,
    pub data: serde_json::Value,
}

impl WsEvent {
    pub fn new(event: &str, data: serde_json::Value) -> Self {
        Self {
            r#type: "event".to_string(),
            event: event.to_string(),
            data,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    UnknownScenario,
    ScenarioExecutionFailed,
    ScenarioAborted,
    InvalidRequest,
    InternalError,
    ChatNotFound,
    MessageNotFound,
    ChatStreamFailed,
}

// ============================================================================
// Chat event payload types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatStreamChunkEvent {
    pub chat_id: i64,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatStreamFinishedEvent {
    pub chat_id: i64,
    pub message_id: i64,
    pub finish_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatStreamErrorEvent {
    pub chat_id: i64,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessageAddedEvent {
    pub chat_id: i64,
    pub message: ChatMessageDto,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessageDto {
    pub id: i64,
    pub chat_id: i64,
    pub role: String,
    pub content: String,
    pub created_at: String,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatUpdatedEvent {
    pub chat_id: i64,
    pub title: String,
}

// ============================================================================
// Token pricing types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenPriceTier {
    pub after_tokens: u64,
    pub input_token_price: f64,
    pub output_token_price: f64,
}

pub fn calculate_cost(
    usage: &TokenUsage,
    input_price: Option<f64>,
    output_price: Option<f64>,
    price_tiers: Option<&[TokenPriceTier]>,
) -> Option<f64> {
    if let Some(tiers) = price_tiers {
        if tiers.is_empty() {
            return calculate_flat_cost(usage, input_price, output_price);
        }

        let mut total_cost = 0.0;
        let mut remaining_prompt = usage.prompt_tokens;
        let mut remaining_completion = usage.completion_tokens;
        let mut prev_threshold = 0u64;

        let mut sorted_tiers: Vec<&TokenPriceTier> = tiers.iter().collect();
        sorted_tiers.sort_by_key(|t| t.after_tokens);

        for tier in sorted_tiers {
            let tier_capacity = tier.after_tokens - prev_threshold;

            let prompt_in_tier = remaining_prompt.min(tier_capacity);
            let completion_in_tier = remaining_completion.min(tier_capacity);

            total_cost += (prompt_in_tier as f64) * tier.input_token_price / 1_000_000.0;
            total_cost += (completion_in_tier as f64) * tier.output_token_price / 1_000_000.0;

            remaining_prompt = remaining_prompt.saturating_sub(prompt_in_tier);
            remaining_completion = remaining_completion.saturating_sub(completion_in_tier);
            prev_threshold = tier.after_tokens;

            if remaining_prompt == 0 && remaining_completion == 0 {
                break;
            }
        }

        if remaining_prompt > 0 || remaining_completion > 0 {
            if let Some(last_tier) = tiers.last() {
                total_cost += (remaining_prompt as f64) * last_tier.input_token_price / 1_000_000.0;
                total_cost +=
                    (remaining_completion as f64) * last_tier.output_token_price / 1_000_000.0;
            }
        }

        Some(total_cost)
    } else {
        calculate_flat_cost(usage, input_price, output_price)
    }
}

fn calculate_flat_cost(
    usage: &TokenUsage,
    input_price: Option<f64>,
    output_price: Option<f64>,
) -> Option<f64> {
    match (input_price, output_price) {
        (Some(input), Some(output)) => {
            let cost = (usage.prompt_tokens as f64) * input / 1_000_000.0
                + (usage.completion_tokens as f64) * output / 1_000_000.0;
            Some(cost)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ws_request_create_chat_serialization() {
        let req = WsRequest::CreateChat {
            id: "req-1".to_string(),
            title: "Test Chat".to_string(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains(r#""type":"createChat""#));
        assert!(json.contains(r#""title":"Test Chat""#));

        let deserialized: WsRequest = serde_json::from_str(&json).unwrap();
        match deserialized {
            WsRequest::CreateChat { id, title } => {
                assert_eq!(id, "req-1");
                assert_eq!(title, "Test Chat");
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_ws_request_send_message_serialization() {
        let req = WsRequest::SendMessage {
            id: "req-2".to_string(),
            chat_id: 42,
            content: "Hello".to_string(),
            model: "gpt-4".to_string(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains(r#""type":"sendMessage""#));
        assert!(json.contains(r#""chatId":42"#));

        let deserialized: WsRequest = serde_json::from_str(&json).unwrap();
        match deserialized {
            WsRequest::SendMessage { id, chat_id, content, model } => {
                assert_eq!(id, "req-2");
                assert_eq!(chat_id, 42);
                assert_eq!(content, "Hello");
                assert_eq!(model, "gpt-4");
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_ws_request_edit_message_serialization() {
        let req = WsRequest::EditMessage {
            id: "req-3".to_string(),
            message_id: 100,
            content: "Edited".to_string(),
            model: "gpt-4".to_string(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains(r#""type":"editMessage""#));
        assert!(json.contains(r#""messageId":100"#));
    }

    #[test]
    fn test_ws_request_abort_chat_serialization() {
        let req = WsRequest::AbortChat {
            id: "req-4".to_string(),
            chat_id: 5,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains(r#""type":"abortChat""#));

        let deserialized: WsRequest = serde_json::from_str(&json).unwrap();
        match deserialized {
            WsRequest::AbortChat { id, chat_id } => {
                assert_eq!(id, "req-4");
                assert_eq!(chat_id, 5);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_error_code_chat_variants_serialization() {
        let code = ErrorCode::ChatNotFound;
        let json = serde_json::to_string(&code).unwrap();
        assert_eq!(json, r#""CHAT_NOT_FOUND""#);

        let code = ErrorCode::MessageNotFound;
        let json = serde_json::to_string(&code).unwrap();
        assert_eq!(json, r#""MESSAGE_NOT_FOUND""#);

        let code = ErrorCode::ChatStreamFailed;
        let json = serde_json::to_string(&code).unwrap();
        assert_eq!(json, r#""CHAT_STREAM_FAILED""#);
    }

    #[test]
    fn test_chat_stream_chunk_event_serialization() {
        let event = ChatStreamChunkEvent {
            chat_id: 1,
            content: "Hello".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""chatId":1"#));
        assert!(json.contains(r#""content":"Hello""#));
    }

    #[test]
    fn test_chat_stream_finished_event_serialization() {
        let event = ChatStreamFinishedEvent {
            chat_id: 1,
            message_id: 42,
            finish_reason: "stop".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""chatId":1"#));
        assert!(json.contains(r#""messageId":42"#));
        assert!(json.contains(r#""finishReason":"stop""#));
    }

    #[test]
    fn test_chat_message_dto_serialization() {
        let msg = ChatMessageDto {
            id: 42,
            chat_id: 1,
            role: "assistant".to_string(),
            content: "Hello!".to_string(),
            created_at: "2026-06-28T15:00:00Z".to_string(),
            model: Some("gpt-4".to_string()),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""chatId":1"#));
        assert!(json.contains(r#""role":"assistant""#));
        assert!(json.contains(r#""createdAt":"2026-06-28T15:00:00Z""#));
        assert!(json.contains(r#""model":"gpt-4""#));
    }

    #[test]
    fn test_chat_message_added_event_serialization() {
        let event = ChatMessageAddedEvent {
            chat_id: 1,
            message: ChatMessageDto {
                id: 42,
                chat_id: 1,
                role: "user".to_string(),
                content: "Hi".to_string(),
                created_at: "2026-06-28T15:00:00Z".to_string(),
                model: Some("gpt-4".to_string()),
            },
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains(r#""chatId":1"#));
        assert!(json.contains(r#""role":"user""#));
        assert!(json.contains(r#""model":"gpt-4""#));
    }

    #[test]
    fn test_flat_cost_calculation() {
        let usage = TokenUsage {
            prompt_tokens: 1000,
            completion_tokens: 500,
            total_tokens: 1500,
        };

        let cost = calculate_cost(&usage, Some(10.0), Some(20.0), None).unwrap();
        assert!((cost - 0.02).abs() < 1e-10);
    }

    #[test]
    fn test_tiered_cost_calculation() {
        let usage = TokenUsage {
            prompt_tokens: 300_000,
            completion_tokens: 100_000,
            total_tokens: 400_000,
        };

        let tiers = vec![
            TokenPriceTier {
                after_tokens: 250_000,
                input_token_price: 5.0,
                output_token_price: 15.0,
            },
            TokenPriceTier {
                after_tokens: 500_000,
                input_token_price: 10.0,
                output_token_price: 30.0,
            },
        ];

        let cost = calculate_cost(&usage, None, None, Some(&tiers)).unwrap();

        let expected = (250_000.0 * 5.0 / 1_000_000.0)
            + (50_000.0 * 10.0 / 1_000_000.0)
            + (100_000.0 * 15.0 / 1_000_000.0);
        assert!((cost - expected).abs() < 1e-10);
    }

    #[test]
    fn test_no_pricing_returns_none() {
        let usage = TokenUsage {
            prompt_tokens: 1000,
            completion_tokens: 500,
            total_tokens: 1500,
        };

        assert!(calculate_cost(&usage, None, None, None).is_none());
    }
}
