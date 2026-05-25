#![allow(dead_code)]

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Simple in-process metrics registry (Prometheus-compatible text output)
pub struct Metrics {
    pub http_requests_total: AtomicU64,
    pub sessions_created_total: AtomicU64,
    pub prompts_total: AtomicU64,
    pub agent_restarts_total: AtomicU64,
}

impl Metrics {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            http_requests_total: AtomicU64::new(0),
            sessions_created_total: AtomicU64::new(0),
            prompts_total: AtomicU64::new(0),
            agent_restarts_total: AtomicU64::new(0),
        })
    }

    pub fn inc_http_requests(&self) {
        self.http_requests_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_sessions_created(&self) {
        self.sessions_created_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_prompts(&self) {
        self.prompts_total.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_agent_restarts(&self) {
        self.agent_restarts_total.fetch_add(1, Ordering::Relaxed);
    }

    /// Render metrics in Prometheus text exposition format
    pub fn render(&self) -> String {
        format!(
            "# HELP aintegrix_http_requests_total Total HTTP requests\n\
             # TYPE aintegrix_http_requests_total counter\n\
             aintegrix_http_requests_total {}\n\
             # HELP aintegrix_sessions_created_total Total sessions created\n\
             # TYPE aintegrix_sessions_created_total counter\n\
             aintegrix_sessions_created_total {}\n\
             # HELP aintegrix_prompts_total Total prompts sent\n\
             # TYPE aintegrix_prompts_total counter\n\
             aintegrix_prompts_total {}\n\
             # HELP aintegrix_agent_restarts_total Total agent restarts\n\
             # TYPE aintegrix_agent_restarts_total counter\n\
             aintegrix_agent_restarts_total {}\n",
            self.http_requests_total.load(Ordering::Relaxed),
            self.sessions_created_total.load(Ordering::Relaxed),
            self.prompts_total.load(Ordering::Relaxed),
            self.agent_restarts_total.load(Ordering::Relaxed),
        )
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self {
            http_requests_total: AtomicU64::new(0),
            sessions_created_total: AtomicU64::new(0),
            prompts_total: AtomicU64::new(0),
            agent_restarts_total: AtomicU64::new(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_increment_and_render() {
        let m = Metrics::new();
        m.inc_http_requests();
        m.inc_http_requests();
        m.inc_prompts();

        let output = m.render();
        assert!(output.contains("aintegrix_http_requests_total 2"));
        assert!(output.contains("aintegrix_prompts_total 1"));
        assert!(output.contains("aintegrix_sessions_created_total 0"));
    }
}
