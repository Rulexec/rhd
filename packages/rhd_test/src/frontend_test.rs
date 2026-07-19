use std::sync::{Arc, Mutex};

use crate::control_server::start_control_server;
use crate::mock_server::start_mock_server;
use crate::utils::spawn_daemon;

pub async fn run_frontend_test(ws_port: Option<u16>, control_port: Option<u16>) -> bool {
    let workspace_root = std::env::current_dir().unwrap();
    let _rhd_bin = workspace_root.join("target/debug/rhd");
    let _models_dir = workspace_root.join("test_e2e/models");
    let _scenarios_dir = workspace_root.join("test_e2e/scenarios");
    let projects_dir = workspace_root.join("test_e2e/projects");
    let mcp_dir = workspace_root.join("test_e2e/mcp");

    let (ai_port, requests, response, _flag_value, stream_sender, auto_stream) = start_mock_server().await;
    println!("Mock AI server started on port {ai_port}");

    let daemon_dir = tempfile::tempdir().unwrap();
    let logs_dir = daemon_dir.path().join("logs");
    std::fs::create_dir_all(&logs_dir).unwrap();
    let socket_path = daemon_dir.path().join("rhd.sock");

    let control_port = control_port.unwrap_or_else(|| find_available_port());
    let daemon_ready: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    start_control_server(control_port, response.clone(), requests.clone(), daemon_ready.clone(), stream_sender.clone(), auto_stream.clone(), Some(logs_dir.clone()), Some(socket_path.clone())).await;
    println!("Control server started on port {control_port}");

    let ws_port = ws_port.unwrap_or_else(|| find_available_port());
    let _ = std::fs::remove_file(&socket_path);

    let db_dir = daemon_dir.path().join("db");
    std::fs::create_dir_all(&db_dir).unwrap();

    let extra_args = vec![
        "--projects-dir".to_string(),
        projects_dir.to_string_lossy().to_string(),
        "--mcp-dir".to_string(),
        mcp_dir.to_string_lossy().to_string(),
        "--db-dir".to_string(),
        db_dir.to_string_lossy().to_string(),
        "--ws-port".to_string(),
        ws_port.to_string(),
    ];

    let env_vars = vec![("E2E_MODEL_PORT", ai_port.to_string())];

    let spawned = match spawn_daemon(
        daemon_dir.path(),
        &socket_path,
        &logs_dir,
        &env_vars,
        &extra_args,
        Some(&workspace_root),
        10,
        "WebSocket listening",
    )
    .await
    {
        Ok(s) => s,
        Err(err) => {
            println!("FAIL: Daemon startup failed");
            println!("{err}");
            return false;
        }
    };

    let mut daemon = spawned.daemon;
    println!("Daemon spawned (PID: {:?})", daemon.id());

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
