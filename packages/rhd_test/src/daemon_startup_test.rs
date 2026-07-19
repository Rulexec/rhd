use crate::utils::spawn_daemon;

pub async fn run_daemon_startup_test() -> (bool, String) {
    let mut log = String::new();

    let daemon_dir = tempfile::tempdir().unwrap();
    let logs_dir = daemon_dir.path().join("logs");
    log.push_str(&format!("  Daemon cwd: {}\n", daemon_dir.path().display()));
    log.push_str(&format!("  Logs dir: {}\n", logs_dir.display()));

    let socket_path = daemon_dir.path().join("rhd.sock");

    let spawned = match spawn_daemon(
        daemon_dir.path(),
        &socket_path,
        &logs_dir,
        &[],
        &[],
        None,
        5,
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
    log.push_str(&format!("  Daemon spawned (PID: {:?})\n", daemon.id()));

    daemon.kill().await.ok();
    let _ = std::fs::remove_file(&socket_path);

    log.push_str("  PASS: Daemon started successfully (listening message received)\n");
    (false, log)
}
