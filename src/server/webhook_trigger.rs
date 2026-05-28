use super::error::ApiError;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::agent::process::AgentProcess;
use crate::agent::session as acp;
use crate::server::routes::AppState;

/// Git webhook payload (simplified)
#[derive(Deserialize)]
pub struct GitWebhook {
    pub object_kind: String,
    #[serde(default)]
    pub object_attributes: Option<MrAttributes>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub struct MrAttributes {
    pub iid: Option<u64>,
    pub title: Option<String>,
    pub action: Option<String>,
    pub source_branch: Option<String>,
    pub target_branch: Option<String>,
    pub description: Option<String>,
}

#[derive(Serialize)]
pub struct WebhookResponse {
    pub status: String,
    pub action_taken: Option<String>,
}

/// Receive Git webhook and trigger agent action
pub async fn git_webhook(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<GitWebhook>,
) -> Result<Json<WebhookResponse>, (StatusCode, Json<ApiError>)> {
    // Validate webhook secret
    let secret = headers
        .get("X-Webhook-Token")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let expected = std::env::var("AINTEGRIX_WEBHOOK_SECRET").unwrap_or_default();
    if !expected.is_empty() && (secret.len() != expected.len() || !bool::from(subtle::ConstantTimeEq::ct_eq(secret.as_bytes(), expected.as_bytes()))) {
        return Err((StatusCode::UNAUTHORIZED, Json(ApiError {
            code: "invalid_webhook_secret".to_owned(),
            message: "invalid webhook token".to_owned(),
        })));
    }

    match payload.object_kind.as_str() {
        "merge_request" => handle_mr_webhook(&state, &payload).await,
        _ => Ok(Json(WebhookResponse {
            status: "ignored".to_owned(),
            action_taken: Some(format!("event '{}' not configured", payload.object_kind)),
        })),
    }
}

async fn handle_mr_webhook(
    state: &AppState,
    payload: &GitWebhook,
) -> Result<Json<WebhookResponse>, (StatusCode, Json<ApiError>)> {
    let attrs = payload.object_attributes.as_ref().ok_or_else(|| {
        (StatusCode::BAD_REQUEST, Json(ApiError {
            code: "missing_attributes".to_owned(),
            message: "merge_request webhook missing object_attributes".to_owned(),
        }))
    })?;

    let action = attrs.action.as_deref().unwrap_or("");
    if action != "open" && action != "reopen" && action != "update" {
        return Ok(Json(WebhookResponse {
            status: "ignored".to_owned(),
            action_taken: Some(format!("MR action '{action}' not triggering review")),
        }));
    }

    // Use claude for code review (or fallback to first available agent)
    let review_agent = if state.config.agents.contains_key("claude") {
        "claude"
    } else {
        state.config.agents.keys().next().map(|s| s.as_str()).unwrap_or("kiro")
    };

    let config = &state.config.agents[review_agent];
    let title = attrs.title.as_deref().unwrap_or("untitled");
    let source = attrs.source_branch.as_deref().unwrap_or("unknown");
    let target = attrs.target_branch.as_deref().unwrap_or("main");
    let desc = attrs.description.as_deref().unwrap_or("");

    let prompt = format!(
        "Review this merge request for bugs, security issues, and code quality:\n\
         Title: {title}\n\
         Branch: {source} → {target}\n\
         Description: {desc}\n\n\
         Provide a concise code review with actionable feedback."
    );

    // Spawn agent and get review (fire-and-forget in background)
    let config_clone = config.clone();
    let agent_name = review_agent.to_owned();
    tokio::spawn(async move {
        let result = run_review(&agent_name, &config_clone, &prompt).await;
        match result {
            Ok(_) => tracing::info!(agent = %agent_name, "webhook review completed"),
            Err(e) => tracing::error!(agent = %agent_name, error = %e, "webhook review failed"),
        }
    });

    Ok(Json(WebhookResponse {
        status: "accepted".to_owned(),
        action_taken: Some(format!("triggered code review with {review_agent}")),
    }))
}

async fn run_review(
    name: &str,
    config: &crate::config::types::AgentConfig,
    prompt: &str,
) -> Result<String, crate::error::AppError> {
    let mut process = AgentProcess::spawn(name, config).await?;
    acp::initialize(&mut process).await?;
    let session_id = acp::session_new(&mut process, "/tmp", None).await?;
    let messages = vec![serde_json::json!({"type": "text", "text": prompt})];
    let result = acp::session_prompt(&mut process, &session_id, messages, Duration::from_secs(120), "/tmp").await?;
    let _ = process.child.kill().await;
    Ok(result)
}
