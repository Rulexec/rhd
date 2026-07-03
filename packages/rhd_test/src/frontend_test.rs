use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::process::Command;

use crate::control_server::start_control_server;
use crate::mock_server::start_mock_server;

pub async fn run_frontend_test(ws_port: Option<u16>, control_port: Option<u16>) -> bool {
    let workspace_root = std::env::current_dir().unwrap();
    let rhd_bin = workspace_root.join("target/debug/rhd");
    let models_dir = workspace_root.join("test_e2e/models");
    let scenarios_dir = workspace_root.join("test_e2e/scenarios");

    let (ai_port, requests, response, _flag_value, stream_sender, auto_stream) = start_mock_server().await;
    println!("Mock AI server started on port {ai_port}");

    let control_port = control_port.unwrap_or_else(|| find_available_port());
    let daemon_ready: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    start_control_server(control_port, response.clone(), requests.clone(), daemon_ready.clone(), stream_sender.clone(), auto_stream.clone()).await;
    println!("Control server started on port {control_port}");

    let ws_port = ws_port.unwrap_or_else(|| find_available_port());

    let daemon_dir = tempfile::tempdir().unwrap();
    let logs_dir = daemon_dir.path().join("logs");
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
        .arg("--ws-port")
        .arg(ws_port.to_string())
        .env("E2E_MODEL_PORT", ai_port.to_string())
        .current_dir(daemon_dir.path())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn daemon");

    println!("Daemon spawned (PID: {:?})", daemon.id());

    let mut stdout = daemon.stdout.take().expect("failed to take stdout");
    
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
                if line.contains("listening on") {
                    found_listening = true;
                    break;
                }
            }
            Err(_) => break,
        }
    }

    if !found_listening {
        println!("FAIL: Daemon did not start in time");
        daemon.kill().await.ok();
        return false;
    }

    println!("WebSocket server started on port {ws_port}");
    println!("Daemon ready");
    println!("Press Ctrl+C to stop");
    *daemon_ready.lock().unwrap() = true;

    // Wait for interrupt signal
    tokio::signal::ctrl_c().await.ok();

    daemon.kill().await.ok();
    let _ = std::fs::remove_file(&socket_path);

    true
}

fn find_available_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}
