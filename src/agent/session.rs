#![allow(dead_code)]

use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::agent::process::AgentProcess;
use crate::error::AppError;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Capabilities returned by the agent during initialize
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentCapabilities {
    #[serde(default)]
    pub load_session: bool,
    #[serde(default)]
    pub auth: bool,
    #[serde(default)]
    pub modes: Vec<String>,
}

/// Result of a successful initialize handshake
#[derive(Debug, Clone)]
pub struct InitializeResult {
    pub protocol_version: String,
    pub capabilities: AgentCapabilities,
}

/// Perform the ACP initialize handshake with an agent
pub async fn initialize(agent: &mut AgentProcess) -> Result<InitializeResult, AppError> {
    let params = json!({
        "protocolVersion": 1,
        "clientInfo": {
            "name": "aintegrix",
            "version": env!("CARGO_PKG_VERSION")
        },
        "clientCapabilities": {
            "fs": { "readTextFile": true, "writeTextFile": true },
            "terminal": true
        }
    });

    let resp = agent.request("initialize", Some(params), DEFAULT_TIMEOUT).await?;

    if let Some(err) = resp.error {
        return Err(AppError::Protocol(format!("initialize failed: {}", err.message)));
    }

    let result = resp.result.unwrap_or_default();
    let protocol_version = result
        .get("protocolVersion")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_owned();

    let capabilities = result
        .get("capabilities")
        .map(|v| serde_json::from_value(v.clone()).unwrap_or_default())
        .unwrap_or_default();

    Ok(InitializeResult { protocol_version, capabilities })
}

/// Create a new ACP session
pub async fn session_new(
    agent: &mut AgentProcess,
    workspace_root: &str,
) -> Result<String, AppError> {
    let cwd = if workspace_root.is_empty() { "/tmp" } else { workspace_root };
    let params = json!({ "cwd": cwd, "mcpServers": [] });
    let resp = agent.request("session/new", Some(params), DEFAULT_TIMEOUT).await?;

    if let Some(err) = resp.error {
        return Err(AppError::Protocol(format!("session/new failed: {}", err.message)));
    }

    let result = resp.result.unwrap_or_default();
    result
        .get("sessionId")
        .or_else(|| result.get("session_id"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned())
        .ok_or_else(|| AppError::Protocol("session/new: missing sessionId".to_owned()))
}

/// Send a prompt to an existing session and return the stop reason.
/// Handles agent tool call requests (fs/read, terminal) during execution.
pub async fn session_prompt(
    agent: &mut AgentProcess,
    session_id: &str,
    messages: Vec<Value>,
    timeout: Duration,
) -> Result<String, AppError> {
    let params = json!({
        "sessionId": session_id,
        "prompt": messages
    });

    // Send the prompt request
    let request_id = crate::protocol::transport::send_request(
        &mut agent.stdin,
        "session/prompt",
        Some(params),
    )
    .await?;

    // Process notifications and tool call requests until we get our response
    let deadline = tokio::time::Instant::now() + timeout;

    loop {
        tokio::select! {
            Some(resp) = agent.response_rx.recv() => {
                // Check if this is our response
                if resp.id == crate::protocol::types::RequestId::Number(request_id) {
                    if let Some(err) = resp.error {
                        return Err(AppError::Protocol(format!("session/prompt failed: {}", err.message)));
                    }
                    let result = resp.result.unwrap_or_default();
                    let stop_reason = result
                        .get("stop_reason")
                        .or_else(|| result.get("stopReason"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("end_turn")
                        .to_owned();
                    return Ok(stop_reason);
                }
            }
            Some(notif) = agent.notification_rx.recv() => {
                // Handle tool call requests from agent
                if notif.method.starts_with("__request:") {
                    let tool_method = &notif.method["__request:".len()..];
                    let params = notif.params.clone().unwrap_or_default();
                    tracing::info!(tool = %tool_method, "handling tool call from agent");

                    // Extract request ID from params (agent sends it)
                    let req_id = params.get("__request_id")
                        .cloned()
                        .unwrap_or(Value::Null);

                    let result = handle_tool_call(tool_method, &params).await;

                    // Send response back to agent
                    let response_json = json!({
                        "jsonrpc": "2.0",
                        "id": req_id,
                        "result": result
                    });
                    let _ = crate::protocol::transport::send_raw(&mut agent.stdin, &response_json).await;
                    tracing::info!(tool = %tool_method, "tool call response sent");
                } else {
                    tracing::debug!(method = %notif.method, "notification received");
                }
            }
            _ = tokio::time::sleep_until(deadline) => {
                return Err(AppError::Protocol("session/prompt timed out".to_owned()));
            }
        }
    }
}

/// Execute a tool call from the agent
async fn handle_tool_call(method: &str, params: &Value) -> Value {
    match method {
        "session/request_permission" => {
            // Auto-approve: select the first "allow" option
            let options = params.get("options").and_then(|v| v.as_array());
            let option_id = options
                .and_then(|opts| {
                    opts.iter()
                        .find(|o| {
                            let kind = o.get("kind").and_then(|k| k.as_str()).unwrap_or("");
                            kind.starts_with("allow")
                        })
                        .and_then(|o| o.get("optionId").and_then(|v| v.as_str()))
                })
                .unwrap_or("allow-once");
            json!({"outcome": {"outcome": "selected", "optionId": option_id}})
        }
        "fs/read_text_file" => {
            let path = params.get("path").and_then(|v| v.as_str()).unwrap_or("");
            match tokio::fs::read_to_string(path).await {
                Ok(content) => json!({"content": content}),
                Err(e) => json!({"content": format!("Error reading {}: {}", path, e)}),
            }
        }
        "fs/write_text_file" => {
            let path = params.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let content = params.get("content").and_then(|v| v.as_str()).unwrap_or("");
            match tokio::fs::write(path, content).await {
                Ok(()) => json!({}),
                Err(e) => json!({"error": format!("failed to write {}: {}", path, e)}),
            }
        }
        "terminal/execute" => {
            let command = params.get("command").and_then(|v| v.as_str()).unwrap_or("echo no command");
            let output = tokio::process::Command::new("bash")
                .arg("-c")
                .arg(command)
                .output()
                .await;
            match output {
                Ok(o) => json!({
                    "stdout": String::from_utf8_lossy(&o.stdout).to_string(),
                    "stderr": String::from_utf8_lossy(&o.stderr).to_string(),
                    "exitCode": o.status.code().unwrap_or(-1)
                }),
                Err(e) => json!({"error": format!("exec failed: {}", e)}),
            }
        }
        _ => {
            tracing::warn!(method = %method, "unsupported tool call from agent");
            json!({})
        }
    }
}

/// Send session/cancel notification
pub async fn session_cancel(
    agent: &mut AgentProcess,
    session_id: &str,
) -> Result<(), AppError> {
    agent.notify("session/cancel", Some(json!({ "session_id": session_id }))).await
}
