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
        "clientInfo": {
            "name": "aintegrix",
            "version": env!("CARGO_PKG_VERSION")
        },
        "capabilities": {
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
    let params = json!({ "workspace_root": workspace_root });
    let resp = agent.request("session/new", Some(params), DEFAULT_TIMEOUT).await?;

    if let Some(err) = resp.error {
        return Err(AppError::Protocol(format!("session/new failed: {}", err.message)));
    }

    let result = resp.result.unwrap_or_default();
    result
        .get("session_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned())
        .ok_or_else(|| AppError::Protocol("session/new: missing session_id".to_owned()))
}

/// Send a prompt to an existing session and return the stop reason
pub async fn session_prompt(
    agent: &mut AgentProcess,
    session_id: &str,
    messages: Vec<Value>,
    timeout: Duration,
) -> Result<String, AppError> {
    let params = json!({
        "session_id": session_id,
        "messages": messages
    });

    let resp = agent.request("session/prompt", Some(params), timeout).await?;

    if let Some(err) = resp.error {
        return Err(AppError::Protocol(format!("session/prompt failed: {}", err.message)));
    }

    let result = resp.result.unwrap_or_default();
    result
        .get("stop_reason")
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned())
        .ok_or_else(|| AppError::Protocol("session/prompt: missing stop_reason".to_owned()))
}

/// Send session/cancel notification
pub async fn session_cancel(
    agent: &mut AgentProcess,
    session_id: &str,
) -> Result<(), AppError> {
    agent.notify("session/cancel", Some(json!({ "session_id": session_id }))).await
}
