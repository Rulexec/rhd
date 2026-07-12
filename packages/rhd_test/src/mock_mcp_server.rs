use std::io::{self, BufRead, Write};

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    #[serde(default)]
    id: Option<serde_json::Value>,
    method: String,
    #[serde(default)]
    params: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

impl JsonRpcResponse {
    fn success(id: serde_json::Value, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    fn error(id: serde_json::Value, code: i64, message: String) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError { code, message }),
        }
    }
}

pub fn run_mock_mcp_server() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout_lock = stdout.lock();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let request: JsonRpcRequest = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(e) => {
                let error_resp = JsonRpcResponse::error(
                    serde_json::Value::Null,
                    -32700,
                    format!("Parse error: {}", e),
                );
                let _ = writeln!(stdout_lock, "{}", serde_json::to_string(&error_resp).unwrap());
                continue;
            }
        };

        let id = request.id.unwrap_or(serde_json::Value::Null);

        let response = match request.method.as_str() {
            "initialize" => {
                let result = serde_json::json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "tools": {}
                    },
                    "serverInfo": {
                        "name": "mock-mcp",
                        "version": "0.1.0"
                    }
                });
                JsonRpcResponse::success(id, result)
            }
            "notifications/initialized" => {
                continue;
            }
            "tools/list" => {
                let result = serde_json::json!({
                    "tools": [
                        {
                            "name": "echo",
                            "description": "Echoes back the input message",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "message": {
                                        "type": "string",
                                        "description": "The message to echo back"
                                    }
                                },
                                "required": ["message"]
                            }
                        }
                    ]
                });
                JsonRpcResponse::success(id, result)
            }
            "tools/call" => {
                let params = request.params.unwrap_or(serde_json::json!({}));
                let tool_name = params
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("");

                match tool_name {
                    "echo" => {
                        let message = params
                            .get("arguments")
                            .and_then(|a| a.get("message"))
                            .and_then(|m| m.as_str())
                            .unwrap_or("");

                        let result = serde_json::json!({
                            "content": [
                                {
                                    "type": "text",
                                    "text": format!("Echo: {}", message)
                                }
                            ]
                        });
                        JsonRpcResponse::success(id, result)
                    }
                    _ => {
                        let result = serde_json::json!({
                            "content": [
                                {
                                    "type": "text",
                                    "text": format!("Error: unknown tool '{}'", tool_name)
                                }
                            ],
                            "isError": true
                        });
                        JsonRpcResponse::success(id, result)
                    }
                }
            }
            _ => {
                JsonRpcResponse::error(
                    id,
                    -32601,
                    format!("Method not found: {}", request.method),
                )
            }
        };

        let _ = writeln!(stdout_lock, "{}", serde_json::to_string(&response).unwrap());
    }
}
