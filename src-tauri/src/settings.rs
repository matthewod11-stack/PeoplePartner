// People Partner - Settings Module
// Generic key-value store for application settings
//
// Uses the existing `settings` table from 001_initial.sql:
//   CREATE TABLE settings (
//       key TEXT PRIMARY KEY,
//       value TEXT NOT NULL,
//       updated_at TEXT DEFAULT (datetime('now'))
//   );

use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use thiserror::Error;

use crate::db::DbPool;

// ============================================================================
// Error Types
// ============================================================================

#[derive(Error, Debug, Serialize)]
pub enum SettingsError {
    #[error("Database error: {0}")]
    Database(String),
}

impl From<sqlx::Error> for SettingsError {
    fn from(err: sqlx::Error) -> Self {
        SettingsError::Database(err.to_string())
    }
}

// ============================================================================
// Data Types
// ============================================================================

/// A single setting row from the database
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Setting {
    pub key: String,
    pub value: String,
    pub updated_at: String,
}

// ============================================================================
// CRUD Operations
// ============================================================================

/// Get a setting value by key
/// Returns None if the setting doesn't exist
pub async fn get_setting(pool: &DbPool, key: &str) -> Result<Option<String>, SettingsError> {
    let result: Option<(String,)> = sqlx::query_as(
        "SELECT value FROM settings WHERE key = ?"
    )
    .bind(key)
    .fetch_optional(pool)
    .await?;

    Ok(result.map(|(value,)| value))
}

/// Set a setting value (upsert - creates or updates)
pub async fn set_setting(pool: &DbPool, key: &str, value: &str) -> Result<(), SettingsError> {
    sqlx::query(
        r#"
        INSERT INTO settings (key, value, updated_at)
        VALUES (?, ?, datetime('now'))
        ON CONFLICT(key) DO UPDATE SET
            value = excluded.value,
            updated_at = datetime('now')
        "#
    )
    .bind(key)
    .bind(value)
    .execute(pool)
    .await?;

    Ok(())
}

/// Delete a setting by key
/// Does nothing if the setting doesn't exist
pub async fn delete_setting(pool: &DbPool, key: &str) -> Result<(), SettingsError> {
    sqlx::query("DELETE FROM settings WHERE key = ?")
        .bind(key)
        .execute(pool)
        .await?;

    Ok(())
}

/// Check if a setting exists
pub async fn has_setting(pool: &DbPool, key: &str) -> Result<bool, SettingsError> {
    let result: Option<(i64,)> = sqlx::query_as(
        "SELECT 1 FROM settings WHERE key = ?"
    )
    .bind(key)
    .fetch_optional(pool)
    .await?;

    Ok(result.is_some())
}

/// Get all settings (useful for debugging/settings panel)
pub async fn list_settings(pool: &DbPool) -> Result<Vec<Setting>, SettingsError> {
    let settings: Vec<Setting> = sqlx::query_as(
        "SELECT key, value, updated_at FROM settings ORDER BY key"
    )
    .fetch_all(pool)
    .await?;

    Ok(settings)
}

#[cfg(test)]
mod tests {
    // Integration tests would require database setup
    // Unit tests for this module are minimal since it's mostly DB operations
}
