#![allow(dead_code)]

use std::sync::Arc;

use dashmap::DashMap;
use tokio::sync::broadcast;

/// Session status
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionStatus {
    Active,
    Prompting,
    Idle,
    Closed,
}

/// Tracked session state
#[derive(Debug, Clone)]
pub struct ManagedSession {
    pub id: String,
    pub agent_name: String,
    pub agent_session_id: Option<String>,
    pub status: SessionStatus,
    pub workspace_root: String,
    pub update_tx: broadcast::Sender<String>,
}

/// Manages all active sessions with concurrency limits
pub struct SessionManager {
    sessions: DashMap<String, ManagedSession>,
    /// Count of active sessions per agent
    agent_session_count: DashMap<String, u32>,
    /// Max sessions per agent (from config)
    agent_limits: DashMap<String, u32>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: DashMap::new(),
            agent_session_count: DashMap::new(),
            agent_limits: DashMap::new(),
        }
    }

    /// Set the max_sessions limit for an agent
    pub fn set_agent_limit(&self, agent_name: &str, max_sessions: u32) {
        self.agent_limits.insert(agent_name.to_owned(), max_sessions);
    }

    /// Try to create a new session, respecting concurrency limits
    pub fn create_session(
        &self,
        id: String,
        agent_name: &str,
        workspace_root: &str,
    ) -> Result<broadcast::Receiver<String>, String> {
        // Check limit
        let limit = self.agent_limits.get(agent_name).map(|v| *v).unwrap_or(3);
        let current = self.agent_session_count.get(agent_name).map(|v| *v).unwrap_or(0);

        if current >= limit {
            return Err(format!(
                "agent '{agent_name}' at max sessions ({limit})"
            ));
        }

        let (tx, rx) = broadcast::channel(256);

        let session = ManagedSession {
            id: id.clone(),
            agent_name: agent_name.to_owned(),
            agent_session_id: None,
            status: SessionStatus::Active,
            workspace_root: workspace_root.to_owned(),
            update_tx: tx,
        };

        self.sessions.insert(id, session);
        *self.agent_session_count.entry(agent_name.to_owned()).or_insert(0) += 1;

        Ok(rx)
    }

    /// Get a broadcast receiver for session updates
    pub fn subscribe(&self, session_id: &str) -> Option<broadcast::Receiver<String>> {
        self.sessions.get(session_id).map(|s| s.update_tx.subscribe())
    }

    /// Publish an update to all subscribers of a session
    pub fn publish_update(&self, session_id: &str, update: String) {
        if let Some(session) = self.sessions.get(session_id) {
            let _ = session.update_tx.send(update);
        }
    }

    /// Close a session and decrement the agent count
    pub fn close_session(&self, session_id: &str) {
        if let Some((_, session)) = self.sessions.remove(session_id) {
            self.agent_session_count
                .entry(session.agent_name)
                .and_modify(|count| *count = count.saturating_sub(1));
        }
    }

    /// Get session info
    pub fn get(&self, session_id: &str) -> Option<ManagedSession> {
        self.sessions.get(session_id).map(|s| s.clone())
    }

    /// List all active sessions
    pub fn list(&self) -> Vec<ManagedSession> {
        self.sessions.iter().map(|entry| entry.value().clone()).collect()
    }

    /// Count active sessions for an agent
    pub fn agent_count(&self, agent_name: &str) -> u32 {
        self.agent_session_count.get(agent_name).map(|v| *v).unwrap_or(0)
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Wrap in Arc for shared state
pub type SharedSessionManager = Arc<SessionManager>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_session() {
        let mgr = SessionManager::new();
        mgr.set_agent_limit("kiro", 2);

        let rx = mgr.create_session("s1".to_owned(), "kiro", "/tmp");
        assert!(rx.is_ok());
        assert_eq!(mgr.agent_count("kiro"), 1);
    }

    #[test]
    fn test_max_sessions_enforced() {
        let mgr = SessionManager::new();
        mgr.set_agent_limit("kiro", 1);

        mgr.create_session("s1".to_owned(), "kiro", "/tmp").unwrap();
        let result = mgr.create_session("s2".to_owned(), "kiro", "/tmp");
        assert!(result.is_err());
    }

    #[test]
    fn test_close_decrements_count() {
        let mgr = SessionManager::new();
        mgr.set_agent_limit("kiro", 5);

        mgr.create_session("s1".to_owned(), "kiro", "/tmp").unwrap();
        assert_eq!(mgr.agent_count("kiro"), 1);

        mgr.close_session("s1");
        assert_eq!(mgr.agent_count("kiro"), 0);
    }

    #[test]
    fn test_publish_and_subscribe() {
        let mgr = SessionManager::new();
        let _rx = mgr.create_session("s1".to_owned(), "kiro", "/tmp").unwrap();

        let mut rx2 = mgr.subscribe("s1").unwrap();
        mgr.publish_update("s1", "hello".to_owned());

        let msg = rx2.try_recv().unwrap();
        assert_eq!(msg, "hello");
    }
}
