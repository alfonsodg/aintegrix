//! HTTP integration tests for AIntegriX API endpoints
//! Tests the router, auth middleware, and handlers without spawning real agents.

use std::sync::Arc;

use axum::http::StatusCode;
use axum_test::TestServer;

use aintegrix::config::types::*;
use aintegrix::server::routes::{AppState, create_router};

fn test_state() -> Arc<AppState> {
    use std::collections::HashMap;

    let mut agents = HashMap::new();
    agents.insert("mock".to_owned(), AgentConfig {
        command: "/bin/false".to_owned(),
        args: vec![],
        mode: "native".to_owned(),
        max_sessions: 3,
        auto_restart: false,
        env: HashMap::new(),
        default_model: Some("test-model".to_owned()),
        models: vec!["test-model".to_owned(), "other-model".to_owned()],
        prompt_prefix: Some("PREFIX:".to_owned()),
        prompt_suffix: None,
        context_inject: Default::default(),
    });

    Arc::new(AppState {
        config: AppConfig {
            server: ServerConfig { host: "127.0.0.1".to_owned(), port: 0 },
            agents,
            permissions: PermissionsConfig::default(),
            logging: LoggingConfig::default(),
            routing: aintegrix::server::routing::RoutingConfig::default(),
        },
        sessions: dashmap::DashMap::new(),
        usage: dashmap::DashMap::new(),
    })
}

fn test_server() -> TestServer {
    let state = test_state();
    let app = create_router(state);
    TestServer::new(app)
}

// --- Health ---

#[tokio::test]
async fn test_health_no_auth() {
    let server = test_server();
    let resp = server.get("/health").await;
    resp.assert_status(StatusCode::OK);
}

#[tokio::test]
async fn test_readiness_with_agents() {
    let server = test_server();
    let resp = server.get("/readiness").await;
    resp.assert_status(StatusCode::OK);
}

// --- Agents ---

#[tokio::test]
async fn test_list_agents() {
    let server = test_server();
    let resp = server.get("/api/v1/agents").await;
    resp.assert_status(StatusCode::OK);
    let body: Vec<serde_json::Value> = resp.json();
    assert_eq!(body.len(), 1);
    assert_eq!(body[0]["name"], "mock");
}

#[tokio::test]
async fn test_agent_models() {
    let server = test_server();
    let resp = server.get("/api/v1/agents/mock/models").await;
    resp.assert_status(StatusCode::OK);
    let body: serde_json::Value = resp.json();
    assert_eq!(body["default_model"], "test-model");
    assert_eq!(body["models"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn test_agent_models_not_found() {
    let server = test_server();
    let resp = server.get("/api/v1/agents/nonexistent/models").await;
    resp.assert_status(StatusCode::NOT_FOUND);
}

// --- Agent Status ---

#[tokio::test]
async fn test_agent_status() {
    let server = test_server();
    let resp = server.get("/api/v1/agents/status").await;
    resp.assert_status(StatusCode::OK);
    let body: Vec<serde_json::Value> = resp.json();
    assert_eq!(body.len(), 1);
    assert_eq!(body[0]["status"], "idle");
    assert_eq!(body[0]["active_sessions"], 0);
}

// --- Sessions (error cases without real agent) ---

#[tokio::test]
async fn test_create_session_invalid_agent() {
    let server = test_server();
    let resp = server
        .post("/api/v1/sessions")
        .json(&serde_json::json!({"agent": "nonexistent"}))
        .await;
    resp.assert_status(StatusCode::NOT_FOUND);
    let body: serde_json::Value = resp.json();
    assert_eq!(body["code"], "agent_not_found");
}

#[tokio::test]
async fn test_create_session_missing_agent() {
    let server = test_server();
    let resp = server
        .post("/api/v1/sessions")
        .json(&serde_json::json!({}))
        .await;
    resp.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_get_session_not_found() {
    let server = test_server();
    let resp = server.get("/api/v1/sessions/fake_id").await;
    resp.assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_delete_session_not_found() {
    let server = test_server();
    let resp = server.delete("/api/v1/sessions/fake_id").await;
    resp.assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_prompt_session_not_found() {
    let server = test_server();
    let resp = server
        .post("/api/v1/sessions/fake_id/prompt")
        .json(&serde_json::json!({"messages": []}))
        .await;
    resp.assert_status(StatusCode::NOT_FOUND);
}

// --- Usage ---

#[tokio::test]
async fn test_usage_empty() {
    let server = test_server();
    let resp = server.get("/api/v1/usage").await;
    resp.assert_status(StatusCode::OK);
    let body: Vec<serde_json::Value> = resp.json();
    assert!(body.is_empty());
}

// --- Webhook ---

#[tokio::test]
async fn test_webhook_ignored_event() {
    let server = test_server();
    let resp = server
        .post("/api/v1/webhooks/gitlab")
        .json(&serde_json::json!({"object_kind": "push"}))
        .await;
    resp.assert_status(StatusCode::OK);
    let body: serde_json::Value = resp.json();
    assert_eq!(body["status"], "ignored");
}

// --- Orchestrate (validation only) ---

#[tokio::test]
async fn test_orchestrate_invalid_agent() {
    let server = test_server();
    let resp = server
        .post("/api/v1/orchestrate")
        .json(&serde_json::json!({
            "agents": ["nonexistent"],
            "messages": [{"type": "text", "text": "hi"}]
        }))
        .await;
    resp.assert_status(StatusCode::NOT_FOUND);
}

// --- Pipeline (validation only) ---

#[tokio::test]
async fn test_pipeline_empty_steps() {
    let server = test_server();
    let resp = server
        .post("/api/v1/pipelines")
        .json(&serde_json::json!({
            "steps": [],
            "messages": [{"type": "text", "text": "hi"}]
        }))
        .await;
    resp.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_pipeline_invalid_agent() {
    let server = test_server();
    let resp = server
        .post("/api/v1/pipelines")
        .json(&serde_json::json!({
            "steps": [{"agent": "nonexistent"}],
            "messages": [{"type": "text", "text": "hi"}]
        }))
        .await;
    resp.assert_status(StatusCode::NOT_FOUND);
}

// --- Sessions list ---

#[tokio::test]
async fn test_list_sessions_empty() {
    let server = test_server();
    let resp = server.get("/api/v1/sessions").await;
    resp.assert_status(StatusCode::OK);
    let body: Vec<serde_json::Value> = resp.json();
    assert!(body.is_empty());
}
