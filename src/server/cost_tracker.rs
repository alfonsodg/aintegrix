use std::sync::Arc;

use axum::extract::{Query, State};
use axum::Json;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};

use crate::server::routes::AppState;

/// In-memory usage record
#[derive(Debug, Clone, Serialize)]
pub struct UsageRecord {
    pub agent: String,
    pub model: String,
    pub session_id: String,
    pub prompts: u32,
    pub timestamp: String,
}

/// Shared usage store (added to AppState)
pub type UsageStore = DashMap<String, Vec<UsageRecord>>;

/// Record a prompt execution
pub fn record_usage(store: &UsageStore, agent: &str, model: &str, session_id: &str) {
    let record = UsageRecord {
        agent: agent.to_owned(),
        model: model.to_owned(),
        session_id: session_id.to_owned(),
        prompts: 1,
        timestamp: chrono::Utc::now().to_rfc3339(),
    };
    store.entry(agent.to_owned()).or_default().push(record);
}

#[derive(Deserialize)]
pub struct UsageQuery {
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub since: Option<String>,
}

#[derive(Serialize)]
pub struct UsageSummary {
    pub agent: String,
    pub total_prompts: u32,
    pub sessions: u32,
}

pub async fn get_usage(
    State(state): State<Arc<AppState>>,
    Query(query): Query<UsageQuery>,
) -> Json<Vec<UsageSummary>> {
    let mut summaries = Vec::new();

    for entry in state.usage.iter() {
        let agent = entry.key().clone();
        if let Some(ref filter) = query.agent
            && &agent != filter
        {
            continue;
        }

        let records = entry.value();
        let filtered: Vec<&UsageRecord> = if let Some(ref since) = query.since {
            records.iter().filter(|r| r.timestamp.as_str() >= since.as_str()).collect()
        } else {
            records.iter().collect()
        };

        let total_prompts = filtered.len() as u32;
        let sessions: std::collections::HashSet<&str> =
            filtered.iter().map(|r| r.session_id.as_str()).collect();

        summaries.push(UsageSummary {
            agent,
            total_prompts,
            sessions: sessions.len() as u32,
        });
    }

    Json(summaries)
}
