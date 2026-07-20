use std::sync::{Arc, Mutex};
use tokio::process::Command;

use super::setup::SetupResult;

pub struct ExecutionResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub failed: bool,
}

pub async fn run_scenario(
    setup: &SetupResult,
    random_response: &str,
    log: &mut String,
) -> (ExecutionResult, String) {
    let workspace_root = std::env::current_dir().unwrap();
    let rhd_bin = workspace_root.join("target/debug/rhd");

    let run_output = Command::new(&rhd_bin)
        .arg("run")
        .arg("rhd_test")
        .arg("--socket")
        .arg(&setup.socket_path)
        .current_dir(setup.client_dir.path())
        .output()
        .await
        .expect("failed to run scenario");

    let stdout = String::from_utf8_lossy(&run_output.stdout);
    let stderr = String::from_utf8_lossy(&run_output.stderr);
    let exit_code = run_output.status.code().unwrap_or(-1);

    log.push_str(&format!("  rhd run exit code: {exit_code}\n"));
    log.push_str(&format!("  rhd run stdout: {stdout}\n"));
    if !stderr.is_empty() {
        log.push_str(&format!("  rhd run stderr: {stderr}\n"));
    }

    let daemon_stdout_content = setup.daemon_stdout.lock().unwrap().clone();

    let mut failed = false;

    if exit_code != 0 {
        log.push_str(&format!(
            "  FAIL: expected exit code 0, got {exit_code}\n"
        ));
        failed = true;
    } else {
        log.push_str("  PASS: exit code is 0\n");
    }

    let expected_output = format!("AI said: {random_response}");
    if !stdout.contains(&expected_output) {
        log.push_str(&format!(
            "  FAIL: output does not contain '{expected_output}'\n"
        ));
        log.push_str(&format!("    actual output: {stdout}\n"));
        failed = true;
    } else {
        log.push_str(&format!(
            "  PASS: output contains '{expected_output}'\n"
        ));
    }

    (ExecutionResult {
        exit_code,
        stdout: stdout.to_string(),
        stderr: stderr.to_string(),
        failed,
    }, daemon_stdout_content)
}
