use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::server::routes::AppState;

#[derive(Serialize)]
pub struct AgentStatusInfo {
    pub name: String,
    pub status: String,
    pub active_sessions: u32,
    pub max_sessions: u32,
}

pub async fn get_agent_status(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<AgentStatusInfo>> {
    let statuses: Vec<AgentStatusInfo> = state
        .config
        .agents
        .iter()
        .map(|(name, cfg)| {
            let active = state
                .sessions
                .iter()
                .filter(|e| e.key().starts_with(name.as_str()))
                .count() as u32;

            let status = if active > 0 { "busy" } else { "idle" };

            AgentStatusInfo {
                name: name.clone(),
                status: status.to_owned(),
                active_sessions: active,
                max_sessions: cfg.max_sessions,
            }
        })
        .collect();

    Json(statuses)
}
