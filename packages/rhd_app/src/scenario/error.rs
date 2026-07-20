use thiserror::Error;


#[derive(Debug, Error)]
pub enum ExecuteError {
    #[error("scenario '{scenario}' step '{step}': unknown model '{model}'")]
    UnknownModel {
        scenario: String,
        step: String,
        model: String,
    },

    #[error("scenario '{scenario}' step '{step}': no default model configured")]
    NoDefaultModel { scenario: String, step: String },

    #[error("scenario '{scenario}' step '{step}': AI request failed: {message}")]
    AiFailed {
        scenario: String,
        step: String,
        message: String,
    },

    #[error("scenario aborted")]
    Aborted,
}

impl From<rhd_ai::AiError> for ExecuteError {
    fn from(err: rhd_ai::AiError) -> Self {
        ExecuteError::AiFailed {
            scenario: String::new(),
            step: String::new(),
            message: err.to_string(),
        }
    }
}

#[derive(Debug)]
pub struct ExecuteOutput {
    pub outputs: Vec<String>,
}
