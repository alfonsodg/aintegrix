use serde_json::{json, Value};

/// Execute a tool call from the agent (workspace-sandboxed)
pub async fn handle_tool_call(method: &str, params: &Value) -> Value {
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
