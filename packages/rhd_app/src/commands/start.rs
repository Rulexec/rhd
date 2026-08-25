//! Start command - spawns child processes from YAML config

use serde::Deserialize;
use std::path::{Path, PathBuf};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::signal::ctrl_c;
use tokio::task::JoinHandle;

#[derive(Deserialize)]
struct StartConfig {
    children: Vec<ChildConfig>,
}

#[derive(Deserialize)]
struct ChildConfig {
    name: String,
    cmd: String,
    cwd: Option<String>,
    args: Option<Vec<String>>,
}

pub async fn execute(config_path: String) -> Result<(), Box<dyn std::error::Error>> {
    // Read and parse config
    let config_content = tokio::fs::read_to_string(&config_path).await?;
    let config: StartConfig = serde_yaml::from_str(&config_content)?;

    // Resolve config directory for relative paths
    let config_dir = Path::new(&config_path)
        .parent()
        .unwrap_or(Path::new("."))
        .to_path_buf();

    // Spawn all child processes
    let mut children: Vec<(String, Child)> = Vec::new();
    let mut output_tasks: Vec<JoinHandle<()>> = Vec::new();

    for child_config in config.children {
        let cwd = resolve_cwd(&config_dir, child_config.cwd.as_deref());
        let args = child_config.args.unwrap_or_default();

        let mut child = Command::new(&child_config.cmd)
            .args(&args)
            .current_dir(&cwd)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        // Spawn stdout forwarding task
        if let Some(stdout) = child.stdout.take() {
            let name = child_config.name.clone();
            let task = tokio::spawn(async move {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    println!("[{}] {}", name, line);
                }
            });
            output_tasks.push(task);
        }

        // Spawn stderr forwarding task
        if let Some(stderr) = child.stderr.take() {
            let name = child_config.name.clone();
            let task = tokio::spawn(async move {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    eprintln!("[{}] {}", name, line);
                }
            });
            output_tasks.push(task);
        }

        children.push((child_config.name, child));
    }

    // Wait for Ctrl+C or all children to exit
    tokio::select! {
        _ = ctrl_c() => {
            eprintln!("\nReceived Ctrl+C, shutting down...");
            // Send SIGTERM to all children
            for (name, child) in children.iter_mut() {
                eprintln!("Stopping {}...", name);
                let _ = child.kill().await;
            }
        }
        _ = wait_for_all_children(&mut children) => {
            // All children exited normally
        }
    }

    // Wait for all output tasks to complete
    for task in output_tasks {
        let _ = task.await;
    }

    Ok(())
}

fn resolve_cwd(config_dir: &Path, cwd: Option<&str>) -> PathBuf {
    match cwd {
        Some(cwd_str) => {
            let cwd_path = Path::new(cwd_str);
            if cwd_path.is_absolute() {
                cwd_path.to_path_buf()
            } else {
                config_dir.join(cwd_path)
            }
        }
        None => config_dir.to_path_buf(),
    }
}

async fn wait_for_all_children(children: &mut [(String, Child)]) {
    for (name, child) in children.iter_mut() {
        match child.wait().await {
            Ok(status) => {
                if !status.success() {
                    eprintln!("[{}] exited with status: {}", name, status);
                }
            }
            Err(e) => {
                eprintln!("[{}] error waiting for process: {}", name, e);
            }
        }
    }
}
