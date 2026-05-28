use super::error::ApiError;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::agent::process::AgentProcess;
use crate::agent::session as acp;
use crate::server::routes::AppState;

#[derive(Deserialize)]
pub struct OrchestrateRequest {
    agents: Vec<String>,
    messages: Vec<serde_json::Value>,
    #[serde(default = "default_strategy")]
    strategy: String,
    #[serde(default = "default_workspace")]
    workspace_root: String,
    /// Judge agent for "jury" strategy
    #[serde(default)]
    judge: Option<String>,
}

fn default_strategy() -> String {
    "parallel".to_owned()
}
fn default_workspace() -> String {
    "/tmp".to_owned()
}

#[derive(Serialize)]
pub struct OrchestrateResponse {
    strategy: String,
    results: Vec<AgentResult>,
}

#[derive(Serialize)]
pub struct AgentResult {
    agent: String,
    status: String,
    stop_reason: Option<String>,
    error: Option<String>,
    duration_ms: u64,
}

pub async fn orchestrate(
    State(state): State<Arc<AppState>>,
    Json(req): Json<OrchestrateRequest>,
) -> Result<Json<OrchestrateResponse>, (StatusCode, Json<ApiError>)> {
    // Validate all agents exist
    for name in &req.agents {
        if !state.config.agents.contains_key(name) {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ApiError {
                    code: "agent_not_found".to_owned(),
                    message: format!("agent '{name}' not configured"),
                }),
            ));
        }
    }

    let results = match req.strategy.as_str() {
        "race" => run_race(&state, &req).await,
        "jury" => {
            let judge_name = req.judge.as_deref().unwrap_or("claude");
            if !state.config.agents.contains_key(judge_name) {
                return Err((StatusCode::NOT_FOUND, Json(ApiError {
                    code: "judge_not_found".to_owned(),
                    message: format!("judge agent '{judge_name}' not configured"),
                })));
            }
            run_jury(&state, &req, judge_name).await
        }
        _ => run_parallel(&state, &req).await,
    };

    Ok(Json(OrchestrateResponse {
        strategy: req.strategy,
        results,
    }))
}

async fn run_parallel(state: &AppState, req: &OrchestrateRequest) -> Vec<AgentResult> {
    let mut handles = Vec::new();

    for agent_name in &req.agents {
        let config = state.config.agents[agent_name].clone();
        let name = agent_name.clone();
        let messages = req.messages.clone();
        let workspace = req.workspace_root.clone();

        handles.push(tokio::spawn(async move {
            run_single_agent(&name, &config, &workspace, messages).await
        }));
    }

    let mut results = Vec::new();
    for handle in handles {
        match handle.await {
            Ok(result) => results.push(result),
            Err(e) => results.push(AgentResult {
                agent: "unknown".to_owned(),
                status: "error".to_owned(),
                stop_reason: None,
                error: Some(format!("task panicked: {e}")),
                duration_ms: 0,
            }),
        }
    }
    results
}

async fn run_race(state: &AppState, req: &OrchestrateRequest) -> Vec<AgentResult> {
    let mut handles = Vec::new();

    for agent_name in &req.agents {
        let config = state.config.agents[agent_name].clone();
        let name = agent_name.clone();
        let messages = req.messages.clone();
        let workspace = req.workspace_root.clone();

        handles.push(tokio::spawn(async move {
            run_single_agent(&name, &config, &workspace, messages).await
        }));
    }

    // Return first successful result
    let (result, _, remaining) = futures::future::select_all(handles).await;

    // Cancel remaining
    for handle in remaining {
        handle.abort();
    }

    match result {
        Ok(r) => vec![r],
        Err(e) => vec![AgentResult {
            agent: "unknown".to_owned(),
            status: "error".to_owned(),
            stop_reason: None,
            error: Some(format!("task failed: {e}")),
            duration_ms: 0,
        }],
    }
}

async fn run_single_agent(
    name: &str,
    config: &crate::config::types::AgentConfig,
    workspace: &str,
    messages: Vec<serde_json::Value>,
) -> AgentResult {
    let start = Instant::now();

    let result = async {
        let mut process = AgentProcess::spawn(name, config).await?;
        acp::initialize(&mut process).await?;
        let session_id = acp::session_new(&mut process, workspace, None).await?;
        let stop_reason =
            acp::session_prompt(&mut process, &session_id, messages, Duration::from_secs(120), workspace)
                .await?;
        let _ = process.child.kill().await;
        Ok::<String, crate::error::AppError>(stop_reason)
    }
    .await;

    let duration_ms = start.elapsed().as_millis() as u64;

    match result {
        Ok(stop_reason) => AgentResult {
            agent: name.to_owned(),
            status: "success".to_owned(),
            stop_reason: Some(stop_reason),
            error: None,
            duration_ms,
        },
        Err(e) => AgentResult {
            agent: name.to_owned(),
            status: "error".to_owned(),
            stop_reason: None,
            error: Some(e.to_string()),
            duration_ms,
        },
    }
}

async fn run_jury(state: &AppState, req: &OrchestrateRequest, judge_name: &str) -> Vec<AgentResult> {
    // First, run all agents in parallel
    let mut results = run_parallel(state, req).await;

    // Collect successful responses for the judge
    let successful: Vec<&AgentResult> = results.iter().filter(|r| r.status == "success").collect();
    if successful.len() < 2 {
        return results; // Not enough responses to judge
    }

    // Build judge prompt
    let candidates: String = successful
        .iter()
        .enumerate()
        .map(|(i, r)| format!("Candidate {} ({}): completed with {}", i + 1, r.agent, r.stop_reason.as_deref().unwrap_or("unknown")))
        .collect::<Vec<_>>()
        .join("\n");

    let judge_prompt = format!(
        "You are judging responses from multiple AI agents. Select the best one.\n\n{candidates}\n\nWhich candidate produced the best result? Reply with just the number."
    );

    let judge_config = &state.config.agents[judge_name];
    let judge_result = run_single_agent(
        judge_name,
        judge_config,
        &req.workspace_root,
        vec![serde_json::json!({"type": "text", "text": judge_prompt})],
    )
    .await;

    results.push(AgentResult {
        agent: format!("{judge_name} (judge)"),
        status: judge_result.status,
        stop_reason: judge_result.stop_reason,
        error: judge_result.error,
        duration_ms: judge_result.duration_ms,
    });

    results
}
