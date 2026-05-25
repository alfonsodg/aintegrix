#![allow(dead_code)]

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::sse::{Event, Sse};
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::types::AppConfig;
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
        "tools/list" => handle_tools_list(id.clone()),
        "tools/call" => handle_tools_call(id.clone(), req.params, &state.config),
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

fn handle_tools_list(id: Value) -> McpResponse {
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
                    "description": "Create a new session on a specific ACP agent",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "agent": {
                                "type": "string",
                                "description": "Agent name (kiro, copilot, opencode, claude, codex)"
                            },
                            "workspace_root": {
                                "type": "string",
                                "description": "Workspace directory path"
                            },
                            "model": {
                                "type": "string",
                                "description": "Model to use (optional, uses agent default if not specified)"
                            }
                        },
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

fn handle_tools_call(id: Value, params: Option<Value>, config: &AppConfig) -> McpResponse {
    let params = params.unwrap_or(json!({}));
    let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

    match tool_name {
        "acp_list_agents" => {
            let agents: Vec<Value> = config
                .agents
                .iter()
                .map(|(name, cfg)| {
                    json!({
                        "name": name,
                        "command": cfg.command,
                        "mode": cfg.mode,
                        "max_sessions": cfg.max_sessions
                    })
                })
                .collect();

            McpResponse::success(
                id,
                json!({
                    "content": [{"type": "text", "text": serde_json::to_string_pretty(&agents).unwrap_or_default()}]
                }),
            )
        }
        "acp_create_session" => {
            let agent = arguments.get("agent").and_then(|v| v.as_str()).unwrap_or("");
            if !config.agents.contains_key(agent) {
                return McpResponse::success(
                    id,
                    json!({
                        "content": [{"type": "text", "text": format!("Error: agent '{}' not found. Available: {:?}", agent, config.agents.keys().collect::<Vec<_>>())}],
                        "isError": true
                    }),
                );
            }
            let agent_cfg = &config.agents[agent];
            let model = arguments
                .get("model")
                .and_then(|v| v.as_str())
                .map(|s| s.to_owned())
                .or_else(|| agent_cfg.default_model.clone())
                .unwrap_or_else(|| "default".to_owned());
            let session_id = uuid::Uuid::new_v4().to_string();
            McpResponse::success(
                id,
                json!({
                    "content": [{"type": "text", "text": format!("Session created: {} (agent: {}, model: {})", session_id, agent, model)}]
                }),
            )
        }
        "acp_prompt" => {
            let session_id = arguments.get("session_id").and_then(|v| v.as_str()).unwrap_or("");
            let message = arguments.get("message").and_then(|v| v.as_str()).unwrap_or("");
            // TODO: route to actual agent subprocess when session manager is wired
            McpResponse::success(
                id,
                json!({
                    "content": [{"type": "text", "text": format!("Prompt sent to session {}. Message: '{}'. (Agent integration pending full wiring)", session_id, message)}]
                }),
            )
        }
        "acp_close_session" => {
            let session_id = arguments.get("session_id").and_then(|v| v.as_str()).unwrap_or("");
            McpResponse::success(
                id,
                json!({
                    "content": [{"type": "text", "text": format!("Session {} closed.", session_id)}]
                }),
            )
        }
        _ => McpResponse::error(id, -32602, &format!("unknown tool: {tool_name}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn test_config() -> AppConfig {
        use crate::config::types::*;
        AppConfig {
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
                },
            )]),
            permissions: PermissionsConfig::default(),
            logging: LoggingConfig::default(),
            routing: crate::server::routing::RoutingConfig::default(),
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
        let resp = handle_tools_list(json!(2));
        let tools = resp.result.unwrap()["tools"].as_array().unwrap().len();
        assert_eq!(tools, 4);
    }

    #[test]
    fn test_list_agents_tool() {
        let config = test_config();
        let resp = handle_tools_call(
            json!(3),
            Some(json!({"name": "acp_list_agents", "arguments": {}})),
            &config,
        );
        let text = resp.result.unwrap()["content"][0]["text"].as_str().unwrap().to_owned();
        assert!(text.contains("kiro"));
    }

    #[test]
    fn test_unknown_agent() {
        let config = test_config();
        let resp = handle_tools_call(
            json!(4),
            Some(json!({"name": "acp_create_session", "arguments": {"agent": "nonexistent"}})),
            &config,
        );
        let result = resp.result.unwrap();
        assert_eq!(result["isError"], true);
    }
}
