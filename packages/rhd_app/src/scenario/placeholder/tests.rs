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
