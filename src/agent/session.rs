#![allow(dead_code)]

use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::agent::process::AgentProcess;
use crate::error::AppError;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Capabilities advertised by the agent during initialize
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCapabilities {
    #[serde(default)]
    pub load_session: bool,
    #[serde(default)]
    pub prompt_capabilities: PromptCapabilities,
    #[serde(default)]
    pub session_capabilities: SessionCapabilities,
    #[serde(default)]
    pub mcp_capabilities: McpCapabilities,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptCapabilities {
    #[serde(default)]
    pub image: bool,
    #[serde(default)]
    pub embedded_context: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCapabilities {
    #[serde(default)]
    pub fork: Option<serde_json::Value>,
    #[serde(default)]
    pub resume: Option<serde_json::Value>,
    #[serde(default)]
    pub close: Option<serde_json::Value>,
    #[serde(default)]
    pub delete: Option<serde_json::Value>,
    #[serde(default)]
    pub list: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpCapabilities {
    #[serde(default)]
    pub http: bool,
    #[serde(default)]
    pub sse: bool,
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
        .get("agentCapabilities")
        .or_else(|| result.get("capabilities"))
        .map(|v| serde_json::from_value(v.clone()).unwrap_or_default())
        .unwrap_or_default();

    Ok(InitializeResult { protocol_version, capabilities })
}

/// Create a new ACP session
pub async fn session_new(
    agent: &mut AgentProcess,
    workspace_root: &str,
    mcp_servers: Option<&[Value]>,
) -> Result<String, AppError> {
    let cwd = if workspace_root.is_empty() { "/tmp" } else { workspace_root };
    let servers = mcp_servers.unwrap_or(&[]);
    let params = json!({ "cwd": cwd, "mcpServers": servers });
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
    workspace: &str,
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
    let mut response_text = String::new();

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
                    // Return accumulated text or just stop_reason
                    if response_text.is_empty() {
                        return Ok(stop_reason);
                    }
                    return Ok(response_text);
                }
            }
            Some(notif) = agent.notification_rx.recv() => {
                // Handle tool call requests from agent
                if notif.method.starts_with("__request:") {
                    let tool_method = &notif.method["__request:".len()..];
                    let params = notif.params.clone().unwrap_or_default();
                    tracing::info!(tool = %tool_method, "handling tool call from agent");

                    let req_id = params.get("__request_id")
                        .cloned()
                        .unwrap_or(Value::Null);

                    // Inject workspace for path-safe tool calls
                    let mut tool_params = params.clone();
                    if let Some(obj) = tool_params.as_object_mut() {
                        obj.insert("__workspace".to_owned(), Value::String(workspace.to_owned()));
                    }

                    let result = handle_tool_call(tool_method, &tool_params).await;

                    let response_json = json!({
                        "jsonrpc": "2.0",
                        "id": req_id,
                        "result": result
                    });
                    let _ = crate::protocol::transport::send_raw(&mut agent.stdin, &response_json).await;
                    tracing::info!(tool = %tool_method, "tool call response sent");
                } else {
                    // Capture agent message chunks
                    if let Some(params) = &notif.params
                        && let Some(update) = params.get("update")
                        && let Some(update_type) = update.get("sessionUpdate").and_then(|v| v.as_str())
                        && (update_type == "agent_message_chunk" || update_type == "AgentMessageChunk")
                        && let Some(text) = update.get("content").and_then(|c| c.get("text")).and_then(|t| t.as_str())
                    {
                        response_text.push_str(text);
                    }
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
            let workspace = params.get("__workspace").and_then(|v| v.as_str()).unwrap_or("/tmp");
            match crate::server::fs_handler::read_text_file(workspace, path).await {
                Ok(content) => json!({"content": content}),
                Err(e) => json!({"content": format!("Error: {e}")}),
            }
        }
        "fs/write_text_file" => {
            let path = params.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let content = params.get("content").and_then(|v| v.as_str()).unwrap_or("");
            let workspace = params.get("__workspace").and_then(|v| v.as_str()).unwrap_or("/tmp");
            match crate::server::fs_handler::write_text_file(workspace, path, content).await {
                Ok(()) => json!({}),
                Err(e) => json!({"error": format!("{e}")}),
            }
        }
        "terminal/execute" => {
            let command = params.get("command").and_then(|v| v.as_str()).unwrap_or("echo no command");
            let workspace = params.get("__workspace").and_then(|v| v.as_str()).unwrap_or("/tmp");
            tracing::info!(command = %command, workspace = %workspace, "terminal/execute");
            let output = tokio::process::Command::new("bash")
                .arg("-c")
                .arg(command)
                .current_dir(workspace)
                .output()
                .await;
            match output {
                Ok(o) => json!({
                    "stdout": String::from_utf8_lossy(&o.stdout).to_string(),
                    "stderr": String::from_utf8_lossy(&o.stderr).to_string(),
                    "exitCode": o.status.code().unwrap_or(-1)
                }),
                Err(e) => json!({"error": format!("exec failed: {e}")}),
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

/// Change model for a session (capability-gated: agent must support it)
pub async fn session_set_model(
    agent: &mut AgentProcess,
    session_id: &str,
    model: &str,
) -> Result<(), AppError> {
    let params = json!({"sessionId": session_id, "model": model});
    let resp = agent.request("session/set_model", Some(params), DEFAULT_TIMEOUT).await?;
    if let Some(err) = resp.error {
        return Err(AppError::Protocol(format!("session/set_model failed: {}", err.message)));
    }
    Ok(())
}

/// Native fork a session (capability-gated: sessionCapabilities.fork)
pub async fn session_fork(
    agent: &mut AgentProcess,
    session_id: &str,
) -> Result<String, AppError> {
    let params = json!({"sessionId": session_id});
    let resp = agent.request("session/fork", Some(params), DEFAULT_TIMEOUT).await?;
    if let Some(err) = resp.error {
        return Err(AppError::Protocol(format!("session/fork failed: {}", err.message)));
    }
    let result = resp.result.unwrap_or_default();
    result.get("sessionId").and_then(|v| v.as_str()).map(|s| s.to_owned())
        .ok_or_else(|| AppError::Protocol("session/fork: missing sessionId".to_owned()))
}

/// Load a previous session (capability-gated: loadSession)
pub async fn session_load(
    agent: &mut AgentProcess,
    session_id: &str,
    cwd: &str,
) -> Result<(), AppError> {
    let params = json!({"sessionId": session_id, "cwd": cwd, "mcpServers": []});
    let resp = agent.request("session/load", Some(params), Duration::from_secs(60)).await?;
    if let Some(err) = resp.error {
        return Err(AppError::Protocol(format!("session/load failed: {}", err.message)));
    }
    Ok(())
}

/// Resume a session without history replay (capability-gated: sessionCapabilities.resume)
pub async fn session_resume(
    agent: &mut AgentProcess,
    session_id: &str,
    cwd: &str,
) -> Result<(), AppError> {
    let params = json!({"sessionId": session_id, "cwd": cwd});
    let resp = agent.request("session/resume", Some(params), DEFAULT_TIMEOUT).await?;
    if let Some(err) = resp.error {
        return Err(AppError::Protocol(format!("session/resume failed: {}", err.message)));
    }
    Ok(())
}
