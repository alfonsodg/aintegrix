use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::agent::process::AgentProcess;
use crate::agent::session as acp;
use crate::config::types::AppConfig;

/// A live session: owns the agent subprocess
pub struct SessionEntry {
    agent_name: String,
    acp_session_id: String,
    process: AgentProcess,
}

/// Shared application state
pub struct AppState {
    pub config: AppConfig,
    pub sessions: DashMap<String, Arc<Mutex<SessionEntry>>>,
}

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/readiness", get(readiness))
        .route("/api/v1/agents", get(list_agents))
        .route("/api/v1/agents/{name}/models", get(list_agent_models))
        .route("/api/v1/sessions", get(list_sessions))
        .route("/api/v1/sessions", post(create_session))
        .route("/api/v1/sessions/{id}", get(get_session))
        .route("/api/v1/sessions/{id}", delete(close_session))
        .route("/api/v1/sessions/{id}/prompt", post(send_prompt))
        .route("/api/v1/orchestrate", post(super::orchestrate::orchestrate))
        .route("/mcp", post(super::mcp::mcp_handler))
        .route("/mcp/sse", get(super::mcp::mcp_sse))
        .with_state(state)
}

// --- Health ---

async fn health() -> StatusCode {
    StatusCode::OK
}

async fn readiness(State(state): State<Arc<AppState>>) -> StatusCode {
    if state.config.agents.is_empty() {
        return StatusCode::SERVICE_UNAVAILABLE;
    }
    StatusCode::OK
}

// --- Agents ---

#[derive(Serialize)]
struct AgentInfo {
    name: String,
    command: String,
    mode: String,
    max_sessions: u32,
}

async fn list_agents(State(state): State<Arc<AppState>>) -> Json<Vec<AgentInfo>> {
    let agents: Vec<AgentInfo> = state
        .config
        .agents
        .iter()
        .map(|(name, cfg)| AgentInfo {
            name: name.clone(),
            command: cfg.command.clone(),
            mode: cfg.mode.clone(),
            max_sessions: cfg.max_sessions,
        })
        .collect();
    Json(agents)
}

// --- Models ---

#[derive(Serialize)]
struct AgentModels {
    agent: String,
    default_model: Option<String>,
    models: Vec<String>,
}

