#![allow(dead_code)]

use serde::Deserialize;

/// Routing configuration
#[derive(Debug, Default, Deserialize)]
pub struct RoutingConfig {
    #[serde(default)]
    pub default_agent: Option<String>,
    #[serde(default)]
    pub rules: Vec<RoutingRule>,
}

#[derive(Debug, Deserialize)]
pub struct RoutingRule {
    #[serde(rename = "match")]
    pub match_criteria: MatchCriteria,
    pub agent: String,
}

#[derive(Debug, Deserialize)]
pub struct MatchCriteria {
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub file_extensions: Vec<String>,
    #[serde(default)]
    pub workspace_pattern: Option<String>,
}

/// Router that selects the best agent based on rules
pub struct AgentRouter {
    config: RoutingConfig,
    available_agents: Vec<String>,
}

impl AgentRouter {
    pub fn new(config: RoutingConfig, available_agents: Vec<String>) -> Self {
        Self { config, available_agents }
    }

    /// Select an agent for a given prompt context
    pub fn route(&self, prompt_text: &str, workspace: Option<&str>) -> Option<&str> {
        // Evaluate rules in order
        for rule in &self.config.rules {
            if !self.available_agents.contains(&rule.agent) {
                continue;
            }
            if self.matches(&rule.match_criteria, prompt_text, workspace) {
                return Some(&rule.agent);
            }
        }

        // Fallback to default
        self.config
            .default_agent
            .as_deref()
            .filter(|a| self.available_agents.contains(&a.to_string()))
    }

    fn matches(&self, criteria: &MatchCriteria, prompt: &str, workspace: Option<&str>) -> bool {
        let prompt_lower = prompt.to_lowercase();

        // Keyword match
        if !criteria.keywords.is_empty()
            && criteria.keywords.iter().any(|k| prompt_lower.contains(&k.to_lowercase()))
        {
            return true;
        }

        // File extension match (check if prompt mentions files with these extensions)
        if !criteria.file_extensions.is_empty()
            && criteria.file_extensions.iter().any(|ext| prompt_lower.contains(ext))
        {
            return true;
        }

        // Workspace pattern match
        if let (Some(pattern), Some(ws)) = (&criteria.workspace_pattern, workspace) {
            return ws.starts_with(pattern.trim_end_matches("**").trim_end_matches('/'));
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> RoutingConfig {
        RoutingConfig {
            default_agent: Some("kiro".to_owned()),
            rules: vec![
                RoutingRule {
                    match_criteria: MatchCriteria {
                        keywords: vec!["python".to_owned(), "django".to_owned()],
                        file_extensions: vec![],
                        workspace_pattern: None,
                    },
                    agent: "codex".to_owned(),
                },
                RoutingRule {
                    match_criteria: MatchCriteria {
                        keywords: vec![],
                        file_extensions: vec![".rs".to_owned(), ".toml".to_owned()],
                        workspace_pattern: None,
                    },
                    agent: "claude".to_owned(),
                },
                RoutingRule {
                    match_criteria: MatchCriteria {
                        keywords: vec![],
                        file_extensions: vec![],
                        workspace_pattern: Some("/projects/frontend".to_owned()),
                    },
                    agent: "copilot".to_owned(),
                },
            ],
        }
    }

    fn available() -> Vec<String> {
        vec![
            "kiro".to_owned(),
            "codex".to_owned(),
            "claude".to_owned(),
            "copilot".to_owned(),
        ]
    }

    #[test]
    fn test_keyword_routing() {
        let router = AgentRouter::new(test_config(), available());
        assert_eq!(router.route("fix the Python Django model", None), Some("codex"));
    }

    #[test]
    fn test_file_extension_routing() {
        let router = AgentRouter::new(test_config(), available());
        assert_eq!(router.route("update Cargo.toml dependencies", None), Some("claude"));
    }

    #[test]
    fn test_workspace_routing() {
        let router = AgentRouter::new(test_config(), available());
        assert_eq!(
            router.route("fix the button", Some("/projects/frontend/app")),
            Some("copilot")
        );
    }

    #[test]
    fn test_default_fallback() {
        let router = AgentRouter::new(test_config(), available());
        assert_eq!(router.route("do something generic", None), Some("kiro"));
    }

    #[test]
    fn test_unavailable_agent_skipped() {
        let router = AgentRouter::new(test_config(), vec!["kiro".to_owned()]);
        // codex not available, should fall through to default
        assert_eq!(router.route("fix the Python code", None), Some("kiro"));
    }
}
