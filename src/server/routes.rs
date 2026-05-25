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
    pub agent_name: String,
    pub acp_session_id: String,
    pub process: AgentProcess,
}

/// Shared application state
pub struct AppState {
    pub config: AppConfig,
    pub sessions: DashMap<String, Arc<Mutex<SessionEntry>>>,
    pub usage: super::cost_tracker::UsageStore,
}

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/readiness", get(readiness))
        .route("/api/v1/agents", get(list_agents))
        .route("/api/v1/agents/{name}/models", get(list_agent_models))
        .route("/api/v1/agents/status", get(super::agent_status::get_agent_status))
        .route("/api/v1/usage", get(super::cost_tracker::get_usage))
        .route("/api/v1/sessions", get(list_sessions))
        .route("/api/v1/sessions", post(create_session))
        .route("/api/v1/sessions/{id}", get(get_session))
        .route("/api/v1/sessions/{id}", delete(close_session))
        .route("/api/v1/sessions/{id}/prompt", post(send_prompt))
        .route("/api/v1/sessions/{id}/fork", post(fork_session))
        .route("/api/v1/orchestrate", post(super::orchestrate::orchestrate))
        .route("/api/v1/pipelines", post(super::pipeline::run_pipeline))
        .route("/api/v1/sessions/{id}/stream", post(super::streaming::stream_prompt))
        .route("/api/v1/stream", post(super::streaming::create_and_stream))
        .route("/api/v1/webhooks/gitlab", post(super::webhook_trigger::gitlab_webhook))
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
    #[serde(default)]
    agent: Option<String>,
    #[serde(default = "default_workspace")]
    workspace_root: String,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    auto_route: bool,
    /// Prompt text used for routing decision (not sent to agent)
    #[serde(default)]
    prompt: Option<String>,
    #[serde(default)]
    file_paths: Vec<String>,
    #[serde(default)]
    task_type: Option<String>,
    /// If true, inject git repo context (branch, commits, diff) into session
    #[serde(default)]
    git_context: bool,
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
    // Resolve agent: explicit, auto-route, or error
    let agent_name = if let Some(ref name) = req.agent {
        name.clone()
    } else if req.auto_route {
        let prompt_text = req.prompt.as_deref().unwrap_or("");
        super::routing::resolve_agent(
            &state.config.routing,
            prompt_text,
            &req.file_paths,
            req.task_type.as_deref(),
        )
        .ok_or_else(|| {
            (StatusCode::BAD_REQUEST, Json(ApiError {
                code: "no_route".to_owned(),
                message: "auto_route enabled but no matching rule and no fallback".to_owned(),
            }))
        })?
    } else {
        return Err((StatusCode::BAD_REQUEST, Json(ApiError {
            code: "missing_agent".to_owned(),
            message: "either 'agent' or 'auto_route: true' is required".to_owned(),
        })));
    };

    let config = state.config.agents.get(&agent_name).ok_or_else(|| {
        not_found("agent_not_found", format!("agent '{agent_name}' not configured"))
    })?;

    // Check session limit
    let active_count = state
        .sessions
        .iter()
        .filter(|e| e.key().starts_with(&agent_name))
        .count() as u32;

    if active_count >= config.max_sessions {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            Json(ApiError {
                code: "max_sessions_reached".to_owned(),
                message: format!("agent '{}' has reached max sessions ({})", agent_name, config.max_sessions),
            }),
        ));
    }

    // Spawn agent subprocess
    let mut process = AgentProcess::spawn(&agent_name, config).await.map_err(|e| {
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

    // Inject git context if requested
    if req.git_context
        && let Some(ctx) = super::git_context::extract(&req.workspace_root)
    {
        let context_text = super::git_context::format_for_prompt(&ctx);
        let context_msg = vec![serde_json::json!({"type": "text", "text": format!("[Git Context]\n{context_text}")})];
        let _ = acp::session_prompt(&mut process, &acp_session_id, context_msg, std::time::Duration::from_secs(30)).await;
    }

    let model = req.model.unwrap_or_else(|| {
        config.default_model.clone().unwrap_or_else(|| "default".to_owned())
    });

    let session_id = format!("{}_{}", agent_name, uuid::Uuid::new_v4());
    let entry = SessionEntry {
        agent_name: agent_name.clone(),
        acp_session_id,
        process,
    };

    state.sessions.insert(session_id.clone(), Arc::new(Mutex::new(entry)));

    tracing::info!(session = %session_id, agent = %agent_name, model = %model, "session created");

    Ok((
        StatusCode::CREATED,
        Json(CreateSessionResponse {
            id: session_id,
            agent: agent_name,
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
    let agent_name = entry.agent_name.clone();

    // Apply prompt rewriting (prefix/suffix)
    let messages = if let Some(cfg) = state.config.agents.get(&agent_name) {
        rewrite_messages(req.messages, cfg.prompt_prefix.as_deref(), cfg.prompt_suffix.as_deref())
    } else {
        req.messages
    };

    let stop_reason = acp::session_prompt(
        &mut entry.process,
        &acp_sid,
        messages,
        Duration::from_secs(120),
    )
    .await
    .map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError {
            code: "prompt_failed".to_owned(),
            message: format!("session/prompt failed: {e}"),
        }))
    })?;

    // Record usage
    let model = state.config.agents.get(&agent_name)
        .and_then(|c| c.default_model.clone())
        .unwrap_or_else(|| "default".to_owned());
    super::cost_tracker::record_usage(&state.usage, &agent_name, &model, &id);

    Ok(Json(PromptResponse { stop_reason }))
}

fn rewrite_messages(
    mut messages: Vec<serde_json::Value>,
    prefix: Option<&str>,
    suffix: Option<&str>,
) -> Vec<serde_json::Value> {
    if prefix.is_none() && suffix.is_none() {
        return messages;
    }
    // Wrap first text message with prefix/suffix
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

// --- Fork ---

#[derive(Deserialize)]
struct ForkRequest {
    target_agent: String,
}

#[derive(Serialize)]
struct ForkResponse {
    id: String,
    agent: String,
    forked_from: String,
    status: String,
}

async fn fork_session(
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(req): Json<ForkRequest>,
) -> Result<(StatusCode, Json<ForkResponse>), (StatusCode, Json<ApiError>)> {
    // Verify source session exists
    let source_agent = {
        let entry_arc = state.sessions.get(&id).ok_or_else(|| {
            not_found("session_not_found", format!("session '{id}' not found"))
        })?.value().clone();
        let entry = entry_arc.lock().await;
        entry.agent_name.clone()
    };

    // Verify target agent exists
    let config = state.config.agents.get(&req.target_agent).ok_or_else(|| {
        not_found("agent_not_found", format!("agent '{}' not configured", req.target_agent))
    })?;

    // Spawn new agent for the fork
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

    let acp_session_id = acp::session_new(&mut process, "/tmp").await.map_err(|e| {
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
