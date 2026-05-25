#![allow(dead_code)]

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::sse::{Event, Sse};
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::routes::AppState;

/// JSON-RPC request for MCP
#[derive(Deserialize)]
pub struct McpRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

/// JSON-RPC response
#[derive(Serialize)]
pub struct McpResponse {
    pub jsonrpc: String,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<Value>,
}

impl McpResponse {
    fn success(id: Value, result: Value) -> Self {
        Self { jsonrpc: "2.0".to_owned(), id, result: Some(result), error: None }
    }

    fn error(id: Value, code: i32, message: &str) -> Self {
        Self {
            jsonrpc: "2.0".to_owned(),
            id,
            result: None,
            error: Some(json!({"code": code, "message": message})),
        }
    }
}

/// POST /mcp — Streamable HTTP MCP endpoint
pub async fn mcp_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<McpRequest>,
) -> impl IntoResponse {
    let id = req.id.unwrap_or(Value::Null);

    let response = match req.method.as_str() {
        "initialize" => handle_initialize(id.clone()),
        "notifications/initialized" => return (StatusCode::OK, Json(json!({}))).into_response(),
        "tools/list" => handle_tools_list(id.clone(), &state.config),
        "tools/call" => handle_tools_call(id.clone(), req.params, &state).await,
        _ => McpResponse::error(id, -32601, "method not found"),
    };

    Json(response).into_response()
}

/// GET /mcp/sse — SSE transport for MCP (server-sent events)
pub async fn mcp_sse(State(_state): State<Arc<AppState>>) -> impl IntoResponse {
    let stream = tokio_stream::once(Ok::<_, std::convert::Infallible>(
        Event::default()
            .event("endpoint")
            .data("/mcp"),
    ));

    Sse::new(stream)
}

fn handle_initialize(id: Value) -> McpResponse {
    McpResponse::success(
        id,
        json!({
            "protocolVersion": "2025-11-25",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "aintegrix",
                "version": env!("CARGO_PKG_VERSION")
            }
        }),
    )
}

fn handle_tools_list(id: Value, config: &crate::config::types::AppConfig) -> McpResponse {
    let is_local = config.server.host == "127.0.0.1" || config.server.host == "localhost";

    let create_desc = if is_local {
        "Create a new session on a specific ACP agent. Pass workspace_root with the LOCAL directory path where the code lives (e.g. '/home/user/project'). Do NOT use repo paths — this is a local server with direct filesystem access."
    } else {
        "Create a new session on a specific ACP agent. Pass workspace_root with a Git repo path (e.g. 'myorg/project') to clone, or a server-side directory. Push changes before calling."
    };

    let ws_desc = if is_local {
        "Absolute local directory path where the code lives (e.g. '/home/user/myproject')"
    } else {
        "Git repo path (e.g. 'myorg/myproject') or server-side directory path"
    };
    McpResponse::success(
        id,
        json!({
            "tools": [
                {
                    "name": "acp_list_agents",
                    "description": "List all available ACP agents and their configuration",
                    "inputSchema": {
                        "type": "object",
                        "properties": {}
                    }
                },
                {
                    "name": "acp_create_session",
                    "description": create_desc,
                    "inputSchema": {
                        "type": "object",
                        "properties": if is_local { json!({
                            "agent": {"type": "string", "description": "Agent name (kiro, copilot, opencode, claude, codex)"},
                            "workspace_root": {"type": "string", "description": ws_desc},
                            "model": {"type": "string", "description": "Model to use (optional)"}
                        }) } else { json!({
                            "agent": {"type": "string", "description": "Agent name (kiro, copilot, opencode, claude, codex)"},
                            "workspace_root": {"type": "string", "description": ws_desc},
                            "branch": {"type": "string", "description": "Branch to clone (defaults to 'develop')"},
                            "model": {"type": "string", "description": "Model to use (optional)"}
                        }) },
                        "required": ["agent"]
                    }
                },
                {
                    "name": "acp_prompt",
                    "description": "Send a prompt to an ACP agent session and get the response",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "session_id": {
                                "type": "string",
                                "description": "Session ID from acp_create_session"
                            },
                            "message": {
                                "type": "string",
                                "description": "The prompt message to send to the agent"
                            }
                        },
                        "required": ["session_id", "message"]
                    }
                },
                {
                    "name": "acp_close_session",
                    "description": "Close an ACP agent session",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "session_id": {
                                "type": "string",
                                "description": "Session ID to close"
                            }
                        },
                        "required": ["session_id"]
                    }
                }
            ]
        }),
    )
}

