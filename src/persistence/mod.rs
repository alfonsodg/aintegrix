#![allow(dead_code)]

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};

use crate::error::AppError;

pub mod repo;

/// Initialize the SQLite database with WAL mode and run migrations
pub async fn init_pool(db_path: &str) -> Result<SqlitePool, AppError> {
    let url = format!("sqlite:{db_path}?mode=rwc");

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .map_err(|e| AppError::Config(format!("database connection failed: {e}")))?;

    // Enable WAL mode
    sqlx::query("PRAGMA journal_mode=WAL")
        .execute(&pool)
        .await
        .map_err(|e| AppError::Config(format!("failed to set WAL mode: {e}")))?;

    // Run migrations
    run_migrations(&pool).await?;

    Ok(pool)
}

async fn run_migrations(pool: &SqlitePool) -> Result<(), AppError> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS sessions (
            id TEXT PRIMARY KEY,
            agent_name TEXT NOT NULL,
            agent_session_id TEXT,
            status TEXT NOT NULL DEFAULT 'active',
            workspace_root TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            closed_at TEXT
        )",
    )
    .execute(pool)
    .await
    .map_err(|e| AppError::Config(format!("migration failed: {e}")))?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS turns (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL REFERENCES sessions(id),
            request_id TEXT,
            prompt_text TEXT,
            stop_reason TEXT,
            started_at TEXT NOT NULL,
            completed_at TEXT,
            updates_count INTEGER DEFAULT 0
        )",
    )
    .execute(pool)
    .await
    .map_err(|e| AppError::Config(format!("migration failed: {e}")))?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS agent_state (
            agent_name TEXT PRIMARY KEY,
            status TEXT NOT NULL DEFAULT 'stopped',
            pid INTEGER,
            capabilities_json TEXT,
            last_health_check TEXT,
            started_at TEXT,
            error_count INTEGER DEFAULT 0
        )",
    )
    .execute(pool)
    .await
    .map_err(|e| AppError::Config(format!("migration failed: {e}")))?;

    Ok(())
}
