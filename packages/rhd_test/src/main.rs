use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::{extract::State, routing::post, Json, Router};
use rand::Rng;
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tokio::process::Command;

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

fn create_temp_script(dir: &std::path::Path) {
    let script_path = dir.join("random_cmd.sh");
    let script_content = r#"#!/bin/sh
OUTPUT=$(head -c 8 /dev/urandom | base64 | head -c 8)
echo "$OUTPUT"
if [ $((RANDOM % 2)) -eq 0 ]; then
  exit 0
else
  exit $((RANDOM % 5 + 1))
fi
"#;
    std::fs::write(&script_path, script_content).unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&script_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script_path, perms).unwrap();
    }
}

async fn wait_for_socket(socket_path: &std::path::Path, timeout_secs: u64) -> bool {
    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(timeout_secs);
    while start.elapsed() < timeout {
        if socket_path.exists() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    false
}

fn generate_random_string(length: usize) -> String {
    let mut rng = rand::thread_rng();
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

#[tokio::main]
async fn main() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.parent().unwrap().parent().unwrap();
    let rhd_bin = workspace_root.join("target/debug/rhd");
    let models_dir = workspace_root.join("test_e2e/models");
    let scenarios_dir = workspace_root.join("test_e2e/scenarios");

    let (port, requests, response) = start_mock_server().await;
    println!("Mock AI server started on port {port}");

    let random_response = generate_random_string(8);
    *response.lock().unwrap() = random_response.clone();
    println!("Random AI response: {random_response}");

    let temp_dir = tempfile::tempdir().unwrap();
    create_temp_script(temp_dir.path());
    println!("Temp scripts at: {}", temp_dir.path().display());

    let socket_path = std::env::current_dir().unwrap().join("rhd.sock");
    let _ = std::fs::remove_file(&socket_path);

    let mut daemon = Command::new(&rhd_bin)
        .arg("daemon")
        .arg("--models-dir")
        .arg(&models_dir)
        .arg("--scenarios-dir")
        .arg(&scenarios_dir)
        .env("E2E_MODEL_PORT", port.to_string())
        .env("E2E_SCRIPTS_DIR", temp_dir.path())
        .current_dir(workspace_root)
        .spawn()
        .expect("failed to spawn daemon");

    println!("Daemon spawned (PID: {:?})", daemon.id());

    if !wait_for_socket(&socket_path, 10).await {
        eprintln!("ERROR: Daemon failed to create socket");
        daemon.kill().await.ok();
        std::process::exit(1);
    }
    println!("Daemon socket ready");

    let run_output = Command::new(&rhd_bin)
        .arg("run")
        .arg("rhd_test")
        .current_dir(workspace_root)
        .output()
        .await
        .expect("failed to run scenario");

    let stdout = String::from_utf8_lossy(&run_output.stdout);
    let stderr = String::from_utf8_lossy(&run_output.stderr);
    let exit_code = run_output.status.code().unwrap_or(-1);

    println!("rhd run exit code: {exit_code}");
    println!("rhd run stdout: {stdout}");
    if !stderr.is_empty() {
        println!("rhd run stderr: {stderr}");
    }

    daemon.kill().await.ok();
    let _ = std::fs::remove_file(&socket_path);

    let mut failed = false;

    if exit_code != 0 {
        eprintln!("FAIL: expected exit code 0, got {exit_code}");
        failed = true;
    } else {
        println!("PASS: exit code is 0");
    }

    let expected_output = format!("AI said: {random_response}");
    if !stdout.contains(&expected_output) {
        eprintln!("FAIL: output does not contain '{expected_output}'");
        eprintln!("  actual output: {stdout}");
        failed = true;
    } else {
        println!("PASS: output contains '{expected_output}'");
    }

    let recorded = requests.lock().unwrap();
    if recorded.len() != 1 {
        eprintln!("FAIL: expected 1 AI request, got {}", recorded.len());
        failed = true;
    } else {
        println!("PASS: exactly 1 AI request received");
        let req = &recorded[0];

        if req.model != "test-model" {
            eprintln!("FAIL: expected model 'test-model', got '{}'", req.model);
            failed = true;
        } else {
            println!("PASS: model is 'test-model'");
        }

        if req.system_content.contains('%') {
            eprintln!(
                "FAIL: system prompt contains unresolved placeholders: {}",
                req.system_content
            );
            failed = true;
        } else {
            println!("PASS: system prompt has no unresolved placeholders");
        }

        if req.user_content.contains('%') {
            eprintln!(
                "FAIL: user message contains unresolved placeholders: {}",
                req.user_content
            );
            failed = true;
        } else {
            println!("PASS: user message has no unresolved placeholders");
        }

        println!("  system: {}", req.system_content);
        println!("  user: {}", req.user_content);
    }

    if failed {
        eprintln!("\nE2E TEST FAILED");
        std::process::exit(1);
    } else {
        println!("\nE2E TEST PASSED");
    }
}
