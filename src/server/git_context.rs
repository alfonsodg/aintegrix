use std::path::Path;
use std::process::Command;

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct GitContext {
    pub branch: String,
    pub recent_commits: Vec<String>,
    pub diff_summary: String,
}

/// Extract git context from a directory. Returns None if not a git repo.
pub fn extract(cwd: &str) -> Option<GitContext> {
    let path = Path::new(cwd);
    if !path.join(".git").exists() && !is_inside_git_repo(cwd) {
        return None;
    }

    let branch = run_git(cwd, &["rev-parse", "--abbrev-ref", "HEAD"])
        .unwrap_or_else(|| "unknown".to_owned());

    let commits_raw = run_git(cwd, &["log", "--oneline", "-5", "--no-decorate"])
        .unwrap_or_default();
    let recent_commits: Vec<String> = commits_raw.lines().map(|l| l.to_owned()).collect();

    let diff_summary = run_git(cwd, &["diff", "--stat", "HEAD"])
        .unwrap_or_default();

    Some(GitContext { branch, recent_commits, diff_summary })
}

/// Format git context as text for injection into agent prompt
pub fn format_for_prompt(ctx: &GitContext) -> String {
    let mut out = String::new();
    out.push_str(&format!("Branch: {}\n", ctx.branch));
    if !ctx.recent_commits.is_empty() {
        out.push_str("Recent commits:\n");
        for c in &ctx.recent_commits {
            out.push_str(&format!("  {c}\n"));
        }
    }
    if !ctx.diff_summary.is_empty() {
        out.push_str(&format!("Uncommitted changes:\n{}\n", ctx.diff_summary));
    }
    out
}

fn is_inside_git_repo(cwd: &str) -> bool {
    run_git(cwd, &["rev-parse", "--is-inside-work-tree"])
        .map(|s| s.trim() == "true")
        .unwrap_or(false)
}

fn run_git(cwd: &str, args: &[&str]) -> Option<String> {
    Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_current_repo() {
        // This test runs inside the aintegrix repo
        let ctx = extract(env!("CARGO_MANIFEST_DIR"));
        assert!(ctx.is_some());
        let ctx = ctx.unwrap();
        assert!(!ctx.branch.is_empty());
        assert!(!ctx.recent_commits.is_empty());
    }

    #[test]
    fn test_extract_non_repo() {
        let ctx = extract("/tmp");
        assert!(ctx.is_none());
    }

    #[test]
    fn test_format() {
        let ctx = GitContext {
            branch: "develop".to_owned(),
            recent_commits: vec!["abc1234 fix: something".to_owned()],
            diff_summary: " 2 files changed".to_owned(),
        };
        let text = format_for_prompt(&ctx);
        assert!(text.contains("develop"));
        assert!(text.contains("abc1234"));
    }
}
