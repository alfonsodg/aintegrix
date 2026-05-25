#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use tokio::sync::Mutex;

use crate::agent::process::AgentProcess;
use crate::agent::session as acp;
use crate::config::types::AgentConfig;
use crate::error::AppError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentStatus {
    Starting,
    Ready,
    Failed,
    Stopped,
}

struct AgentEntry {
    process: AgentProcess,
    status: AgentStatus,
    restart_count: u32,
}

/// Manages all agent subprocesses with health checks and auto-restart
pub struct AgentPool {
    agents: DashMap<String, Arc<Mutex<AgentEntry>>>,
    configs: HashMap<String, AgentConfig>,
    max_retries: u32,
}

impl AgentPool {
    pub fn new(configs: HashMap<String, AgentConfig>) -> Self {
        Self {
            agents: DashMap::new(),
            configs,
            max_retries: 5,
        }
    }

    /// Start all configured agents
    pub async fn start_all(&self) -> Vec<String> {
        let mut started = Vec::new();
        for (name, config) in &self.configs {
            match self.spawn_agent(name, config).await {
                Ok(()) => {
                    tracing::info!(agent = %name, "agent started");
                    started.push(name.clone());
                }
                Err(e) => {
                    tracing::error!(agent = %name, error = %e, "failed to start agent");
                }
            }
        }
        started
    }

    async fn spawn_agent(&self, name: &str, config: &AgentConfig) -> Result<(), AppError> {
        let mut process = AgentProcess::spawn(name, config).await?;

        // Perform ACP initialize handshake
        let init_result = acp::initialize(&mut process).await?;
        tracing::info!(
            agent = %name,
            protocol_version = %init_result.protocol_version,
            "agent initialized"
        );

        let entry = AgentEntry {
            process,
            status: AgentStatus::Ready,
            restart_count: 0,
        };

        self.agents.insert(name.to_owned(), Arc::new(Mutex::new(entry)));
        Ok(())
    }

    /// Check health of all agents and restart dead ones
    pub async fn health_check(&self) {
        let names: Vec<String> = self.agents.iter().map(|e| e.key().clone()).collect();

        for name in names {
            let should_restart = {
                let entry_arc = match self.agents.get(&name) {
                    Some(e) => e.value().clone(),
                    None => continue,
                };
                let mut entry = entry_arc.lock().await;

                if !entry.process.is_alive() {
                    tracing::warn!(agent = %name, "agent process died");
                    entry.status = AgentStatus::Failed;
                    true
                } else {
                    false
                }
            };

            if should_restart {
                self.try_restart(&name).await;
            }
        }
    }

    async fn try_restart(&self, name: &str) {
        let config = match self.configs.get(name) {
            Some(c) => c,
            None => return,
        };

        if !config.auto_restart {
            return;
        }

        let restart_count = {
            let entry_arc = match self.agents.get(name) {
                Some(e) => e.value().clone(),
                None => return,
            };
            let entry = entry_arc.lock().await;
            entry.restart_count
        };

        if restart_count >= self.max_retries {
            tracing::error!(agent = %name, retries = restart_count, "max retries reached, agent marked failed");
            return;
        }

        // Exponential backoff
        let delay = Duration::from_secs(2u64.pow(restart_count.min(4)));
        tokio::time::sleep(delay).await;

        tracing::info!(agent = %name, attempt = restart_count + 1, "restarting agent");

        match self.spawn_agent(name, config).await {
            Ok(()) => {
                if let Some(entry_arc) = self.agents.get(name) {
                    let mut entry = entry_arc.lock().await;
                    entry.restart_count = restart_count + 1;
                }
            }
            Err(e) => {
                tracing::error!(agent = %name, error = %e, "restart failed");
            }
        }
    }

    /// Get agent status
    pub async fn status(&self, name: &str) -> Option<AgentStatus> {
        let entry_arc = self.agents.get(name)?.value().clone();
        let entry = entry_arc.lock().await;
        Some(entry.status.clone())
    }

    /// Get all agent statuses
    pub async fn all_statuses(&self) -> HashMap<String, AgentStatus> {
        let mut result = HashMap::new();
        for entry in self.agents.iter() {
            let name = entry.key().clone();
            let arc = entry.value().clone();
            let e = arc.lock().await;
            result.insert(name, e.status.clone());
        }
        result
    }

    /// Shutdown all agents gracefully
    pub async fn shutdown_all(&self) {
        let names: Vec<String> = self.agents.iter().map(|e| e.key().clone()).collect();
        for name in names {
            if let Some((_, arc)) = self.agents.remove(&name) {
                let mut entry = arc.lock().await;
                tracing::info!(agent = %name, "shutting down agent");
                let _ = entry.process.child.kill().await;
                entry.status = AgentStatus::Stopped;
            }
        }
    }

    /// Spawn the health check background task
    pub fn spawn_health_loop(pool: Arc<Self>, interval: Duration) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                ticker.tick().await;
                pool.health_check().await;
            }
        })
    }
}
