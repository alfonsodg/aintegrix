use std::path::{Path, PathBuf};

use crate::error::AppError;

/// Handles fs/read_text_file requests from agents
pub async fn read_text_file(workspace_root: &str, path: &str) -> Result<String, AppError> {
    let resolved = resolve_safe_path(workspace_root, path)?;
    tokio::fs::read_to_string(&resolved)
        .await
        .map_err(AppError::Io)
}

/// Handles fs/write_text_file requests from agents
pub async fn write_text_file(
    workspace_root: &str,
    path: &str,
    content: &str,
) -> Result<(), AppError> {
    let resolved = resolve_safe_path(workspace_root, path)?;

    // Ensure parent directory exists
    if let Some(parent) = resolved.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    tokio::fs::write(&resolved, content).await.map_err(AppError::Io)
}

/// Resolve a path ensuring it stays within workspace_root (prevent traversal)
fn resolve_safe_path(workspace_root: &str, file_path: &str) -> Result<PathBuf, AppError> {
    let root = Path::new(workspace_root).canonicalize().map_err(|e| {
        AppError::Agent(format!("invalid workspace root: {e}"))
    })?;

    let target = root.join(file_path);
    let resolved = target.canonicalize().unwrap_or(target);

    // Ensure the resolved path is within workspace_root
    if !resolved.starts_with(&root) {
        return Err(AppError::Agent(format!(
            "path traversal denied: '{file_path}' escapes workspace"
        )));
    }

    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_read_write_file() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().to_str().unwrap();

        write_text_file(root, "test.txt", "hello world").await.unwrap();
        let content = read_text_file(root, "test.txt").await.unwrap();
        assert_eq!(content, "hello world");
    }

    #[tokio::test]
    async fn test_path_traversal_blocked() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().to_str().unwrap();

        // Create a file first so canonicalize works
        write_text_file(root, "legit.txt", "ok").await.unwrap();

        let result = read_text_file(root, "../../etc/passwd").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_nested_directory_creation() {
        let dir = TempDir::new().unwrap();
        let root = dir.path().to_str().unwrap();

        write_text_file(root, "sub/dir/file.txt", "nested").await.unwrap();
        let content = read_text_file(root, "sub/dir/file.txt").await.unwrap();
        assert_eq!(content, "nested");
    }
}
