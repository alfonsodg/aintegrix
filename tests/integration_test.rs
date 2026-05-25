use std::collections::HashMap;
use std::time::Duration;

use aintegrix::agent::process::AgentProcess;
use aintegrix::agent::session;
use aintegrix::config::types::AgentConfig;

fn mock_agent_config() -> AgentConfig {
    AgentConfig {
        command: env!("CARGO_BIN_EXE_mock_agent").to_owned(),
        args: vec![],
        mode: "native".to_owned(),
        max_sessions: 3,
        auto_restart: false,
        env: HashMap::new(),
        default_model: None,
        models: vec![],
        prompt_prefix: None,
        prompt_suffix: None,
        context_inject: Default::default(),
    }
}

#[tokio::test]
async fn test_initialize_handshake() {
    let config = mock_agent_config();
    let mut agent = AgentProcess::spawn("mock", &config).await.unwrap();

    let result = session::initialize(&mut agent).await.unwrap();
    assert_eq!(result.protocol_version, "1.0");
    assert!(result.capabilities.load_session);

    agent.shutdown().await;
}

#[tokio::test]
async fn test_session_new() {
    let config = mock_agent_config();
    let mut agent = AgentProcess::spawn("mock", &config).await.unwrap();

    session::initialize(&mut agent).await.unwrap();
    let session_id = session::session_new(&mut agent, "/tmp/test").await.unwrap();
    assert_eq!(session_id, "mock-session-001");

    agent.shutdown().await;
}

#[tokio::test]
async fn test_session_prompt() {
    let config = mock_agent_config();
    let mut agent = AgentProcess::spawn("mock", &config).await.unwrap();

    session::initialize(&mut agent).await.unwrap();
    session::session_new(&mut agent, "/tmp/test").await.unwrap();

    let messages = vec![serde_json::json!({
        "role": "user",
        "content": [{"type": "text", "text": "hello"}]
    })];

    let stop_reason =
        session::session_prompt(&mut agent, "mock-session-001", messages, Duration::from_secs(5))
            .await
            .unwrap();

    assert_eq!(stop_reason, "end_turn");
    agent.shutdown().await;
}
