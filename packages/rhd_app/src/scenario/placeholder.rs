use std::collections::HashMap;

use crate::log::OutputLine;

#[derive(Debug, Clone)]
pub struct StepResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub stdout_stderr: Vec<OutputLine>,
    pub success: bool,
    pub message: Option<String>,
    pub cwd: Option<String>,
}

impl StepResult {
    pub fn stdout_stderr_combined(&self) -> String {
        let mut combined = String::new();
        for line in &self.stdout_stderr {
            combined.push_str(line.content());
        }
        combined
    }
}

#[derive(Debug, Clone, Default)]
pub struct ExecutionContext {
    pub steps: HashMap<String, StepResult>,
    pub flags: HashMap<String, bool>,
}

impl ExecutionContext {
    pub fn record_step(&mut self, name: String, result: StepResult) {
        self.steps.insert(name, result);
    }

    pub fn set_flag(&mut self, key: String, value: bool) {
        self.flags.insert(key, value);
    }

    pub fn get_flag(&self, key: &str) -> Option<bool> {
        self.flags.get(key).copied()
    }
}

pub fn resolve_placeholders(template: &str, context: &ExecutionContext) -> String {
    let mut output = String::with_capacity(template.len());
    let mut remaining = template;

    while let Some(start) = remaining.find('%') {
        let after_percent = &remaining[start + 1..];
        if let Some(end) = after_percent.find('%') {
            let placeholder = &after_percent[..end];
            output.push_str(&remaining[..start]);
            output.push_str(&resolve_single(placeholder, context));
            remaining = &after_percent[end + 1..];
        } else {
            output.push_str(remaining);
            return output;
        }
    }

    output.push_str(remaining);
    output
}

fn resolve_single(placeholder: &str, context: &ExecutionContext) -> String {
    let Some((step_name, field)) = placeholder.split_once('.') else {
        return String::new();
    };

    // Check if this is a flag placeholder (field starts with "flag_")
    if let Some(flag_name) = field.strip_prefix("flag_") {
        let full_key = format!("{}.flag_{}", step_name, flag_name);
        return context
            .flags
            .get(&full_key)
            .copied()
            .unwrap_or(false)
            .to_string();
    }

    let Some(result) = context.steps.get(step_name) else {
        return String::new();
    };

    match field {
        "exitCode" => result.exit_code.to_string(),
        "stdout" => result.stdout.clone(),
        "stderr" => result.stderr.clone(),
        "stdoutStderr" => result.stdout_stderr_combined(),
        "success" => result.success.to_string(),
        "message" => result.message.clone().unwrap_or_default(),
        "cwd" => result.cwd.clone().unwrap_or_default(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests;
