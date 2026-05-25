#![allow(dead_code)]

use std::collections::HashMap;
use std::process::Stdio;
use std::time::Duration;

use tokio::process::{Child, ChildStdin, Command};
use tokio::sync::mpsc;

use crate::config::types::AgentConfig;
use crate::error::AppError;
use crate::protocol::transport::{read_loop, send_notification, send_request};
use crate::protocol::types::{Notification, RequestId, Response};

/// Represents a running agent subprocess
pub struct AgentProcess {
    pub name: String,
    pub child: Child,
    pub stdin: ChildStdin,
    pub response_rx: mpsc::UnboundedReceiver<Response>,
    pub notification_rx: mpsc::UnboundedReceiver<Notification>,
}

impl AgentProcess {
    /// Spawn an agent subprocess from config
    pub async fn spawn(name: &str, config: &AgentConfig) -> Result<Self, AppError> {
        let mut cmd = Command::new(&config.command);
        cmd.args(&config.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Inject environment variables
        for (key, value) in &config.env {
            cmd.env(key, value);
        }

        let mut child = cmd.spawn().map_err(|e| {
            AppError::Agent(format!("failed to spawn agent '{name}': {e}"))
        })?;

        let stdin = child.stdin.take().ok_or_else(|| {
            AppError::Agent(format!("failed to capture stdin for agent '{name}'"))
        })?;

        let stdout = child.stdout.take().ok_or_else(|| {
            AppError::Agent(format!("failed to capture stdout for agent '{name}'"))
        })?;

        // Spawn stderr logger
        if let Some(stderr) = child.stderr.take() {
            let agent_name = name.to_owned();
            tokio::spawn(async move {
                use tokio::io::{AsyncBufReadExt, BufReader};
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    tracing::debug!(agent = %agent_name, stderr = %line);
                }
            });
        }

        // Spawn stdout read loop
        let (response_tx, response_rx) = mpsc::unbounded_channel();
        let (notification_tx, notification_rx) = mpsc::unbounded_channel();
        tokio::spawn(read_loop(stdout, response_tx, notification_tx));

        Ok(Self { name: name.to_owned(), child, stdin, response_rx, notification_rx })
    }

    /// Check if the agent process is still alive
    pub fn is_alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// Send a JSON-RPC request and wait for response
    pub async fn request(
        &mut self,
        method: &str,
        params: Option<serde_json::Value>,
        timeout: Duration,
    ) -> Result<Response, AppError> {
        let id = send_request(&mut self.stdin, method, params).await?;
        crate::protocol::transport::wait_for_response(
            &mut self.response_rx,
            RequestId::Number(id),
            timeout,
        )
        .await
    }

    /// Send a notification (no response expected)
    pub async fn notify(
        &mut self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<(), AppError> {
        send_notification(&mut self.stdin, method, params).await
    }

    /// Graceful shutdown: close stdin, wait, then kill
    pub async fn shutdown(mut self) {
        // Close stdin to signal EOF
        drop(self.stdin);

        // Wait up to 5 seconds for graceful exit
        let result = tokio::time::timeout(Duration::from_secs(5), self.child.wait()).await;

        if result.is_err() {
            tracing::warn!(agent = %self.name, "agent did not exit gracefully, killing");
            let _ = self.child.kill().await;
        }
    }
}

/// Registry of all configured agents and their spawn configs
pub struct AgentRegistry {
    pub configs: HashMap<String, AgentConfig>,
}

impl AgentRegistry {
    pub fn from_config(agents: HashMap<String, AgentConfig>) -> Self {
        Self { configs: agents }
    }

    pub fn agent_names(&self) -> Vec<&str> {
        self.configs.keys().map(|s| s.as_str()).collect()
    }
}
