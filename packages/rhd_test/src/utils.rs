use std::path::Path;
use std::time::Duration;

use rand::Rng;
use tokio::io::AsyncBufReadExt;
use tokio::process::{Child, ChildStdout, Command};

pub struct SpawnedDaemon {
    pub daemon: Child,
    pub stdout: ChildStdout,
    pub stdout_buf: String,
}

pub async fn spawn_daemon(
    daemon_dir: &Path,
    socket_path: &Path,
    logs_dir: &Path,
    env_vars: &[(&str, String)],
    extra_args: &[String],
    current_dir: Option<&Path>,
    timeout_secs: u64,
    listening_message: &str,
) -> Result<SpawnedDaemon, String> {
    let workspace_root = std::env::current_dir().unwrap();
    let rhd_bin = workspace_root.join("target/debug/rhd");
    let models_dir = workspace_root.join("test_e2e/models");
    let scenarios_dir = workspace_root.join("test_e2e/scenarios");

    let _ = std::fs::remove_file(socket_path);

    let mut cmd = Command::new(&rhd_bin);
    cmd.arg("daemon")
        .arg("--models-dir")
        .arg(&models_dir)
        .arg("--scenarios-dir")
        .arg(&scenarios_dir)
        .arg("--socket")
        .arg(socket_path)
        .arg("--logs")
        .arg(logs_dir);

    for (key, value) in env_vars {
        cmd.env(key, value);
    }

    for arg in extra_args {
        cmd.arg(arg);
    }

    if let Some(dir) = current_dir {
        cmd.current_dir(dir);
    } else {
        cmd.current_dir(daemon_dir);
    }

    cmd.stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());

    let mut daemon = cmd.spawn().expect("failed to spawn daemon");

    let mut stdout = daemon.stdout.take().expect("failed to take stdout");
    let mut reader = tokio::io::BufReader::new(&mut stdout);
    let mut stdout_buf = String::new();
    let mut found_listening = false;

    let timeout_duration = Duration::from_secs(timeout_secs);

    loop {
        let mut line = String::new();
        let read_future = reader.read_line(&mut line);
        let timeout_future = tokio::time::sleep(timeout_duration);

        tokio::select! {
            result = read_future => {
                match result {
                    Ok(0) => break,
                    Ok(_) => {
                        stdout_buf.push_str(&line);
                        if line.contains(listening_message) {
                            found_listening = true;
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            _ = timeout_future => {
                break;
            }
        }
    }

    if !found_listening {
        daemon.kill().await.ok();
        return Err(format!(
            "Daemon did not print '{}' message within {} seconds\nstdout so far: {}",
            listening_message, timeout_secs, stdout_buf
        ));
    }

    Ok(SpawnedDaemon {
        daemon,
        stdout,
        stdout_buf,
    })
}

pub fn generate_random_string(rng: &mut impl Rng, length: usize) -> String {
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

pub fn create_temp_script(dir: &std::path::Path, rng: &mut impl Rng) {
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