async fn list_agent_models(
    Path(name): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<AgentModels>, (StatusCode, Json<ApiError>)> {
    match state.config.agents.get(&name) {
        Some(cfg) => Ok(Json(AgentModels {
            agent: name,
            default_model: cfg.default_model.clone(),
            models: cfg.models.clone(),
        })),
        None => Err(not_found("agent_not_found", format!("agent '{name}' not configured"))),
    }
}

// --- Sessions ---

#[derive(Serialize)]
struct SessionInfo {
    id: String,
    agent: String,
    status: String,
}

#[derive(Deserialize)]
struct CreateSessionRequest {
    agent: String,
    #[serde(default = "default_workspace")]
    workspace_root: String,
    #[serde(default)]
    model: Option<String>,
}

fn default_workspace() -> String {
    "/tmp".to_owned()
}

#[derive(Serialize)]
struct CreateSessionResponse {
    id: String,
    agent: String,
    model: String,
    status: String,
}

#[derive(Deserialize)]
struct PromptRequest {
    messages: Vec<serde_json::Value>,
}

#[derive(Serialize)]
struct PromptResponse {
    stop_reason: String,
}

#[derive(Serialize)]
struct ApiError {
    code: String,
    message: String,
}

fn not_found(code: &str, message: String) -> (StatusCode, Json<ApiError>) {
    (StatusCode::NOT_FOUND, Json(ApiError { code: code.to_owned(), message }))
}

async fn list_sessions(State(state): State<Arc<AppState>>) -> Json<Vec<SessionInfo>> {
    let sessions: Vec<SessionInfo> = state
        .sessions
        .iter()
        .map(|entry| {
            let id = entry.key().clone();
            // We can't await inside iter, so just report basic info
            SessionInfo { id, agent: "".to_owned(), status: "active".to_owned() }
        })
        .collect();
    Json(sessions)
}

async fn create_session(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateSessionRequest>,
) -> Result<(StatusCode, Json<CreateSessionResponse>), (StatusCode, Json<ApiError>)> {
    let config = state.config.agents.get(&req.agent).ok_or_else(|| {
        not_found("agent_not_found", format!("agent '{}' not configured", req.agent))
    })?;

    // Check session limit
    let active_count = state
        .sessions
        .iter()
        .filter(|e| {
            // Safe sync check via key naming convention
            e.key().starts_with(&req.agent)
        })
        .count() as u32;

    if active_count >= config.max_sessions {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            Json(ApiError {
                code: "max_sessions_reached".to_owned(),
                message: format!("agent '{}' has reached max sessions ({})", req.agent, config.max_sessions),
            }),
        ));
    }

    // Spawn agent subprocess
    let mut process = AgentProcess::spawn(&req.agent, config).await.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError {
            code: "spawn_failed".to_owned(),
            message: format!("failed to spawn agent: {e}"),
        }))
    })?;

    // ACP initialize handshake
    acp::initialize(&mut process).await.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError {
            code: "initialize_failed".to_owned(),
            message: format!("ACP initialize failed: {e}"),
        }))
    })?;

    // Create ACP session
    let acp_session_id = acp::session_new(&mut process, &req.workspace_root).await.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError {
            code: "session_new_failed".to_owned(),
            message: format!("ACP session/new failed: {e}"),
        }))
    })?;

    let model = req.model.unwrap_or_else(|| {
        config.default_model.clone().unwrap_or_else(|| "default".to_owned())
    });

    let session_id = format!("{}_{}", req.agent, uuid::Uuid::new_v4());
    let entry = SessionEntry {
        agent_name: req.agent.clone(),
        acp_session_id,
        process,
    };

    state.sessions.insert(session_id.clone(), Arc::new(Mutex::new(entry)));

    tracing::info!(session = %session_id, agent = %req.agent, model = %model, "session created");

    Ok((
        StatusCode::CREATED,
        Json(CreateSessionResponse {
            id: session_id,
            agent: req.agent,
            model,
            status: "active".to_owned(),
        }),
    ))
}

async fn get_session(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<SessionInfo>, (StatusCode, Json<ApiError>)> {
    let entry_arc = state.sessions.get(&id).ok_or_else(|| {
        not_found("session_not_found", format!("session '{id}' not found"))
    })?.value().clone();

    let entry = entry_arc.lock().await;
    Ok(Json(SessionInfo {
        id,
        agent: entry.agent_name.clone(),
        status: "active".to_owned(),
    }))
}

async fn close_session(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<StatusCode, (StatusCode, Json<ApiError>)> {
    let (_, entry_arc) = state.sessions.remove(&id).ok_or_else(|| {
        not_found("session_not_found", format!("session '{id}' not found"))
    })?;

    let mut entry = entry_arc.lock().await;
    let _ = entry.process.child.kill().await;
    tracing::info!(session = %id, "session closed");

    Ok(StatusCode::NO_CONTENT)
}

async fn send_prompt(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(req): Json<PromptRequest>,
) -> Result<Json<PromptResponse>, (StatusCode, Json<ApiError>)> {
    let entry_arc = state.sessions.get(&id).ok_or_else(|| {
        not_found("session_not_found", format!("session '{id}' not found"))
    })?.value().clone();

    let mut entry = entry_arc.lock().await;
    let acp_sid = entry.acp_session_id.clone();

    let stop_reason = acp::session_prompt(
        &mut entry.process,
        &acp_sid,
        req.messages,
        Duration::from_secs(120),
    )
    .await
    .map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError {
            code: "prompt_failed".to_owned(),
            message: format!("session/prompt failed: {e}"),
        }))
    })?;

    Ok(Json(PromptResponse { stop_reason }))
}
