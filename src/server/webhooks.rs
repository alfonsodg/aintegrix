#![allow(dead_code)]

use std::time::Duration;

use hmac::{Hmac, Mac};
use serde::Serialize;
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Webhook event types
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    SessionCreated,
    SessionClosed,
    TurnCompleted,
    AgentFailed,
    AgentRestarted,
}

/// Webhook payload
#[derive(Debug, Clone, Serialize)]
pub struct WebhookPayload {
    pub event: EventType,
    pub timestamp: String,
    pub data: serde_json::Value,
}

/// Webhook target configuration
#[derive(Debug, Clone)]
pub struct WebhookTarget {
    pub url: String,
    pub secret: Option<String>,
    pub events: Vec<String>,
}

/// Dispatches webhook events to configured targets
pub struct WebhookDispatcher {
    targets: Vec<WebhookTarget>,
    client: reqwest::Client,
    max_retries: u32,
}

impl WebhookDispatcher {
    pub fn new(targets: Vec<WebhookTarget>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_default();

        Self { targets, client, max_retries: 3 }
    }

    /// Fire a webhook event asynchronously (non-blocking)
    pub fn fire(&self, event: EventType, data: serde_json::Value) {
        let payload = WebhookPayload {
            event: event.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            data,
        };

        let event_name = serde_json::to_value(&event)
            .ok()
            .and_then(|v| v.as_str().map(|s| s.to_owned()))
            .unwrap_or_default();

        for target in &self.targets {
            if !target.events.contains(&event_name) && !target.events.contains(&"*".to_owned()) {
                continue;
            }

            let client = self.client.clone();
            let url = target.url.clone();
            let secret = target.secret.clone();
            let payload = payload.clone();
            let max_retries = self.max_retries;

            tokio::spawn(async move {
                deliver(&client, &url, secret.as_deref(), &payload, max_retries).await;
            });
        }
    }
}

async fn deliver(
    client: &reqwest::Client,
    url: &str,
    secret: Option<&str>,
    payload: &WebhookPayload,
    max_retries: u32,
) {
    let body = serde_json::to_string(payload).unwrap_or_default();

    for attempt in 0..max_retries {
        let mut req = client.post(url).header("content-type", "application/json");

        if let Some(secret) = secret {
            let signature = compute_signature(secret, &body);
            req = req.header("x-signature", signature);
        }

        match req.body(body.clone()).send().await {
            Ok(resp) if resp.status().is_success() => return,
            Ok(resp) => {
                tracing::warn!(url, status = %resp.status(), attempt, "webhook delivery failed");
            }
            Err(e) => {
                tracing::warn!(url, error = %e, attempt, "webhook delivery error");
            }
        }

        // Exponential backoff
        tokio::time::sleep(Duration::from_secs(2u64.pow(attempt))).await;
    }

    tracing::error!(url, "webhook delivery exhausted retries");
}

fn compute_signature(secret: &str, body: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("valid key");
    mac.update(body.as_bytes());
    let result = mac.finalize();
    format!("sha256={}", hex::encode(result.into_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_signature() {
        let sig = compute_signature("mysecret", r#"{"event":"test"}"#);
        assert!(sig.starts_with("sha256="));
        assert_eq!(sig.len(), 7 + 64); // "sha256=" + 64 hex chars
    }

    #[test]
    fn test_dispatcher_creation() {
        let targets = vec![WebhookTarget {
            url: "http://localhost:9999/hook".to_owned(),
            secret: Some("secret".to_owned()),
            events: vec!["agent_failed".to_owned()],
        }];
        let dispatcher = WebhookDispatcher::new(targets);
        assert_eq!(dispatcher.max_retries, 3);
    }
}
