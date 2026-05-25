use serde::Deserialize;

/// Routing configuration from YAML
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RoutingConfig {
    #[serde(default)]
    pub rules: Vec<RoutingRule>,
    #[serde(default)]
    pub fallback: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RoutingRule {
    #[serde(rename = "match")]
    pub matcher: RuleMatcher,
    pub agent: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RuleMatcher {
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub file_patterns: Vec<String>,
    #[serde(default)]
    pub task_type: Option<String>,
}

/// Resolve which agent to use based on routing rules
pub fn resolve_agent(
    config: &RoutingConfig,
    prompt_text: &str,
    file_paths: &[String],
    task_type: Option<&str>,
) -> Option<String> {
    let prompt_lower = prompt_text.to_lowercase();

    for rule in &config.rules {
        if matches_rule(&rule.matcher, &prompt_lower, file_paths, task_type) {
            tracing::info!(
                agent = %rule.agent,
                keywords = ?rule.matcher.keywords,
                "routing matched"
            );
            return Some(rule.agent.clone());
        }
    }

    config.fallback.clone()
}

fn matches_rule(
    matcher: &RuleMatcher,
    prompt_lower: &str,
    file_paths: &[String],
    task_type: Option<&str>,
) -> bool {
    // Task type match (exact)
    if matches!(&matcher.task_type, Some(rule_type) if task_type == Some(rule_type.as_str())) {
        return true;
    }

    // Keyword match (any keyword present in prompt)
    if !matcher.keywords.is_empty()
        && matcher.keywords.iter().any(|kw| prompt_lower.contains(&kw.to_lowercase()))
    {
        return true;
    }

    // File pattern match (glob-like: *.rs, Cargo.toml)
    if !matcher.file_patterns.is_empty() && !file_paths.is_empty() {
        for pattern in &matcher.file_patterns {
            for path in file_paths {
                if matches_glob(pattern, path) {
                    return true;
                }
            }
        }
    }

    false
}

fn matches_glob(pattern: &str, path: &str) -> bool {
    if pattern.starts_with("*.") {
        let ext = &pattern[1..]; // e.g. ".rs"
        path.ends_with(ext)
    } else {
        path.ends_with(pattern) || path.contains(pattern)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> RoutingConfig {
        RoutingConfig {
            rules: vec![
                RoutingRule {
                    matcher: RuleMatcher {
                        keywords: vec!["frontend".to_owned(), "react".to_owned(), "css".to_owned()],
                        file_patterns: vec![],
                        task_type: None,
                    },
                    agent: "copilot".to_owned(),
                },
                RoutingRule {
                    matcher: RuleMatcher {
                        keywords: vec![],
                        file_patterns: vec!["*.rs".to_owned(), "Cargo.toml".to_owned()],
                        task_type: None,
                    },
                    agent: "kiro".to_owned(),
                },
                RoutingRule {
                    matcher: RuleMatcher {
                        keywords: vec!["review".to_owned(), "security".to_owned()],
                        file_patterns: vec![],
                        task_type: None,
                    },
                    agent: "claude".to_owned(),
                },
                RoutingRule {
                    matcher: RuleMatcher {
                        keywords: vec![],
                        file_patterns: vec![],
                        task_type: Some("refactor".to_owned()),
                    },
                    agent: "opencode".to_owned(),
                },
            ],
            fallback: Some("kiro".to_owned()),
        }
    }

    #[test]
    fn test_keyword_match() {
        let cfg = test_config();
        let result = resolve_agent(&cfg, "fix the React component", &[], None);
        assert_eq!(result, Some("copilot".to_owned()));
    }

    #[test]
    fn test_file_pattern_match() {
        let cfg = test_config();
        let files = vec!["src/main.rs".to_owned()];
        let result = resolve_agent(&cfg, "fix this", &files, None);
        assert_eq!(result, Some("kiro".to_owned()));
    }

    #[test]
    fn test_task_type_match() {
        let cfg = test_config();
        let result = resolve_agent(&cfg, "do it", &[], Some("refactor"));
        assert_eq!(result, Some("opencode".to_owned()));
    }

    #[test]
    fn test_fallback() {
        let cfg = test_config();
        let result = resolve_agent(&cfg, "something random", &[], None);
        assert_eq!(result, Some("kiro".to_owned()));
    }

    #[test]
    fn test_security_keyword() {
        let cfg = test_config();
        let result = resolve_agent(&cfg, "review this code for security issues", &[], None);
        assert_eq!(result, Some("claude".to_owned()));
    }
}
