use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::agent::process::AgentProcess;
use crate::agent::session as acp;
use crate::server::routes::AppState;

#[derive(Debug, Clone, Deserialize)]
pub struct PipelineStep {
    pub agent: String,
    #[serde(default)]
    pub prompt_template: Option<String>,
}

#[derive(Deserialize)]
pub struct PipelineRequest {
    steps: Vec<PipelineStep>,
    messages: Vec<serde_json::Value>,
    #[serde(default = "default_workspace")]
    workspace_root: String,
}

fn default_workspace() -> String {
    "/tmp".to_owned()
}

#[derive(Serialize)]
pub struct PipelineResponse {
    pub steps_completed: usize,
    pub results: Vec<StepResult>,
    pub final_status: String,
}

#[derive(Serialize)]
pub struct StepResult {
    pub step: usize,
    pub agent: String,
    pub status: String,
    pub stop_reason: Option<String>,
    pub error: Option<String>,
    pub duration_ms: u64,
}

#[derive(Serialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
}

pub async fn run_pipeline(
    State(state): State<Arc<AppState>>,
    Json(req): Json<PipelineRequest>,
) -> Result<Json<PipelineResponse>, (StatusCode, Json<ApiError>)> {
    if req.steps.is_empty() {
        return Err((StatusCode::BAD_REQUEST, Json(ApiError {
            code: "empty_pipeline".to_owned(),
            message: "pipeline must have at least one step".to_owned(),
        })));
    }

    // Validate all agents exist
    for step in &req.steps {
        if !state.config.agents.contains_key(&step.agent) {
            return Err((StatusCode::NOT_FOUND, Json(ApiError {
                code: "agent_not_found".to_owned(),
                message: format!("agent '{}' not configured", step.agent),
            })));
        }
    }

    let mut results = Vec::new();
    let mut current_messages = req.messages;

    for (i, step) in req.steps.iter().enumerate() {
        let start = Instant::now();
        let config = &state.config.agents[&step.agent];

        // Apply prompt template if provided (inject previous output)
        if let Some(ref template) = step.prompt_template
            && i > 0
        {
            let prev_context = format!("Previous agent output: step {} completed successfully", i);
            let rendered = template.replace("{{previous}}", &prev_context);
            current_messages = vec![serde_json::json!({"type": "text", "text": rendered})];
        }

        let result = run_step(&step.agent, config, &req.workspace_root, &current_messages).await;
        let duration_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(stop_reason) => {
                results.push(StepResult {
                    step: i + 1,
                    agent: step.agent.clone(),
                    status: "success".to_owned(),
                    stop_reason: Some(stop_reason),
                    error: None,
                    duration_ms,
                });
            }
            Err(e) => {
                results.push(StepResult {
                    step: i + 1,
                    agent: step.agent.clone(),
                    status: "error".to_owned(),
                    stop_reason: None,
                    error: Some(e.to_string()),
                    duration_ms,
                });
                // Pipeline stops on first error
                return Ok(Json(PipelineResponse {
                    steps_completed: i,
                    results,
                    final_status: "failed".to_owned(),
                }));
            }
        }
    }

    let steps_completed = results.len();
    Ok(Json(PipelineResponse {
        steps_completed,
        results,
        final_status: "completed".to_owned(),
    }))
}

async fn run_step(
    name: &str,
    config: &crate::config::types::AgentConfig,
    workspace: &str,
    messages: &[serde_json::Value],
) -> Result<String, crate::error::AppError> {
    let mut process = AgentProcess::spawn(name, config).await?;
    acp::initialize(&mut process).await?;
    let session_id = acp::session_new(&mut process, workspace).await?;
    let stop_reason = acp::session_prompt(
        &mut process,
        &session_id,
        messages.to_vec(),
        Duration::from_secs(120),
    )
    .await?;
    let _ = process.child.kill().await;
    Ok(stop_reason)
}
