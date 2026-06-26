use std::sync::Arc;

use super::placeholder::{resolve_placeholders, ExecutionContext, StepResult};
use super::RunCommandAction;
use crate::execution::ExecutionHandle;
use crate::log::{LogSink, OutputLine, SectionTracker};
use rhd_api::LogSectionKind;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;

pub async fn execute_run_command(
    cmd: &RunCommandAction,
    context: &ExecutionContext,
    sink: &mut LogSink,
    step_name: &str,
    client_cwd: &str,
    handle: Option<Arc<ExecutionHandle>>,
) -> StepResult {
    let resolved_command = resolve_placeholders(&cmd.command, context);
    let resolved_args: Vec<String> = cmd
        .args
        .iter()
        .map(|arg| resolve_placeholders(arg, context))
        .collect();

    let running_tracker = SectionTracker::start(sink, LogSectionKind::RunningCommand);
    sink.log_step(
        step_name,
        "running command",
        &format!("{} {}", resolved_command, resolved_args.join(" ")),
    );
    if let Some(h) = &handle {
        h.add_section(running_tracker.end(sink));
    }

    let mut command = tokio::process::Command::new(&resolved_command);
    command.args(&resolved_args);

    let resolved_cwd = match &cmd.working_dir {
        Some(dir) => resolve_placeholders(dir, context),
        None => client_cwd.to_string(),
    };
    command.current_dir(&resolved_cwd);

    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());

    match command.spawn() {
        Ok(mut child) => {
            let stdout = child.stdout.take().unwrap();
            let stderr = child.stderr.take().unwrap();

            let (tx, mut rx) = mpsc::channel::<OutputLine>(100);

            let tx_out = tx.clone();
            let stdout_task = tokio::spawn(async move {
                let mut reader = BufReader::new(stdout);
                let mut line = String::new();
                loop {
                    line.clear();
                    match reader.read_line(&mut line).await {
                        Ok(0) => break,
                        Ok(_) => {
                            let _ = tx_out.send(OutputLine::Stdout(line.clone())).await;
                        }
                        Err(_) => break,
                    }
                }
            });

            let tx_err = tx.clone();
            let stderr_task = tokio::spawn(async move {
                let mut reader = BufReader::new(stderr);
                let mut line = String::new();
                loop {
                    line.clear();
                    match reader.read_line(&mut line).await {
                        Ok(0) => break,
                        Ok(_) => {
                            let _ = tx_err.send(OutputLine::Stderr(line.clone())).await;
                        }
                        Err(_) => break,
                    }
                }
            });

            drop(tx);

            let mut output_lines = Vec::new();
            let mut stdout_buf = String::new();
            let mut stderr_buf = String::new();

            while let Some(output_line) = rx.recv().await {
                match &output_line {
                    OutputLine::Stdout(s) => stdout_buf.push_str(s),
                    OutputLine::Stderr(s) => stderr_buf.push_str(s),
                }
                output_lines.push(output_line);
            }

            let _ = tokio::join!(stdout_task, stderr_task);
            let status = child.wait().await;

            let (exit_code, success) = match status {
                Ok(s) => (s.code().unwrap_or(-1), s.success()),
                Err(_) => (-1, false),
            };

            let exit_tracker = SectionTracker::start(sink, LogSectionKind::ExitCode);
            sink.log_step_dashed(step_name, "command exit code", &exit_code.to_string());
            if let Some(h) = &handle {
                h.add_section(exit_tracker.end(sink));
                h.set_step_exit_code(exit_code);
            }

            let output_tracker = SectionTracker::start(sink, LogSectionKind::CommandOutput);
            sink.log_command_output(step_name, &output_lines);
            if let Some(h) = &handle {
                h.add_section(output_tracker.end(sink));
            }

            StepResult {
                exit_code,
                stdout: stdout_buf,
                stderr: stderr_buf,
                stdout_stderr: output_lines,
                success,
                message: None,
                cwd: Some(resolved_cwd),
            }
        }
        Err(err) => {
            sink.log_step(step_name, "command spawn error", &err.to_string());
            if let Some(h) = &handle {
                h.set_step_exit_code(-1);
            }
            StepResult {
                exit_code: -1,
                stdout: String::new(),
                stderr: err.to_string(),
                stdout_stderr: vec![OutputLine::Stderr(err.to_string())],
                success: false,
                message: None,
                cwd: Some(resolved_cwd),
            }
        },
    }
}
