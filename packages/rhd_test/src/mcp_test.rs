use std::sync::{Arc, Mutex};

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use tokio::process::Command;

use crate::mock_server::{SharedFlagValue, SharedRequests, SharedResponse};
use crate::utils::{generate_random_string, spawn_daemon};

pub async fn run_mcp_test(
    iter_seed: u64,
    port: u16,
    requests: SharedRequests,
    response: SharedResponse,
    flag_value: SharedFlagValue,
) -> (bool, String) {
    let mut log = String::new();
    let mut rng = StdRng::seed_from_u64(iter_seed);

    let workspace_root = std::env::current_dir().unwrap();
    let rhd_bin = workspace_root.join("target/debug/rhd");

    let random_response = generate_random_string(&mut rng, 8);
    *response.lock().unwrap() = random_response.clone();
    log.push_str(&format!("  AI response: {random_response}\n"));

    let expected_flag: bool = rng.gen();
    *flag_value.lock().unwrap() = expected_flag;
    log.push_str(&format!("  Expected flag value: {expected_flag}\n"));

    let temp_dir = tempfile::tempdir().unwrap();
    log.push_str(&format!(
        "  Temp scripts at: {}\n",
        temp_dir.path().display()
    ));

    let daemon_dir = tempfile::tempdir().unwrap();
    let client_dir = tempfile::tempdir().unwrap();
    let logs_dir = daemon_dir.path().join("logs");
    log.push_str(&format!("  Daemon cwd: {}\n", daemon_dir.path().display()));
    log.push_str(&format!("  Client cwd: {}\n", client_dir.path().display()));
    log.push_str(&format!("  Logs dir: {}\n", logs_dir.display()));

    let socket_path = daemon_dir.path().join("rhd.sock");

    let env_vars = vec![("E2E_MODEL_PORT", port.to_string())];

    let spawned = match spawn_daemon(
        daemon_dir.path(),
        &socket_path,
        &logs_dir,
        &env_vars,
        &[],
        None,
        10,
        "daemon listening",
    )
    .await
    {
        Ok(s) => s,
        Err(err) => {
            log.push_str("  FAIL: Daemon startup failed\n");
            log.push_str(&format!("  {err}\n"));
            return (true, log);
        }
    };

    let mut daemon = spawned.daemon;
    let stdout = spawned.stdout;
    let _stdout_buf = spawned.stdout_buf;
    log.push_str(&format!("  Daemon spawned (PID: {:?})\n", daemon.id()));

    let daemon_stdout: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
    let daemon_stdout_clone = daemon_stdout.clone();
    tokio::spawn(async move {
        use tokio::io::AsyncBufReadExt;
        let mut reader = tokio::io::BufReader::new(stdout);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    daemon_stdout_clone.lock().unwrap().push_str(&line);
                }
            }
        }
    });

    log.push_str("  Daemon ready (listening message received)\n");

    let run_output = Command::new(&rhd_bin)
        .arg("run")
        .arg("mcp_test")
        .arg("--socket")
        .arg(&socket_path)
        .current_dir(client_dir.path())
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

    daemon.kill().await.ok();
    let _ = std::fs::remove_file(&socket_path);

    let mut failed = false;

    if exit_code != 0 {
        log.push_str(&format!(
            "  FAIL: expected exit code 0, got {exit_code}\n"
        ));
        failed = true;
    } else {
        log.push_str("  PASS: exit code is 0\n");
    }

    let expected_output = format!("Flag was set: {}", expected_flag);
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

    let recorded = requests.lock().unwrap();
    if recorded.is_empty() {
        log.push_str("  FAIL: no AI requests received\n");
        failed = true;
    } else {
        log.push_str(&format!("  PASS: {} AI request(s) received\n", recorded.len()));

        let req = &recorded[0];
        if req.model != "test-model" {
            log.push_str(&format!(
                "  FAIL: expected model 'test-model', got '{}'\n",
                req.model
            ));
            failed = true;
        } else {
            log.push_str("  PASS: model is 'test-model'\n");
        }

        log.push_str(&format!("  system: {}\n", req.system_content));
        log.push_str(&format!("  user: {}\n", req.user_content));
    }

    let log_entries: Vec<_> = match std::fs::read_dir(&logs_dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                name.starts_with("mcp_test-") && e.path().is_dir()
            })
            .collect(),
        Err(err) => {
            log.push_str(&format!("  FAIL: could not read logs_dir: {err}\n"));
            failed = true;
            return (failed, log);
        }
    };

    if log_entries.len() != 1 {
        log.push_str(&format!(
            "  FAIL: expected 1 log directory, got {}\n",
            log_entries.len()
        ));
        failed = true;
    } else {
        log.push_str("  PASS: exactly 1 log directory created\n");
        let log_dir = log_entries[0].path();
        let log_file = log_dir.join("log.txt");

        if !log_file.exists() {
            log.push_str(&format!(
                "  FAIL: log.txt does not exist in {}\n",
                log_dir.display()
            ));
            failed = true;
        } else {
            log.push_str(&format!("  PASS: log.txt exists in {}\n", log_dir.display()));

            match std::fs::read_to_string(&log_file) {
                Ok(log_content) => {
                    log.push_str(&format!("  log.txt size: {} bytes\n", log_content.len()));

                    let expected_flag_str = format!("Flag was set: {}", expected_flag);
                    let mut expected_substrings: Vec<String> = vec![
                        "===== mcp_test: executing scenario =====".to_string(),
                        "===== ai1: AI request =====".to_string(),
                        "model: test_model".to_string(),
                        "----- system prompt -----".to_string(),
                        "----- message -----".to_string(),
                        "===== ai1: AI response =====".to_string(),
                        "===== out1: output step =====".to_string(),
                        expected_flag_str,
                    ];
                    if expected_flag {
                        expected_substrings.push("===== cmd1: skipped =====".to_string());
                    }

                    for expected in &expected_substrings {
                        if !log_content.contains(expected) {
                            log.push_str(&format!(
                                "  FAIL: log.txt does not contain '{expected}'\n"
                            ));
                            failed = true;
                        } else {
                            log.push_str(&format!(
                                "  PASS: log.txt contains '{expected}'\n"
                            ));
                        }
                    }
                }
                Err(err) => {
                    log.push_str(&format!("  FAIL: could not read log.txt: {err}\n"));
                    failed = true;
                }
            }
        }
    }

    (failed, log)
}
