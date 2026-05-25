use std::path::PathBuf;

use crate::error::AppError;

const WORKSPACE_BASE: &str = "/tmp/aintegrix-ws";
const GITLAB_HOST: &str = "git@scovil.labtau.com";

/// Clone a repo into a temporary workspace. Returns the path.
pub async fn clone_repo(repo: &str, branch: &str) -> Result<PathBuf, AppError> {
    let id = uuid::Uuid::new_v4().to_string();
    let dest = PathBuf::from(format!("{WORKSPACE_BASE}/{id}"));

    tokio::fs::create_dir_all(&dest).await.map_err(|e| {
        AppError::Agent(format!("failed to create workspace dir: {e}"))
    })?;

    let url = if repo.contains('@') || repo.contains("://") {
        repo.to_owned()
    } else {
        format!("{GITLAB_HOST}:{repo}.git")
    };

    let output = tokio::process::Command::new("git")
        .args(["clone", "--depth", "1", "-b", branch, &url, dest.to_str().unwrap()])
        .output()
        .await
        .map_err(|e| AppError::Agent(format!("git clone failed: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Cleanup on failure
        let _ = tokio::fs::remove_dir_all(&dest).await;
        return Err(AppError::Agent(format!("git clone failed: {stderr}")));
    }

    tracing::info!(repo = %repo, branch = %branch, path = %dest.display(), "workspace cloned");
    Ok(dest)
}

/// Remove a workspace directory
pub async fn cleanup(path: &str) {
    if path.starts_with(WORKSPACE_BASE) {
        let _ = tokio::fs::remove_dir_all(path).await;
        tracing::info!(path = %path, "workspace cleaned up");
    }
}
