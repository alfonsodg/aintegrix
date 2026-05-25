use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::config::types::AppConfig;

/// Shared application state
pub struct AppState {
    pub config: AppConfig,
}

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/readiness", get(readiness))
        .route("/api/v1/agents", get(list_agents))
        .route("/api/v1/sessions", get(list_sessions))
        .route("/api/v1/sessions", post(create_session))
        .route("/api/v1/sessions/{id}", get(get_session))
        .route("/api/v1/sessions/{id}", delete(close_session))
        .route("/api/v1/sessions/{id}/prompt", post(send_prompt))
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
    #[allow(dead_code)]
    #[serde(default = "default_workspace")]
    workspace_root: String,
}

fn default_workspace() -> String {
    "/tmp".to_owned()
}

#[derive(Serialize)]
struct CreateSessionResponse {
    id: String,
    agent: String,
    status: String,
}

#[derive(Deserialize)]
struct PromptRequest {
    #[allow(dead_code)]
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

async fn list_sessions() -> Json<Vec<SessionInfo>> {
    // TODO: integrate with session manager
    Json(vec![])
}

async fn create_session(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateSessionRequest>,
) -> Result<(StatusCode, Json<CreateSessionResponse>), (StatusCode, Json<ApiError>)> {
    if !state.config.agents.contains_key(&req.agent) {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ApiError {
                code: "agent_not_found".to_owned(),
                message: format!("agent '{}' not configured", req.agent),
            }),
        ));
    }

    // TODO: integrate with agent pool to create real session
    let session_id = uuid::Uuid::new_v4().to_string();
    Ok((
        StatusCode::CREATED,
        Json(CreateSessionResponse {
            id: session_id,
            agent: req.agent,
            status: "active".to_owned(),
        }),
    ))
}

async fn get_session(
    Path(id): Path<String>,
) -> Result<Json<SessionInfo>, (StatusCode, Json<ApiError>)> {
    // TODO: lookup from session manager
    Err((
        StatusCode::NOT_FOUND,
        Json(ApiError {
            code: "session_not_found".to_owned(),
            message: format!("session '{id}' not found"),
        }),
    ))
}

async fn close_session(
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ApiError>)> {
    // TODO: close via session manager
    let _ = id;
    Err((
        StatusCode::NOT_FOUND,
        Json(ApiError {
            code: "session_not_found".to_owned(),
            message: "session not found".to_owned(),
        }),
    ))
}

async fn send_prompt(
    Path(id): Path<String>,
    Json(_req): Json<PromptRequest>,
) -> Result<Json<PromptResponse>, (StatusCode, Json<ApiError>)> {
    // TODO: route to agent via session manager
    let _ = id;
    Err((
        StatusCode::NOT_FOUND,
        Json(ApiError {
            code: "session_not_found".to_owned(),
            message: "session not found".to_owned(),
        }),
    ))
}
