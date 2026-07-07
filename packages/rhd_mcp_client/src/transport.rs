use crate::protocol::{JsonRpcRequest, JsonRpcResponse};
use crate::{McpError, McpResult};
use std::collections::HashMap;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

pub struct StdioTransport {
    child: Mutex<Child>,
    stdin: Mutex<tokio::process::ChildStdin>,
    reader: Mutex<BufReader<tokio::process::ChildStdout>>,
}

impl StdioTransport {
    pub async fn spawn(
        cmd: &str,
        args: &[String],
        cwd: Option<&str>,
        env: &HashMap<String, String>,
    ) -> McpResult<Self> {
        let mut command = Command::new(cmd);
        command.args(args);

        if let Some(cwd) = cwd {
            command.current_dir(cwd);
        }

        for (key, value) in env {
            command.env(key, value);
        }

        command.stdin(std::process::Stdio::piped());
        command.stdout(std::process::Stdio::piped());
        command.stderr(std::process::Stdio::inherit());

        let mut child = command.spawn().map_err(|e| {
            McpError::Transport(format!(
                "Failed to spawn MCP server '{}' (cmd: '{}', cwd: {:?}): {}",
                cmd, cmd, cwd, e
            ))
        })?;

        let stdin = child.stdin.take().ok_or_else(|| {
            McpError::Transport("Failed to capture stdin".to_string())
        })?;

        let stdout = child.stdout.take().ok_or_else(|| {
            McpError::Transport("Failed to capture stdout".to_string())
        })?;

        let reader = BufReader::new(stdout);

        Ok(Self {
            child: Mutex::new(child),
            stdin: Mutex::new(stdin),
            reader: Mutex::new(reader),
        })
    }

    pub async fn send_request(&self, request: &JsonRpcRequest) -> McpResult<JsonRpcResponse> {
        let request_json = serde_json::to_string(request)?;
        let mut stdin = self.stdin.lock().await;
        stdin.write_all(request_json.as_bytes()).await.map_err(|e| {
            McpError::Transport(format!("Failed to write to stdin: {}", e))
        })?;
        stdin.write_all(b"\n").await.map_err(|e| {
            McpError::Transport(format!("Failed to write newline: {}", e))
        })?;
        stdin.flush().await.map_err(|e| {
            McpError::Transport(format!("Failed to flush stdin: {}", e))
        })?;

        let mut reader = self.reader.lock().await;
        let mut response_line = String::new();
        reader.read_line(&mut response_line).await.map_err(|e| {
            McpError::Transport(format!("Failed to read from stdout: {}", e))
        })?;

        if response_line.trim().is_empty() {
            return Err(McpError::Transport("Empty response from MCP server".to_string()));
        }

        let response: JsonRpcResponse = serde_json::from_str(&response_line)?;
        Ok(response)
    }

    pub async fn kill(&self) -> McpResult<()> {
        let mut child = self.child.lock().await;
        child.kill().await.map_err(|e| {
            McpError::Transport(format!("Failed to kill MCP server: {}", e))
        })?;
        Ok(())
    }

    pub async fn pid(&self) -> Option<u32> {
        let child = self.child.lock().await;
        child.id()
    }
}

impl Drop for StdioTransport {
    fn drop(&mut self) {
        // Attempt to kill the child process on drop
        // This is best-effort since we can't await in drop
        if let Ok(mut child) = self.child.try_lock() {
            let _ = child.start_kill();
        }
    }
}
