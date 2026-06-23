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
}

impl ExecutionContext {
    pub fn record_step(&mut self, name: String, result: StepResult) {
        self.steps.insert(name, result);
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
mod tests {
    use super::*;

    fn make_result(
        exit_code: i32,
        stdout: &str,
        stderr: &str,
        stdout_stderr: Vec<OutputLine>,
        success: bool,
    ) -> StepResult {
        StepResult {
            exit_code,
            stdout: stdout.into(),
            stderr: stderr.into(),
            stdout_stderr,
            success,
            message: None,
            cwd: None,
        }
    }

    #[test]
    fn resolves_exit_code() {
        let mut ctx = ExecutionContext::default();
        ctx.record_step(
            "testsRun".into(),
            make_result(1, "", "", vec![], false),
        );
        assert_eq!(resolve_placeholders("code: %testsRun.exitCode%", &ctx), "code: 1");
    }

    #[test]
    fn missing_step_resolves_empty() {
        let ctx = ExecutionContext::default();
        assert_eq!(resolve_placeholders("val=%missing.exitCode%", &ctx), "val=");
    }

    #[test]
    fn stdout_stderr_combined_from_lines() {
        let mut ctx = ExecutionContext::default();
        ctx.record_step(
            "run".into(),
            make_result(
                2,
                "out\n",
                "err\n",
                vec![
                    OutputLine::Stdout("out\n".into()),
                    OutputLine::Stderr("err\n".into()),
                ],
                false,
            ),
        );
        assert_eq!(resolve_placeholders("%run.stdoutStderr%", &ctx), "out\nerr\n");
    }

    #[test]
    fn stdout_stderr_empty_when_no_lines() {
        let mut ctx = ExecutionContext::default();
        ctx.record_step("run".into(), make_result(0, "", "", vec![], true));
        assert_eq!(resolve_placeholders("%run.stdoutStderr%", &ctx), "");
    }

    #[test]
    fn no_placeholder_passthrough() {
        let ctx = ExecutionContext::default();
        assert_eq!(resolve_placeholders("hello world", &ctx), "hello world");
    }

    #[test]
    fn unclosed_percent_passthrough() {
        let ctx = ExecutionContext::default();
        assert_eq!(resolve_placeholders("abc%def", &ctx), "abc%def");
    }
}
