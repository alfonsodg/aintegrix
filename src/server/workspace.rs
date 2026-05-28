use std::path::{Path, PathBuf};

use crate::error::AppError;

const WORKSPACE_BASE: &str = "/tmp/aintegrix-ws";

fn git_host() -> String {
    std::env::var("GIT_HOST").unwrap_or_else(|_| "git@github.com".to_owned())
}

/// Validate repo string — only allow safe characters
fn validate_repo(repo: &str) -> Result<(), AppError> {
    if repo.starts_with('-') || repo.contains("..") {
        return Err(AppError::Agent(format!("invalid repo: '{repo}'")));
    }
    // Full URLs are allowed as-is
    if repo.contains('@') || repo.contains("://") {
        return Ok(());
    }
    // Short names: only alphanumeric, /, ., -, _
    if repo.chars().all(|c| c.is_alphanumeric() || "/.\\-_".contains(c)) {
        Ok(())
    } else {
        Err(AppError::Agent(format!("invalid repo characters: '{repo}'")))
    }
}

/// Clone a repo into a temporary workspace. Returns the path.
pub async fn clone_repo(repo: &str, branch: &str) -> Result<PathBuf, AppError> {
    validate_repo(repo)?;

    let id = uuid::Uuid::new_v4().to_string();
    let dest = PathBuf::from(format!("{WORKSPACE_BASE}/{id}"));

    tokio::fs::create_dir_all(&dest).await.map_err(|e| {
        AppError::Agent(format!("failed to create workspace dir: {e}"))
    })?;

    let url = if repo.contains('@') || repo.contains("://") {
        repo.to_owned()
    } else {
        format!("{}:{}.git", git_host(), repo)
    };

    let dest_str = dest.to_str().ok_or_else(|| {
        AppError::Agent("workspace path is not valid UTF-8".to_owned())
    })?;

    let output = tokio::process::Command::new("git")
        .args(["clone", "--depth", "1", "-b", branch, &url, dest_str])
        .output()
        .await
        .map_err(|e| AppError::Agent(format!("git clone failed: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let _ = tokio::fs::remove_dir_all(&dest).await;
        return Err(AppError::Agent(format!("git clone failed: {stderr}")));
    }

    tracing::info!(repo = %repo, branch = %branch, path = %dest.display(), "workspace cloned");
    Ok(dest)
}

/// Remove a workspace directory (path-safe check)
pub async fn cleanup(path: &str) {
    let base = Path::new(WORKSPACE_BASE);
    let target = Path::new(path);
    if target.starts_with(base) && target != base {
        let _ = tokio::fs::remove_dir_all(target).await;
        tracing::info!(path = %path, "workspace cleaned up");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_repos() {
        assert!(validate_repo("org/project").is_ok());
        assert!(validate_repo("git@github.com:org/repo.git").is_ok());
        assert!(validate_repo("https://github.com/org/repo").is_ok());
    }

    #[test]
    fn test_invalid_repos() {
        assert!(validate_repo("--upload-pack=evil").is_err());
        assert!(validate_repo("-c core.sshCommand=evil").is_err());
        assert!(validate_repo("../../etc/passwd").is_err());
        assert!(validate_repo("org/repo;rm -rf /").is_err());
    }

    #[test]
    fn test_git_host_default() {
        // Without env var set, defaults to github
        unsafe { std::env::remove_var("GIT_HOST") };
        assert_eq!(git_host(), "git@github.com");
    }
}
