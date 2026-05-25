#![allow(dead_code)]

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::response::IntoResponse;

use super::session_manager::SharedSessionManager;

/// WebSocket upgrade handler for session update streaming
pub async fn session_stream(
    Path(session_id): Path<String>,
    State(manager): State<SharedSessionManager>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    let rx = manager.subscribe(&session_id);
    ws.on_upgrade(move |socket| handle_socket(socket, rx))
}

async fn handle_socket(
    mut socket: WebSocket,
    rx: Option<tokio::sync::broadcast::Receiver<String>>,
) {
    let Some(mut rx) = rx else {
        let _ = socket
            .send(Message::Close(None))
            .await;
        return;
    };

    loop {
        tokio::select! {
            msg = rx.recv() => {
                match msg {
                    Ok(update) => {
                        if socket.send(Message::Text(update.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                }
            }
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Ping(data))) => {
                        let _ = socket.send(Message::Pong(data)).await;
                    }
                    _ => {}
                }
            }
        }
    }
}

/// SSE fallback endpoint
pub async fn session_events(
    Path(session_id): Path<String>,
    State(manager): State<SharedSessionManager>,
) -> impl IntoResponse {
    use axum::response::sse::{Event, Sse};
    use tokio_stream::wrappers::BroadcastStream;
    use tokio_stream::StreamExt;

    let rx = manager.subscribe(&session_id);

    let stream = match rx {
        Some(rx) => BroadcastStream::new(rx)
            .filter_map(|result| result.ok())
            .map(|data| Ok::<_, std::convert::Infallible>(Event::default().data(data))),
        None => {
            // Return empty stream for non-existent session
            return Sse::new(tokio_stream::empty().map(|_: String| {
                Ok::<_, std::convert::Infallible>(Event::default())
            }))
            .into_response();
        }
    };

    Sse::new(stream).into_response()
}
