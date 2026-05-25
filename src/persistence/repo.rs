#![allow(dead_code)]

use chrono::Utc;
use sqlx::sqlite::SqlitePool;
use uuid::Uuid;

use crate::error::AppError;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SessionRow {
    pub id: String,
    pub agent_name: String,
    pub agent_session_id: Option<String>,
    pub status: String,
    pub workspace_root: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub closed_at: Option<String>,
}

pub struct SessionRepo {
    pool: SqlitePool,
}

impl SessionRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        agent_name: &str,
        workspace_root: Option<&str>,
    ) -> Result<SessionRow, AppError> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT INTO sessions (id, agent_name, status, workspace_root, created_at, updated_at)
             VALUES (?, ?, 'active', ?, ?, ?)",
        )
        .bind(&id)
        .bind(agent_name)
        .bind(workspace_root)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::Agent(format!("failed to create session: {e}")))?;

        Ok(SessionRow {
            id,
            agent_name: agent_name.to_owned(),
            agent_session_id: None,
            status: "active".to_owned(),
            workspace_root: workspace_root.map(|s| s.to_owned()),
            created_at: now.clone(),
            updated_at: now,
            closed_at: None,
        })
    }

    pub async fn get(&self, id: &str) -> Result<Option<SessionRow>, AppError> {
        sqlx::query_as::<_, SessionRow>("SELECT * FROM sessions WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| AppError::Agent(format!("failed to get session: {e}")))
    }

    pub async fn list_active(&self) -> Result<Vec<SessionRow>, AppError> {
        sqlx::query_as::<_, SessionRow>("SELECT * FROM sessions WHERE status = 'active'")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| AppError::Agent(format!("failed to list sessions: {e}")))
    }

    pub async fn close(&self, id: &str) -> Result<(), AppError> {
        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE sessions SET status = 'closed', closed_at = ?, updated_at = ? WHERE id = ?")
            .bind(&now)
            .bind(&now)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| AppError::Agent(format!("failed to close session: {e}")))?;
        Ok(())
    }

    pub async fn update_agent_session_id(
        &self,
        id: &str,
        agent_session_id: &str,
    ) -> Result<(), AppError> {
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "UPDATE sessions SET agent_session_id = ?, updated_at = ? WHERE id = ?",
        )
        .bind(agent_session_id)
        .bind(&now)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::Agent(format!("failed to update session: {e}")))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        crate::persistence::run_migrations(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn test_create_and_get_session() {
        let pool = test_pool().await;
        let repo = SessionRepo::new(pool);

        let session = repo.create("kiro", Some("/tmp/test")).await.unwrap();
        assert_eq!(session.agent_name, "kiro");
        assert_eq!(session.status, "active");

        let found = repo.get(&session.id).await.unwrap().unwrap();
        assert_eq!(found.id, session.id);
    }

    #[tokio::test]
    async fn test_list_active() {
        let pool = test_pool().await;
        let repo = SessionRepo::new(pool);

        repo.create("kiro", None).await.unwrap();
        repo.create("codex", None).await.unwrap();

        let active = repo.list_active().await.unwrap();
        assert_eq!(active.len(), 2);
    }

    #[tokio::test]
    async fn test_close_session() {
        let pool = test_pool().await;
        let repo = SessionRepo::new(pool);

        let session = repo.create("kiro", None).await.unwrap();
        repo.close(&session.id).await.unwrap();

        let found = repo.get(&session.id).await.unwrap().unwrap();
        assert_eq!(found.status, "closed");
        assert!(found.closed_at.is_some());
    }
}
