use thiserror::Error;

#[derive(Debug, Error)]
pub enum AiClientError {
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("API error: status {status}, body: {body}")]
    Api { status: u16, body: String },

    #[error("failed to parse response: {0}")]
    Parse(#[from] serde_json::Error),

    #[error("no choices in response")]
    NoChoices,

    #[error("streaming error: {0}")]
    Stream(String),
}
