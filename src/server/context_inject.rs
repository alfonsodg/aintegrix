#![allow(dead_code)]

use std::path::Path;

use serde::Deserialize;

/// Config for context injection per agent
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ContextInjectConfig {
    #[serde(default)]
    pub files: Vec<String>,
}

/// Read and concatenate context files, skipping missing ones
pub fn load_context(config: &ContextInjectConfig, workspace: &str) -> Option<String> {
    if config.files.is_empty() {
        return None;
    }

    let mut context = String::new();
    for file_path in &config.files {
        let resolved = resolve_path(file_path, workspace);
        if let Ok(content) = std::fs::read_to_string(&resolved) {
            context.push_str(&format!("--- {} ---\n{}\n\n", file_path, content.trim()));
        } else {
            tracing::debug!(path = %resolved, "context file not found, skipping");
        }
    }

    if context.is_empty() { None } else { Some(context) }
}

fn resolve_path(path: &str, workspace: &str) -> String {
    let expanded = shellexpand::tilde(path).to_string();
    if Path::new(&expanded).is_absolute() {
        expanded
    } else {
        format!("{workspace}/{expanded}")
    }
}
