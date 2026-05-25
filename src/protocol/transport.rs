#![allow(dead_code)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, ChildStdout};
use tokio::sync::mpsc;

use crate::error::AppError;

use super::types::{Message, Notification, Request, RequestId, Response};

/// Counter for generating unique request IDs
static REQUEST_COUNTER: AtomicU64 = AtomicU64::new(1);

pub fn next_request_id() -> u64 {
    REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed)
}

/// Sends a JSON-RPC request over stdin (newline-delimited)
pub async fn send_request(
    stdin: &mut ChildStdin,
    method: &str,
    params: Option<Value>,
) -> Result<u64, AppError> {
    let id = next_request_id();
    let req = Request::new(id, method, params);
    let mut line = serde_json::to_string(&req)
        .map_err(|e| AppError::Protocol(format!("serialize error: {e}")))?;
    line.push('\n');
    stdin.write_all(line.as_bytes()).await?;
    stdin.flush().await?;
    Ok(id)
}

/// Sends a JSON-RPC notification over stdin (no response expected)
pub async fn send_notification(
    stdin: &mut ChildStdin,
    method: &str,
    params: Option<Value>,
) -> Result<(), AppError> {
    let notif = Notification::new(method, params);
    let mut line = serde_json::to_string(&notif)
        .map_err(|e| AppError::Protocol(format!("serialize error: {e}")))?;
    line.push('\n');
    stdin.write_all(line.as_bytes()).await?;
    stdin.flush().await?;
    Ok(())
}

/// Send a raw JSON value to the agent's stdin
pub async fn send_raw(
    stdin: &mut ChildStdin,
    value: &Value,
) -> Result<(), AppError> {
    let mut line = serde_json::to_string(value)
        .map_err(|e| AppError::Protocol(format!("serialize error: {e}")))?;
    line.push('\n');
    stdin.write_all(line.as_bytes()).await?;
    stdin.flush().await?;
    Ok(())
}

/// Reads messages from stdout and dispatches them to the appropriate channel.
/// Responses go to `response_tx`, notifications go to `notification_tx`.
pub async fn read_loop(
    stdout: ChildStdout,
    response_tx: mpsc::UnboundedSender<Response>,
    notification_tx: mpsc::UnboundedSender<Notification>,
) {
    let reader = BufReader::new(stdout);
    let mut lines = reader.lines();

    while let Ok(Some(line)) = lines.next_line().await {
        if line.trim().is_empty() {
            continue;
        }

        match serde_json::from_str::<Message>(&line) {
            Ok(Message::Response(resp)) => {
                let _ = response_tx.send(resp);
            }
            Ok(Message::Notification(notif)) => {
                let _ = notification_tx.send(notif);
            }
            Ok(Message::Request(req)) => {
                // Agent-to-client requests (fs/read_text_file, etc.)
                // Include the request ID so we can respond
                let mut params = req.params.unwrap_or(json!({}));
                if let Some(obj) = params.as_object_mut() {
                    obj.insert("__request_id".to_owned(), serde_json::to_value(&req.id).unwrap_or_default());
                }
                let _ = notification_tx.send(Notification {
                    jsonrpc: "2.0".to_owned(),
                    method: format!("__request:{}", req.method),
                    params: Some(params),
                });
            }
            Err(e) => {
                tracing::warn!(error = %e, raw = %line, "failed to parse message from agent");
            }
        }
    }
}

/// Wait for a response with a specific request ID, with timeout.
pub async fn wait_for_response(
    rx: &mut mpsc::UnboundedReceiver<Response>,
    expected_id: RequestId,
    timeout: Duration,
) -> Result<Response, AppError> {
    let result = tokio::time::timeout(timeout, async {
        while let Some(resp) = rx.recv().await {
            if resp.id == expected_id {
                return Ok(resp);
            }
        }
        Err(AppError::Protocol("response channel closed".to_owned()))
    })
    .await;

    match result {
        Ok(inner) => inner,
        Err(_) => Err(AppError::Protocol("request timed out".to_owned())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_next_request_id_increments() {
        let a = next_request_id();
        let b = next_request_id();
        assert_eq!(b, a + 1);
    }
}
