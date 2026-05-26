/// Mock ACP agent for integration testing.
/// Speaks JSON-RPC 2.0 over stdio, responds to standard ACP methods.
use std::io::{self, BufRead, Write};

use serde_json::{json, Value};

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        if line.trim().is_empty() {
            continue;
        }

        let msg: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Only handle requests (have "id" field)
        let id = match msg.get("id") {
            Some(id) => id.clone(),
            None => continue, // notification, ignore
        };

        let method = msg.get("method").and_then(|m| m.as_str()).unwrap_or("");

        let response = match method {
            "initialize" => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": "1.0",
                    "agentCapabilities": {
                        "loadSession": true,
                        "promptCapabilities": {"image": true, "embeddedContext": true},
                        "sessionCapabilities": {"fork": {}, "resume": {}, "close": {}},
                        "mcpCapabilities": {"http": true, "sse": false}
                    },
                    "agentInfo": {
                        "name": "mock-agent",
                        "version": "0.1.0"
                    }
                }
            }),
            "session/new" => {
                // Send a session/update notification first
                let update = json!({
                    "jsonrpc": "2.0",
                    "method": "session/update",
                    "params": { "type": "agent_message_chunk", "text": "Session created." }
                });
                let _ = writeln!(out, "{}", serde_json::to_string(&update).unwrap());

                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": { "session_id": "mock-session-001" }
                })
            }
            "session/prompt" => {
                // Stream some updates
                let updates = [
                    json!({"type": "agent_message_chunk", "text": "Thinking..."}),
                    json!({"type": "tool_call", "id": "tc_1", "name": "read_file", "status": "running"}),
                    json!({"type": "tool_call", "id": "tc_1", "name": "read_file", "status": "completed"}),
                    json!({"type": "agent_message_chunk", "text": "Done."}),
                ];

                for update in &updates {
                    let notif = json!({
                        "jsonrpc": "2.0",
                        "method": "session/update",
                        "params": update
                    });
                    let _ = writeln!(out, "{}", serde_json::to_string(&notif).unwrap());
                }

                json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": { "stop_reason": "end_turn" }
                })
            }
            _ => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": "method not found" }
            }),
        };

        let _ = writeln!(out, "{}", serde_json::to_string(&response).unwrap());
        let _ = out.flush();
    }
}