async fn handle_tools_call(id: Value, params: Option<Value>, state: &AppState) -> McpResponse {
    let params = params.unwrap_or(json!({}));
    let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

    match tool_name {
        "acp_list_agents" => {
            let agents: Vec<Value> = state.config.agents.iter()
                .map(|(name, cfg)| json!({"name": name, "command": cfg.command, "mode": cfg.mode, "max_sessions": cfg.max_sessions}))
                .collect();
            McpResponse::success(id, json!({"content": [{"type": "text", "text": serde_json::to_string_pretty(&agents).unwrap_or_default()}]}))
        }
        "acp_create_session" => {
            let agent = arguments.get("agent").and_then(|v| v.as_str()).unwrap_or("");
            let Some(cfg) = state.config.agents.get(agent) else {
                return McpResponse::success(id, json!({"content": [{"type": "text", "text": format!("Error: agent '{}' not found. Available: {:?}", agent, state.config.agents.keys().collect::<Vec<_>>())}], "isError": true}));
            };
            let workspace = arguments.get("workspace_root")
                .or_else(|| arguments.get("repo"))
                .and_then(|v| v.as_str())
                .unwrap_or("/tmp");
            let branch = arguments.get("branch").and_then(|v| v.as_str()).unwrap_or("develop");

            // Auto-detect: if path exists locally → use it. If not → try clone as repo.
            let (resolved_workspace, ws_path) = if std::path::Path::new(workspace).exists() {
                (workspace.to_owned(), None)
            } else if workspace.contains('/') && !workspace.starts_with('/') {
                // Looks like a repo path (e.g. "myorg/myproject")
                match super::workspace::clone_repo(workspace, branch).await {
                    Ok(path) => {
                        let p = path.to_string_lossy().to_string();
                        (p.clone(), Some(p))
                    }
                    Err(e) => return McpResponse::success(id, json!({"content": [{"type": "text", "text": format!("Error: '{}' not found locally and clone failed: {}", workspace, e)}], "isError": true})),
                }
            } else {
                return McpResponse::success(id, json!({"content": [{"type": "text", "text": format!("Error: '{}' does not exist.", workspace)}], "isError": true}));
            };
            let mut process = match crate::agent::process::AgentProcess::spawn(agent, cfg).await {
                Ok(p) => p,
                Err(e) => return McpResponse::success(id, json!({"content": [{"type": "text", "text": format!("Error spawning agent: {e}")}], "isError": true})),
            };
            if let Err(e) = crate::agent::session::initialize(&mut process).await {
                return McpResponse::success(id, json!({"content": [{"type": "text", "text": format!("Error initializing: {e}")}], "isError": true}));
            }
            let acp_session_id = match crate::agent::session::session_new(&mut process, &resolved_workspace).await {
                Ok(s) => s,
                Err(e) => return McpResponse::success(id, json!({"content": [{"type": "text", "text": format!("Error creating session: {e}")}], "isError": true})),
            };
            let session_id = format!("{}_{}", agent, uuid::Uuid::new_v4());
            let entry = super::routes::SessionEntry { agent_name: agent.to_owned(), acp_session_id, process, workspace_path: ws_path };
            state.sessions.insert(session_id.clone(), std::sync::Arc::new(tokio::sync::Mutex::new(entry)));
            let model = arguments.get("model").and_then(|v| v.as_str()).map(|s| s.to_owned()).or_else(|| cfg.default_model.clone()).unwrap_or_else(|| "default".to_owned());
            McpResponse::success(id, json!({"content": [{"type": "text", "text": format!("Session created: {} (agent: {}, model: {})", session_id, agent, model)}]}))
        }
        "acp_prompt" => {
            let session_id = arguments.get("session_id").and_then(|v| v.as_str()).unwrap_or("");
            let message = arguments.get("message").and_then(|v| v.as_str()).unwrap_or("");
            let Some(entry_arc) = state.sessions.get(session_id).map(|e| e.value().clone()) else {
                return McpResponse::success(id, json!({"content": [{"type": "text", "text": format!("Error: session '{}' not found", session_id)}], "isError": true}));
            };
            let mut entry = entry_arc.lock().await;
            let acp_sid = entry.acp_session_id.clone();
            let messages = vec![json!({"type": "text", "text": message})];
            match crate::agent::session::session_prompt(&mut entry.process, &acp_sid, messages, std::time::Duration::from_secs(120)).await {
                Ok(response_text) => McpResponse::success(id, json!({"content": [{"type": "text", "text": response_text}]})),
                Err(e) => McpResponse::success(id, json!({"content": [{"type": "text", "text": format!("Error: {e}")}], "isError": true})),
            }
        }
        "acp_close_session" => {
            let session_id = arguments.get("session_id").and_then(|v| v.as_str()).unwrap_or("");
            if let Some((_, entry_arc)) = state.sessions.remove(session_id) {
                let mut entry = entry_arc.lock().await;
                let _ = entry.process.child.kill().await;
                McpResponse::success(id, json!({"content": [{"type": "text", "text": format!("Session {} closed.", session_id)}]}))
            } else {
                McpResponse::success(id, json!({"content": [{"type": "text", "text": format!("Session '{}' not found", session_id)}], "isError": true}))
            }
        }
        _ => McpResponse::error(id, -32602, &format!("unknown tool: {tool_name}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use crate::config::types::*;

    fn test_state() -> AppState {
        AppState {
            config: AppConfig {
                server: ServerConfig { host: "127.0.0.1".to_owned(), port: 8050 },
                agents: HashMap::from([(
                    "kiro".to_owned(),
                    AgentConfig {
                        command: "kiro-cli".to_owned(),
                        args: vec!["acp".to_owned()],
                        mode: "native".to_owned(),
                        max_sessions: 3,
                        auto_restart: true,
                        env: HashMap::new(),
                        default_model: None,
                        models: vec![],
                        prompt_prefix: None,
                        prompt_suffix: None,
                        context_inject: Default::default(),
                    },
                )]),
                permissions: PermissionsConfig::default(),
                logging: LoggingConfig::default(),
                routing: crate::server::routing::RoutingConfig::default(),
            },
            sessions: dashmap::DashMap::new(),
            usage: dashmap::DashMap::new(),
        }
    }

    #[test]
    fn test_initialize() {
        let resp = handle_initialize(json!(1));
        let result = resp.result.unwrap();
        assert_eq!(result["protocolVersion"], "2025-11-25");
        assert_eq!(result["serverInfo"]["name"], "aintegrix");
    }

    #[test]
    fn test_tools_list() {
        let state = test_state();
        let resp = handle_tools_list(json!(2), &state.config);
        let tools = resp.result.unwrap()["tools"].as_array().unwrap().len();
        assert_eq!(tools, 4);
    }

    #[tokio::test]
    async fn test_list_agents_tool() {
        let state = test_state();
        let resp = handle_tools_call(
            json!(3),
            Some(json!({"name": "acp_list_agents", "arguments": {}})),
            &state,
        ).await;
        let text = resp.result.unwrap()["content"][0]["text"].as_str().unwrap().to_owned();
        assert!(text.contains("kiro"));
    }

    #[tokio::test]
    async fn test_unknown_agent() {
        let state = test_state();
        let resp = handle_tools_call(
            json!(4),
            Some(json!({"name": "acp_create_session", "arguments": {"agent": "nonexistent"}})),
            &state,
        ).await;
        let result = resp.result.unwrap();
        assert_eq!(result["isError"], true);
    }
}
