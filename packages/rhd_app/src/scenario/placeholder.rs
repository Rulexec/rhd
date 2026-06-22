use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct StepResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub stdout_stderr: String,
    pub success: bool,
    pub message: Option<String>,
    pub cwd: Option<String>,
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
        "stdoutStderr" => {
            if result.success {
                result.stdout_stderr.clone()
            } else {
                String::new()
            }
        }
        "success" => result.success.to_string(),
        "message" => result.message.clone().unwrap_or_default(),
        "cwd" => result.cwd.clone().unwrap_or_default(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_exit_code() {
        let mut ctx = ExecutionContext::default();
        ctx.record_step(
            "testsRun".into(),
            StepResult {
                exit_code: 1,
                stdout: String::new(),
                stderr: String::new(),
                stdout_stderr: String::new(),
                success: false,
                message: None,
                cwd: None,
            },
        );
        assert_eq!(resolve_placeholders("code: %testsRun.exitCode%", &ctx), "code: 1");
    }

    #[test]
    fn missing_step_resolves_empty() {
        let ctx = ExecutionContext::default();
        assert_eq!(resolve_placeholders("val=%missing.exitCode%", &ctx), "val=");
    }

    #[test]
    fn stdout_stderr_empty_on_failure() {
        let mut ctx = ExecutionContext::default();
        ctx.record_step(
            "run".into(),
            StepResult {
                exit_code: 2,
                stdout: "out".into(),
                stderr: "err".into(),
                stdout_stderr: "out\nerr\n".into(),
                success: false,
                message: None,
                cwd: None,
            },
        );
        assert_eq!(resolve_placeholders("%run.stdoutStderr%", &ctx), "");
    }

    #[test]
    fn stdout_stderr_concatenated_on_success() {
        let mut ctx = ExecutionContext::default();
        ctx.record_step(
            "run".into(),
            StepResult {
                exit_code: 0,
                stdout: "out\n".into(),
                stderr: "err\n".into(),
                stdout_stderr: "out\nerr\n".into(),
                success: true,
                message: None,
                cwd: None,
            },
        );
        assert_eq!(resolve_placeholders("%run.stdoutStderr%", &ctx), "out\nerr\n");
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
