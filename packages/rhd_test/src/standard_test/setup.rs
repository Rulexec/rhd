use std::sync::{Arc, Mutex};
use tokio::process::Command;

use crate::utils::spawn_daemon;

pub struct SetupResult {
    pub daemon: tokio::process::Child,
    pub socket_path: std::path::PathBuf,
    pub daemon_dir: tempfile::TempDir,
    pub client_dir: tempfile::TempDir,
    pub logs_dir: std::path::PathBuf,
    pub daemon_stdout: Arc<Mutex<String>>,
}

pub async fn setup_daemon(
    port: u16,
    temp_dir_path: &std::path::Path,
    log: &mut String,
) -> Result<SetupResult, String> {
    let workspace_root = std::env::current_dir().unwrap();
    let rhd_bin = workspace_root.join("target/debug/rhd");

    let daemon_dir = tempfile::tempdir().unwrap();
    let client_dir = tempfile::tempdir().unwrap();
    let logs_dir = daemon_dir.path().join("logs");
    log.push_str(&format!("  Daemon cwd: {}\n", daemon_dir.path().display()));
    log.push_str(&format!("  Client cwd: {}\n", client_dir.path().display()));
    log.push_str(&format!("  Logs dir: {}\n", logs_dir.display()));

    let socket_path = daemon_dir.path().join("rhd.sock");

    let env_vars = vec![
        ("E2E_MODEL_PORT", port.to_string()),
        ("E2E_SCRIPTS_DIR", temp_dir_path.to_string_lossy().to_string()),
    ];

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
        Err(err) => return Err(err.to_string()),
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

    Ok(SetupResult {
        daemon,
        socket_path,
        daemon_dir,
        client_dir,
        logs_dir,
        daemon_stdout,
    })
}

impl SetupResult {
    pub async fn cleanup(mut self) {
        self.daemon.kill().await.ok();
        let _ = std::fs::remove_file(&self.socket_path);
    }
}
