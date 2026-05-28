use super::error::ApiError;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::agent::process::AgentProcess;
use crate::agent::session as acp;
use super::error::not_found;
use super::routes::{AppState, SessionEntry};

// --- Fork ---

#[derive(Deserialize)]
pub struct ForkRequest {
    target_agent: String,
}

#[derive(Serialize)]
pub struct ForkResponse {
    id: String,
    agent: String,
    forked_from: String,
    status: String,
}

pub async fn fork_session(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(req): Json<ForkRequest>,
) -> Result<(StatusCode, Json<ForkResponse>), (StatusCode, Json<ApiError>)> {
    let source_agent = {
        let entry_arc = state.sessions.get(&id).ok_or_else(|| {
            not_found("session_not_found", format!("session '{id}' not found"))
        })?.value().clone();
        let entry = entry_arc.lock().await;
        entry.agent_name.clone()
    };

    let config = state.config.agents.get(&req.target_agent).ok_or_else(|| {
        not_found("agent_not_found", format!("agent '{}' not configured", req.target_agent))
    })?;

    let mut process = AgentProcess::spawn(&req.target_agent, config).await.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError {
            code: "spawn_failed".to_owned(),
            message: format!("failed to spawn agent: {e}"),
        }))
    })?;

    acp::initialize(&mut process).await.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError {
            code: "initialize_failed".to_owned(),
            message: format!("ACP initialize failed: {e}"),
        }))
    })?;

    let acp_session_id = acp::session_new(&mut process, "/tmp", None).await.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError {
            code: "session_new_failed".to_owned(),
            message: format!("ACP session/new failed: {e}"),
        }))
    })?;

    let fork_id = format!("{}_{}", req.target_agent, uuid::Uuid::new_v4());
    let entry = SessionEntry {
        agent_name: req.target_agent.clone(),
        acp_session_id,
        process,
        workspace_path: None,
        capabilities: Default::default(),
    };

    state.sessions.insert(fork_id.clone(), Arc::new(Mutex::new(entry)));
    tracing::info!(fork = %fork_id, from = %id, source_agent = %source_agent, target_agent = %req.target_agent, "session forked");

    Ok((StatusCode::CREATED, Json(ForkResponse {
        id: fork_id,
        agent: req.target_agent,
        forked_from: id,
        status: "active".to_owned(),
    })))
}

// --- Prompt Rewriting ---

pub fn rewrite_messages(
    mut messages: Vec<serde_json::Value>,
    prefix: Option<&str>,
    suffix: Option<&str>,
) -> Vec<serde_json::Value> {
    if prefix.is_none() && suffix.is_none() {
        return messages;
    }
    if let Some(msg) = messages.first_mut()
        && let Some(text) = msg.get("text").and_then(|t| t.as_str())
    {
        let mut new_text = String::new();
        if let Some(p) = prefix {
            new_text.push_str(p);
            new_text.push('\n');
        }
        new_text.push_str(text);
        if let Some(s) = suffix {
            new_text.push('\n');
            new_text.push_str(s);
        }
        msg["text"] = serde_json::Value::String(new_text);
    }
    messages
}

// --- Set Model ---

#[derive(Deserialize)]
pub struct SetModelRequest {
    model: String,
}

#[derive(Serialize)]
pub struct SetModelResponse {
    status: String,
    model: String,
}

pub async fn set_model(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(req): Json<SetModelRequest>,
) -> Result<Json<SetModelResponse>, (StatusCode, Json<ApiError>)> {
    let entry_arc = state.sessions.get(&id).ok_or_else(|| {
        not_found("session_not_found", format!("session '{id}' not found"))
    })?.value().clone();

    let mut entry = entry_arc.lock().await;

    let acp_sid = entry.acp_session_id.clone();
    crate::agent::session::session_set_model(&mut entry.process, &acp_sid, &req.model)
        .await
        .map_err(|e| {
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError {
                code: "set_model_failed".to_owned(),
                message: format!("{e}"),
            }))
        })?;

    Ok(Json(SetModelResponse { status: "ok".to_owned(), model: req.model }))
}
