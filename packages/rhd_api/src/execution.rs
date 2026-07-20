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
    ScenarioResumed,
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
    ScenarioResumed {
        #[serde(rename = "executionId")]
        execution_id: u64,
    },
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
