use std::sync::{Arc, Mutex};

use crate::mock_server::SharedRequests;
use super::setup::SetupResult;
use super::execution::ExecutionResult;

pub fn validate_results(
    setup: &SetupResult,
    execution_result: &ExecutionResult,
    daemon_stdout_content: &str,
    random_response: &str,
    requests: SharedRequests,
    log: &mut String,
) {
    validate_requests(&requests, &setup.client_dir, &setup.daemon_dir, random_response, log);
    validate_logs(&setup.logs_dir, daemon_stdout_content, random_response, log);
}

fn validate_requests(
    requests: &SharedRequests,
    client_dir: &tempfile::TempDir,
    daemon_dir: &tempfile::TempDir,
    random_response: &str,
    log: &mut String,
) {
    let recorded = requests.lock().unwrap();
    if recorded.len() != 1 {
        log.push_str(&format!(
            "  FAIL: expected 1 AI request, got {}\n",
            recorded.len()
        ));
    } else {
        log.push_str("  PASS: exactly 1 AI request received\n");
        let req = &recorded[0];

        if req.model != "test-model" {
            log.push_str(&format!(
                "  FAIL: expected model 'test-model', got '{}'\n",
                req.model
            ));
        } else {
            log.push_str("  PASS: model is 'test-model'\n");
        }

        if req.system_content.contains('%') {
            log.push_str(&format!(
                "  FAIL: system prompt contains unresolved placeholders: {}\n",
                req.system_content
            ));
        } else {
            log.push_str("  PASS: system prompt has no unresolved placeholders\n");
        }

        if req.user_content.contains('%') {
            log.push_str(&format!(
                "  FAIL: user message contains unresolved placeholders: {}\n",
                req.user_content
            ));
        } else {
            log.push_str("  PASS: user message has no unresolved placeholders\n");
        }

        let expected_cwd = client_dir.path().to_string_lossy();
        if !req.user_content.contains(expected_cwd.as_ref()) {
            log.push_str(&format!(
                "  FAIL: user message does not contain client cwd '{}'\n",
                expected_cwd
            ));
        } else {
            log.push_str("  PASS: user message contains client cwd\n");
        }

        let daemon_cwd = daemon_dir.path().to_string_lossy();
        if req.user_content.contains(daemon_cwd.as_ref()) && daemon_cwd != expected_cwd {
            log.push_str("  FAIL: user message contains daemon cwd instead of client cwd\n");
        }

        log.push_str(&format!("  system: {}\n", req.system_content));
        log.push_str(&format!("  user: {}\n", req.user_content));
    }
}

fn validate_logs(
    logs_dir: &std::path::Path,
    daemon_stdout_content: &str,
    random_response: &str,
    log: &mut String,
) {
    let log_entries: Vec<_> = match std::fs::read_dir(logs_dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                name.starts_with("rhd_test-") && e.path().is_dir()
            })
            .collect(),
        Err(err) => {
            log.push_str(&format!("  FAIL: could not read logs_dir: {err}\n"));
            return;
        }
    };

    if log_entries.len() != 1 {
        log.push_str(&format!(
            "  FAIL: expected 1 log directory, got {}\n",
            log_entries.len()
        ));
    } else {
        log.push_str("  PASS: exactly 1 log directory created\n");
        let log_dir = log_entries[0].path();
        let log_file = log_dir.join("log.txt");

        if !log_file.exists() {
            log.push_str(&format!(
                "  FAIL: log.txt does not exist in {}\n",
                log_dir.display()
            ));
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
                    } else {
                        log.push_str("  PASS: log.txt matches daemon stdout\n");
                    }
                }
                Err(err) => {
                    log.push_str(&format!("  FAIL: could not read log.txt: {err}\n"));
                }
            }
        }

        validate_meta_json(&log_dir, log);
    }
}

fn validate_meta_json(log_dir: &std::path::Path, log: &mut String) {
    let meta_file = log_dir.join("meta.json");
    if !meta_file.exists() {
        log.push_str(&format!(
            "  FAIL: meta.json does not exist in {}\n",
            log_dir.display()
        ));
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
                        } else {
                            log.push_str("  PASS: scenario name is 'rhd_test'\n");
                        }

                        if meta.duration_ms == 0 {
                            log.push_str("  FAIL: duration_ms is 0\n");
                        } else {
                            log.push_str(&format!("  PASS: duration_ms is {}\n", meta.duration_ms));
                        }

                        if meta.steps.len() != 3 {
                            log.push_str(&format!(
                                "  FAIL: expected 3 steps, got {}\n",
                                meta.steps.len()
                            ));
                        } else {
                            log.push_str("  PASS: exactly 3 steps in meta.json\n");

                            let step_names: Vec<&str> = meta.steps.iter().map(|s| s.name.as_str()).collect();
                            let expected_names = vec!["cmd1", "ai1", "out1"];
                            if step_names != expected_names {
                                log.push_str(&format!(
                                    "  FAIL: expected step names {:?}, got {:?}\n",
                                    expected_names, step_names
                                ));
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
                            } else {
                                log.push_str("  PASS: step types are correct\n");
                            }

                            // Validate exit_code for runCommand step
                            let cmd_step = &meta.steps[0];
                            if cmd_step.exit_code.is_none() {
                                log.push_str("  FAIL: cmd1 step has no exit_code\n");
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
                            } else {
                                log.push_str(&format!(
                                    "  PASS: ai1 step has model: {}\n",
                                    ai_step.model.as_ref().unwrap()
                                ));
                            }

                            let ai_step = &meta.steps[1];
                            if ai_step.tokens.is_none() {
                                log.push_str("  FAIL: ai1 step has no token usage\n");
                            } else {
                                let tokens = ai_step.tokens.as_ref().unwrap();
                                if tokens.total_tokens == 0 {
                                    log.push_str("  FAIL: ai1 step has 0 total tokens\n");
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
                                }
                                if step.sections.is_empty() {
                                    log.push_str(&format!(
                                        "  FAIL: step {} '{}' has no sections\n",
                                        i, step.name
                                    ));
                                }
                            }
                        }

                        if meta.tokens.is_none() {
                            log.push_str("  FAIL: scenario has no total token usage\n");
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
                    }
                }
            }
            Err(err) => {
                log.push_str(&format!("  FAIL: could not read meta.json: {err}\n"));
            }
        }
    }
}
