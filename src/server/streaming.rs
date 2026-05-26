use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::Json;
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use tokio_stream::wrappers::UnboundedReceiverStream;

use crate::agent::process::AgentProcess;
use crate::agent::session as acp;
use crate::server::routes::AppState;

#[derive(Deserialize)]
pub struct StreamPromptRequest {
    pub messages: Vec<serde_json::Value>,
}

#[derive(Serialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
}

/// Send prompt and stream agent notifications as SSE events
pub async fn stream_prompt(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(req): Json<StreamPromptRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>>, (StatusCode, Json<ApiError>)> {
    let entry_arc = state.sessions.get(&id).ok_or_else(|| {
        (StatusCode::NOT_FOUND, Json(ApiError {
            code: "session_not_found".to_owned(),
            message: format!("session '{id}' not found"),
        }))
    })?.value().clone();

    let messages = req.messages;

    // Create a channel for SSE events
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Result<Event, std::convert::Infallible>>();

    // Spawn task to send prompt and forward notifications
    tokio::spawn(async move {
        let entry = entry_arc.lock().await;
        let acp_sid = entry.acp_session_id.clone();
        drop(entry); // Release lock

        let mut entry = entry_arc.lock().await;
        let params = serde_json::json!({
            "sessionId": acp_sid,
            "prompt": messages
        });

        // send_request borrows stdin, then we use notification_rx in the loop
        // Both are fields of process — use raw pointer trick via split borrow
        let process = &mut entry.process;
        let stdin = &mut process.stdin;
        if let Err(e) = crate::protocol::transport::send_request(stdin, "session/prompt", Some(params)).await {
            let _ = tx.send(Ok(Event::default().event("error").data(format!("{{\"error\": \"{e}\"}}"))));
            return;
        }

        let deadline = tokio::time::Instant::now() + Duration::from_secs(120);
        let process = &mut entry.process;

        loop {
            tokio::select! {
                Some(notification) = process.notification_rx.recv() => {
                    let data = serde_json::to_string(&notification.params).unwrap_or_default();
                    let event = Event::default().event(&notification.method).data(data);
                    if tx.send(Ok(event)).is_err() { break; }
                }
                Some(response) = process.response_rx.recv() => {
                    let data = serde_json::to_string(&response.result).unwrap_or_default();
                    let _ = tx.send(Ok(Event::default().event("done").data(data)));
                    break;
                }
                _ = tokio::time::sleep_until(deadline) => {
                    let _ = tx.send(Ok(Event::default().event("error").data("{\"error\": \"timeout\"}")));
                    break;
                }
            }
        }
    });

    let stream = UnboundedReceiverStream::new(rx);
    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

/// Create session and immediately stream — convenience endpoint
pub async fn create_and_stream(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateStreamRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>>, (StatusCode, Json<ApiError>)> {
    let config = state.config.agents.get(&req.agent).ok_or_else(|| {
        (StatusCode::NOT_FOUND, Json(ApiError {
            code: "agent_not_found".to_owned(),
            message: format!("agent '{}' not configured", req.agent),
        }))
    })?.clone();

    let agent_name = req.agent.clone();
    let messages = req.messages;
    let workspace = req.workspace_root;

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Result<Event, std::convert::Infallible>>();

    tokio::spawn(async move {
        // Spawn agent
        let mut process = match AgentProcess::spawn(&agent_name, &config).await {
            Ok(p) => p,
            Err(e) => {
                let _ = tx.send(Ok(Event::default().event("error").data(format!("{{\"error\": \"spawn: {e}\"}}"))));
                return;
            }
        };

        // Initialize
        if let Err(e) = acp::initialize(&mut process).await {
            let _ = tx.send(Ok(Event::default().event("error").data(format!("{{\"error\": \"init: {e}\"}}"))));
            return;
        }

        // Create session
        let session_id = match acp::session_new(&mut process, &workspace, None).await {
            Ok(s) => s,
            Err(e) => {
                let _ = tx.send(Ok(Event::default().event("error").data(format!("{{\"error\": \"session: {e}\"}}"))));
                return;
            }
        };

        let _ = tx.send(Ok(Event::default().event("session_created").data(
            format!("{{\"sessionId\": \"{session_id}\", \"agent\": \"{agent_name}\"}}")
        )));

        // Send prompt
        let params = serde_json::json!({ "sessionId": session_id, "prompt": messages });
        if let Err(e) = crate::protocol::transport::send_request(&mut process.stdin, "session/prompt", Some(params)).await {
            let _ = tx.send(Ok(Event::default().event("error").data(format!("{{\"error\": \"{e}\"}}"))));
            return;
        }

        // Stream
        let deadline = tokio::time::Instant::now() + Duration::from_secs(120);
        loop {
            tokio::select! {
                Some(notification) = process.notification_rx.recv() => {
                    let data = serde_json::to_string(&notification.params).unwrap_or_default();
                    let event = Event::default()
                        .event(&notification.method)
                        .data(data);
                    if tx.send(Ok(event)).is_err() { break; }
                }
                Some(response) = process.response_rx.recv() => {
                    let data = serde_json::to_string(&response.result).unwrap_or_default();
                    let _ = tx.send(Ok(Event::default().event("done").data(data)));
                    break;
                }
                _ = tokio::time::sleep_until(deadline) => {
                    let _ = tx.send(Ok(Event::default().event("error").data("{\"error\": \"timeout\"}")));
                    break;
                }
            }
        }
        let _ = process.child.kill().await;
    });

    let stream = UnboundedReceiverStream::new(rx);
    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

#[derive(Deserialize)]
pub struct CreateStreamRequest {
    pub agent: String,
    pub messages: Vec<serde_json::Value>,
    #[serde(default = "default_workspace")]
    pub workspace_root: String,
}

fn default_workspace() -> String {
    "/tmp".to_owned()
}
