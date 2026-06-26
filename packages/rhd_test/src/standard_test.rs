use std::sync::{Arc, Mutex};
use std::time::Duration;

use rand::rngs::StdRng;
use rand::SeedableRng;
use tokio::process::Command;

use crate::mock_server::{SharedRequests, SharedResponse};
use crate::utils::{create_temp_script, generate_random_string};

pub async fn run_single_test(
    iter_seed: u64,
    port: u16,
    requests: SharedRequests,
    response: SharedResponse,
) -> (bool, String) {
    let mut log = String::new();
    let mut rng = StdRng::seed_from_u64(iter_seed);

    let workspace_root = std::env::current_dir().unwrap();
    let rhd_bin = workspace_root.join("target/debug/rhd");
    let models_dir = workspace_root.join("test_e2e/models");
    let scenarios_dir = workspace_root.join("test_e2e/scenarios");

    let random_response = generate_random_string(&mut rng, 8);
    *response.lock().unwrap() = random_response.clone();
    log.push_str(&format!("  AI response: {random_response}\n"));

    let temp_dir = tempfile::tempdir().unwrap();
    create_temp_script(temp_dir.path(), &mut rng);
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
    let _ = std::fs::remove_file(&socket_path);

    let mut daemon = Command::new(&rhd_bin)
        .arg("daemon")
        .arg("--models-dir")
        .arg(&models_dir)
        .arg("--scenarios-dir")
        .arg(&scenarios_dir)
        .arg("--socket")
        .arg(&socket_path)
        .arg("--logs")
        .arg(&logs_dir)
        .env("E2E_MODEL_PORT", port.to_string())
        .env("E2E_SCRIPTS_DIR", temp_dir.path())
        .current_dir(daemon_dir.path())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("failed to spawn daemon");

    log.push_str(&format!("  Daemon spawned (PID: {:?})\n", daemon.id()));

    let mut stdout = daemon.stdout.take().expect("failed to take stdout");
    let mut stdout_buf = String::new();
    let mut found_listening = false;
    let start_time = std::time::Instant::now();
    let timeout = Duration::from_secs(10);

    while start_time.elapsed() < timeout {
        let mut line = String::new();
        match tokio::io::AsyncBufReadExt::read_line(
            &mut tokio::io::BufReader::new(&mut stdout),
            &mut line,
        )
        .await
        {
            Ok(0) => break,
            Ok(_) => {
                stdout_buf.push_str(&line);
                if line.contains("listening on") {
                    found_listening = true;
                    break;
                }
            }
            Err(_) => break,
        }
    }

    if !found_listening {
        log.push_str("  FAIL: Daemon did not print 'listening on' message\n");
        log.push_str(&format!("  stdout so far: {stdout_buf}\n"));
        daemon.kill().await.ok();
        return (true, log);
    }

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
        .arg("rhd_test")
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

    let daemon_stdout_content = daemon_stdout.lock().unwrap().clone();

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

    let recorded = requests.lock().unwrap();
    if recorded.len() != 1 {
        log.push_str(&format!(
            "  FAIL: expected 1 AI request, got {}\n",
            recorded.len()
        ));
        failed = true;
    } else {
        log.push_str("  PASS: exactly 1 AI request received\n");
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

        if req.system_content.contains('%') {
            log.push_str(&format!(
                "  FAIL: system prompt contains unresolved placeholders: {}\n",
                req.system_content
            ));
            failed = true;
        } else {
            log.push_str("  PASS: system prompt has no unresolved placeholders\n");
        }

        if req.user_content.contains('%') {
            log.push_str(&format!(
                "  FAIL: user message contains unresolved placeholders: {}\n",
                req.user_content
            ));
            failed = true;
        } else {
            log.push_str("  PASS: user message has no unresolved placeholders\n");
        }

        let expected_cwd = client_dir.path().to_string_lossy();
        if !req.user_content.contains(expected_cwd.as_ref()) {
            log.push_str(&format!(
                "  FAIL: user message does not contain client cwd '{}'\n",
                expected_cwd
            ));
            failed = true;
        } else {
            log.push_str("  PASS: user message contains client cwd\n");
        }

        let daemon_cwd = daemon_dir.path().to_string_lossy();
        if req.user_content.contains(daemon_cwd.as_ref()) && daemon_cwd != expected_cwd {
            log.push_str("  FAIL: user message contains daemon cwd instead of client cwd\n");
            failed = true;
        }

        log.push_str(&format!("  system: {}\n", req.system_content));
        log.push_str(&format!("  user: {}\n", req.user_content));
    }

    let log_entries: Vec<_> = match std::fs::read_dir(&logs_dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                name.starts_with("rhd_test-") && e.path().is_dir()
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

                    let expected_output = format!("AI said: {random_response}");
                    let expected_substrings: Vec<&str> = vec![
                        "===== rhd_test: executing scenario =====",
                        "===== cmd1: running command =====",
                        "[STDOUT]",
                        "----- cmd1: command exit code -----",
                        "----- cmd1: command output -----",
                        "===== ai1: AI request =====",
                        "model: test_model",
                        "----- system prompt -----",
                        "----- message -----",
                        "===== ai1: AI response =====",
                        &random_response,
                        "===== out1: output step =====",
                        &expected_output,
                    ];

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

                    if log_content != daemon_stdout_content {
                        log.push_str("  FAIL: log.txt content does not match daemon stdout\n");
                        log.push_str(&format!("    log.txt:\n{log_content}\n"));
                        log.push_str(&format!("    daemon stdout:\n{daemon_stdout_content}\n"));
                        failed = true;
                    } else {
                        log.push_str("  PASS: log.txt matches daemon stdout\n");
                    }
                }
                Err(err) => {
                    log.push_str(&format!("  FAIL: could not read log.txt: {err}\n"));
                    failed = true;
                }
            }
        }

        let meta_file = log_dir.join("meta.json");
        if !meta_file.exists() {
            log.push_str(&format!(
                "  FAIL: meta.json does not exist in {}\n",
                log_dir.display()
            ));
            failed = true;
        } else {
            log.push_str(&format!("  PASS: meta.json exists in {}\n", log_dir.display()));

            match std::fs::read_to_string(&meta_file) {
                Ok(meta_content) => {
                    match serde_json::from_str::<rhd_api::ScenarioMeta>(&meta_content) {
                        Ok(meta) => {
                            log.push_str(&format!("  PASS: meta.json is valid JSON\n"));

                            if meta.scenario != "rhd_test" {
                                log.push_str(&format!(
                                    "  FAIL: expected scenario 'rhd_test', got '{}'\n",
                                    meta.scenario
                                ));
                                failed = true;
                            } else {
                                log.push_str("  PASS: scenario name is 'rhd_test'\n");
                            }

                            if meta.duration_ms == 0 {
                                log.push_str("  FAIL: duration_ms is 0\n");
                                failed = true;
                            } else {
                                log.push_str(&format!("  PASS: duration_ms is {}\n", meta.duration_ms));
                            }

                            if meta.steps.len() != 3 {
                                log.push_str(&format!(
                                    "  FAIL: expected 3 steps, got {}\n",
                                    meta.steps.len()
                                ));
                                failed = true;
                            } else {
                                log.push_str("  PASS: exactly 3 steps in meta.json\n");

                                let step_names: Vec<&str> = meta.steps.iter().map(|s| s.name.as_str()).collect();
                                let expected_names = vec!["cmd1", "ai1", "out1"];
                                if step_names != expected_names {
                                    log.push_str(&format!(
                                        "  FAIL: expected step names {:?}, got {:?}\n",
                                        expected_names, step_names
                                    ));
                                    failed = true;
                                } else {
                                    log.push_str("  PASS: step names are correct\n");
                                }

                                // Validate step types
                                use rhd_api::StepType;
                                let expected_types = vec![StepType::RunCommand, StepType::AiChat, StepType::Output];
                                let actual_types: Vec<StepType> = meta.steps.iter().map(|s| s.step_type).collect();
                                if actual_types != expected_types {
                                    log.push_str(&format!(
                                        "  FAIL: expected step types {:?}, got {:?}\n",
                                        expected_types, actual_types
                                    ));
                                    failed = true;
                                } else {
                                    log.push_str("  PASS: step types are correct\n");
                                }

                                // Validate exit_code for runCommand step
                                let cmd_step = &meta.steps[0];
                                if cmd_step.exit_code.is_none() {
                                    log.push_str("  FAIL: cmd1 step has no exit_code\n");
                                    failed = true;
                                } else {
                                    log.push_str(&format!(
                                        "  PASS: cmd1 step has exit_code: {}\n",
                                        cmd_step.exit_code.unwrap()
                                    ));
                                }

                                // Validate model for aiChat step
                                let ai_step = &meta.steps[1];
                                if ai_step.model.is_none() {
                                    log.push_str("  FAIL: ai1 step has no model\n");
                                    failed = true;
                                } else {
                                    log.push_str(&format!(
                                        "  PASS: ai1 step has model: {}\n",
                                        ai_step.model.as_ref().unwrap()
                                    ));
                                }

                                let ai_step = &meta.steps[1];
                                if ai_step.tokens.is_none() {
                                    log.push_str("  FAIL: ai1 step has no token usage\n");
                                    failed = true;
                                } else {
                                    let tokens = ai_step.tokens.as_ref().unwrap();
                                    if tokens.total_tokens == 0 {
                                        log.push_str("  FAIL: ai1 step has 0 total tokens\n");
                                        failed = true;
                                    } else {
                                        log.push_str(&format!(
                                            "  PASS: ai1 step has token usage (total: {})\n",
                                            tokens.total_tokens
                                        ));
                                    }
                                }

                                for (i, step) in meta.steps.iter().enumerate() {
                                    // Output steps can legitimately complete in < 1ms
                                    if step.duration_ms == 0 && step.name != "out1" {
                                        log.push_str(&format!(
                                            "  FAIL: step {} '{}' has 0 duration_ms\n",
                                            i, step.name
                                        ));
                                        failed = true;
                                    }
                                    if step.sections.is_empty() {
                                        log.push_str(&format!(
                                            "  FAIL: step {} '{}' has no sections\n",
                                            i, step.name
                                        ));
                                        failed = true;
                                    }
                                }
                            }

                            if meta.tokens.is_none() {
                                log.push_str("  FAIL: scenario has no total token usage\n");
                                failed = true;
                            } else {
                                let tokens = meta.tokens.as_ref().unwrap();
                                log.push_str(&format!(
                                    "  PASS: scenario has total token usage (prompt: {}, completion: {}, total: {})\n",
                                    tokens.prompt_tokens, tokens.completion_tokens, tokens.total_tokens
                                ));
                            }
                        }
                        Err(err) => {
                            log.push_str(&format!(
                                "  FAIL: could not parse meta.json: {err}\n"
                            ));
                            log.push_str(&format!("    content: {meta_content}\n"));
                            failed = true;
                        }
                    }
                }
                Err(err) => {
                    log.push_str(&format!("  FAIL: could not read meta.json: {err}\n"));
                    failed = true;
                }
            }
        }
    }

    (failed, log)
}
