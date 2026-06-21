mod executor;
mod loader;
mod placeholder;

pub use executor::{execute_scenario, ExecuteError, ExecuteOutput};
pub use loader::{load_scenario, ScenarioLoadError};
pub use placeholder::{resolve_placeholders, ExecutionContext, StepResult};

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Action {
    RunCommand(RunCommandAction),
    AiChat(AiChatAction),
    Output(OutputAction),
}

impl Action {
    pub fn name(&self) -> Option<&str> {
        match self {
            Action::RunCommand(a) => a.name.as_deref(),
            Action::AiChat(a) => a.name.as_deref(),
            Action::Output(a) => a.name.as_deref(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunCommandAction {
    pub name: Option<String>,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub working_dir: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiChatAction {
    pub name: Option<String>,
    #[serde(default)]
    pub system_prompt: Option<String>,
    pub message: String,
    #[serde(default)]
    pub model: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputAction {
    pub name: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scenario {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub actions: Vec<Action>,
}
