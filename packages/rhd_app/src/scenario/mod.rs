mod ai_chat;
mod error;
mod executor;
mod loader;
mod placeholder;
mod run_command;

pub use error::ExecuteError;
pub use executor::execute_scenario;
pub use executor::ExecutionConfig;
pub use loader::load_scenarios_dir;

use serde::Deserialize;
use std::collections::HashMap;

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
    #[serde(rename = "cmd")]
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default, rename = "cwd")]
    pub working_dir: Option<String>,
    #[serde(default)]
    pub skip: Option<String>,
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
    #[serde(default)]
    pub mcp: Option<Vec<McpRef>>,
    #[serde(default)]
    pub max_tool_iterations: Option<MaxIterations>,
    #[serde(default)]
    pub skip: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpRef {
    pub name: String,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub args: Option<Vec<String>>,
    #[serde(default)]
    pub env: Option<HashMap<String, String>>,
}

impl McpRef {
    pub fn effective_id(&self) -> &str {
        self.id.as_deref().unwrap_or(&self.name)
    }
}

#[derive(Debug, Clone)]
pub enum MaxIterations {
    Finite(u32),
    Infinite,
}

impl<'de> Deserialize<'de> for MaxIterations {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de;

        struct MaxIterationsVisitor;

        impl<'de> de::Visitor<'de> for MaxIterationsVisitor {
            type Value = MaxIterations;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a positive integer or the string \"inf\"")
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(MaxIterations::Finite(value as u32))
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if value == "inf" {
                    Ok(MaxIterations::Infinite)
                } else {
                    Err(de::Error::custom(format!(
                        "expected \"inf\", got \"{}\"",
                        value
                    )))
                }
            }
        }

        deserializer.deserialize_any(MaxIterationsVisitor)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputAction {
    pub name: Option<String>,
    #[serde(rename = "output")]
    pub text: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scenario {
    #[serde(default)]
    #[allow(dead_code)]
    pub description: Option<String>,
    pub actions: Vec<Action>,
}

impl Scenario {
    pub fn apply_env_vars(&mut self) {
        for action in &mut self.actions {
            match action {
                Action::RunCommand(cmd) => {
                    cmd.command = rhd_util::substitute_env_vars(&cmd.command);
                    cmd.args = cmd
                        .args
                        .iter()
                        .map(|a| rhd_util::substitute_env_vars(a))
                        .collect();
                    cmd.working_dir = cmd
                        .working_dir
                        .as_ref()
                        .map(|d| rhd_util::substitute_env_vars(d));
                }
                Action::AiChat(chat) => {
                    chat.system_prompt = chat
                        .system_prompt
                        .as_ref()
                        .map(|s| rhd_util::substitute_env_vars(s));
                    chat.message = rhd_util::substitute_env_vars(&chat.message);
                    chat.model = chat
                        .model
                        .as_ref()
                        .map(|m| rhd_util::substitute_env_vars(m));
                    
                    // Apply env vars to MCP config
                    if let Some(mcp_refs) = &mut chat.mcp {
                        for mcp_ref in mcp_refs {
                            if let Some(args) = &mut mcp_ref.args {
                                *args = args
                                    .iter()
                                    .map(|a| rhd_util::substitute_env_vars(a))
                                    .collect();
                            }
                            if let Some(env) = &mut mcp_ref.env {
                                *env = env
                                    .iter()
                                    .map(|(k, v)| (k.clone(), rhd_util::substitute_env_vars(v)))
                                    .collect();
                            }
                        }
                    }
                }
                Action::Output(output) => {
                    output.text = rhd_util::substitute_env_vars(&output.text);
                }
            }
        }
    }
}
