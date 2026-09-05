//! Test-only stub MCP server speaking JSON-RPC 2.0 over line-delimited stdio.

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut reader = BufReader::new(tokio::io::stdin()).lines();
    let mut stdout = tokio::io::stdout();

    while let Some(line) = reader.next_line().await? {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(msg) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        // Notifications carry no id → no response.
        let Some(id) = msg.get("id").cloned() else {
            continue;
        };
        let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");

        let response = match method {
            "initialize" => json!({
                "jsonrpc": "2.0", "id": id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": "mcp-stub", "version": "0.1.0" }
                }
            }),
            "tools/list" => json!({
                "jsonrpc": "2.0", "id": id,
                "result": { "tools": [
                    {
                        "name": "echo",
                        "description": "Echo the input",
                        "inputSchema": {
                            "type": "object",
                            "properties": { "input": { "type": "string" } },
                            "required": ["input"]
                        }
                    },
                    {
                        "name": "fail",
                        "description": "Always fails",
                        "inputSchema": { "type": "object", "properties": {} }
                    }
                ] }
            }),
            "tools/call" => {
                let tool = msg
                    .get("params")
                    .and_then(|p| p.get("name"))
                    .and_then(|n| n.as_str())
                    .unwrap_or("");
                if tool == "fail" {
                    json!({
                        "jsonrpc": "2.0", "id": id,
                        "error": { "code": -32603, "message": "boom" }
                    })
                } else {
                    let input = msg
                        .get("params")
                        .and_then(|p| p.get("arguments"))
                        .and_then(|a| a.get("input"))
                        .and_then(|i| i.as_str())
                        .unwrap_or("")
                        .to_string();
                    json!({
                        "jsonrpc": "2.0", "id": id,
                        "result": { "content": [ { "type": "text", "text": format!("echo: {}", input) } ] }
                    })
                }
            }
            _ => json!({ "jsonrpc": "2.0", "id": id, "result": {} }),
        };

        stdout.write_all(response.to_string().as_bytes()).await?;
        stdout.write_all(b"\n").await?;
        stdout.flush().await?;
    }
    Ok(())
}
