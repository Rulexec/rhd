use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::{extract::State, routing::post, Json, Router};
use clap::Parser;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tokio::process::Command;

#[derive(Parser)]
#[command(name = "rhd_test", about = "E2E test runner for rhd")]
struct Args {
    /// Random seed for deterministic test generation
    #[arg(long, default_value_t = 42)]
    seed: u64,

    /// Number of test repetitions
    #[arg(long, default_value_t = 10)]
    repetitions: u32,
}

#[derive(Debug, Deserialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Serialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Debug, Serialize)]
struct ResponseMessage {
    role: String,
    content: String,
}

#[derive(Debug, Clone)]
struct RecordedRequest {
    model: String,
    system_content: String,
    user_content: String,
}

type SharedRequests = Arc<Mutex<Vec<RecordedRequest>>>;
type SharedResponse = Arc<Mutex<String>>;

async fn chat_completions(
    State((requests, response)): State<(SharedRequests, SharedResponse)>,
    Json(body): Json<ChatRequest>,
) -> Json<ChatResponse> {
    let system_content = body
        .messages
        .iter()
        .find(|m| m.role == "system")
        .map(|m| m.content.clone())
        .unwrap_or_default();
    let user_content = body
        .messages
        .iter()
        .find(|m| m.role == "user")
        .map(|m| m.content.clone())
        .unwrap_or_default();

    requests.lock().unwrap().push(RecordedRequest {
        model: body.model.clone(),
        system_content,
        user_content,
    });

    let response_content = response.lock().unwrap().clone();

    Json(ChatResponse {
        choices: vec![Choice {
            message: ResponseMessage {
                role: "assistant".to_string(),
                content: response_content,
            },
        }],
    })
}

async fn start_mock_server() -> (u16, SharedRequests, SharedResponse) {
    let requests: SharedRequests = Arc::new(Mutex::new(Vec::new()));
    let response: SharedResponse = Arc::new(Mutex::new(String::new()));
    let app = Router::new()
        .route("/v1/chat/completions", post(chat_completions))
        .with_state((requests.clone(), response.clone()));

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    (port, requests, response)
}

fn generate_random_string(rng: &mut impl Rng, length: usize) -> String {
    (0..length)
        .map(|_| {
            let idx = rng.gen_range(0..36);
            if idx < 10 {
                (b'0' + idx) as char
            } else {
                (b'a' + idx - 10) as char
            }
        })
        .collect()
}

fn create_temp_script(dir: &std::path::Path, rng: &mut impl Rng) {
    let script_output = generate_random_string(rng, 8);
    let script_exit_code: i32 = rng.gen_range(0..5);
    let script_path = dir.join("random_cmd.sh");
    let script_content = format!(
        r#"#!/bin/sh
echo "{}"
pwd
exit {}
"#,
        script_output, script_exit_code
    );
    std::fs::write(&script_path, script_content).unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&script_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script_path, perms).unwrap();
    }
}

async fn run_single_test(
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

    // Create separate dirs for daemon and client to verify cwd propagation
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

    // Read stdout until "listening on" message
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
            Ok(0) => break, // EOF
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

    // Spawn drain task to capture remaining stdout
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

        // Verify cwd in AI request is client's cwd, not daemon's
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

    // Verify log file was created and contains expected content
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

                    // Compare log.txt with daemon stdout
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
    }

    (failed, log)
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let (port, requests, response) = start_mock_server().await;
    println!("Mock AI server started on port {port}");

    let mut failures: Vec<u64> = Vec::new();

    for i in 0..args.repetitions {
        let iter_seed = args.seed + i as u64;

        requests.lock().unwrap().clear();

        let (failed, log) =
            run_single_test(iter_seed, port, requests.clone(), response.clone()).await;

        if failed {
            println!(
                "\n=== Repetition {}/{} (seed: {}) FAILED ===",
                i + 1,
                args.repetitions,
                iter_seed
            );
            print!("{log}");
            failures.push(iter_seed);
        }
    }

    println!("\n=== Summary ===");
    println!(
        "Total: {}, Passed: {}, Failed: {}",
        args.repetitions,
        args.repetitions as usize - failures.len(),
        failures.len()
    );

    if !failures.is_empty() {
        eprintln!("Failed seeds: {:?}", failures);
        eprintln!("\nTo reproduce first failure:");
        eprintln!(
            "  cargo run -p rhd_test -- --seed {} --repetitions 1",
            failures[0]
        );
        eprintln!("\nE2E TEST FAILED");
        std::process::exit(1);
    } else {
        println!("\nE2E TEST PASSED");
    }
}
