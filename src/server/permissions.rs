#![allow(dead_code)]

use crate::config::types::PermissionsConfig;

/// Result of a permission evaluation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionDecision {
    Approved,
    Denied,
    PendingApproval,
}

/// Evaluates permission requests against declarative YAML policies
pub struct PermissionEngine {
    config: PermissionsConfig,
}

impl PermissionEngine {
    pub fn new(config: PermissionsConfig) -> Self {
        Self { config }
    }

    /// Evaluate a permission request for a given method and optional path
    pub fn evaluate(&self, method: &str, _path: Option<&str>) -> PermissionDecision {
        // Check always_deny first (highest priority)
        if self.config.always_deny.iter().any(|m| m == method) {
            return PermissionDecision::Denied;
        }

        // Check auto_approve
        if self.config.auto_approve.iter().any(|m| m == method) {
            return PermissionDecision::Approved;
        }

        // Check require_approval
        if self.config.require_approval.iter().any(|m| m == method) {
            return PermissionDecision::PendingApproval;
        }

        // Fall through to default policy
        match self.config.default_policy.as_str() {
            "approve" | "allow" => PermissionDecision::Approved,
            "deny" => PermissionDecision::Denied,
            _ => PermissionDecision::Denied,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> PermissionsConfig {
        PermissionsConfig {
            default_policy: "deny".to_owned(),
            auto_approve: vec!["fs/read_text_file".to_owned()],
            require_approval: vec!["fs/write_text_file".to_owned(), "terminal/create".to_owned()],
            always_deny: vec!["terminal/kill".to_owned()],
        }
    }

    #[test]
    fn test_auto_approve() {
        let engine = PermissionEngine::new(test_config());
        assert_eq!(engine.evaluate("fs/read_text_file", None), PermissionDecision::Approved);
    }

    #[test]
    fn test_always_deny() {
        let engine = PermissionEngine::new(test_config());
        assert_eq!(engine.evaluate("terminal/kill", None), PermissionDecision::Denied);
    }

    #[test]
    fn test_require_approval() {
        let engine = PermissionEngine::new(test_config());
        assert_eq!(
            engine.evaluate("fs/write_text_file", None),
            PermissionDecision::PendingApproval
        );
    }

    #[test]
    fn test_default_deny() {
        let engine = PermissionEngine::new(test_config());
        assert_eq!(engine.evaluate("unknown/method", None), PermissionDecision::Denied);
    }

    #[test]
    fn test_default_approve_policy() {
        let config = PermissionsConfig {
            default_policy: "approve".to_owned(),
            auto_approve: vec![],
            require_approval: vec![],
            always_deny: vec!["terminal/kill".to_owned()],
        };
        let engine = PermissionEngine::new(config);
        assert_eq!(engine.evaluate("anything", None), PermissionDecision::Approved);
        // always_deny still overrides
        assert_eq!(engine.evaluate("terminal/kill", None), PermissionDecision::Denied);
    }
}
