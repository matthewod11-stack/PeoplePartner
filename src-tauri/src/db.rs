// HR Command Center - Database Module
// SQLite connection management and migrations

use sqlx::{sqlite::SqlitePoolOptions, Pool, Sqlite};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DbError {
    #[error("Database error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("Migration error: {0}")]
    Migration(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type DbPool = Pool<Sqlite>;
pub type DbResult<T> = Result<T, DbError>;

/// Get the database file path in the app data directory
pub fn get_db_path(app: &AppHandle) -> PathBuf {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .expect("Failed to get app data directory");

    // Ensure directory exists
    fs::create_dir_all(&app_data_dir).expect("Failed to create app data directory");

    app_data_dir.join("hr_command_center.db")
}

fn apply_restrictive_permissions(path: &PathBuf) -> std::io::Result<()> {
    // Ensure the file exists before applying file permissions.
    if !path.exists() {
        fs::File::create(path)?;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }

    Ok(())
}

fn harden_db_file_permissions(db_path: &PathBuf) -> std::io::Result<()> {
    apply_restrictive_permissions(db_path)?;

    // SQLite sidecar files may be created at runtime depending on journaling mode.
    // If they exist, align permissions with the main DB file.
    let wal_path = PathBuf::from(format!("{}-wal", db_path.to_string_lossy()));
    if wal_path.exists() {
        apply_restrictive_permissions(&wal_path)?;
    }

    let shm_path = PathBuf::from(format!("{}-shm", db_path.to_string_lossy()));
    if shm_path.exists() {
        apply_restrictive_permissions(&shm_path)?;
    }

    Ok(())
}

/// Initialize the database connection pool
pub async fn init_db(app: &AppHandle) -> DbResult<DbPool> {
    let db_path = get_db_path(app);
    harden_db_file_permissions(&db_path)?;
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await?;

    // Run migrations
    run_migrations(&pool).await?;
    harden_db_file_permissions(&db_path)?;

    Ok(pool)
}

/// Run database migrations
async fn run_migrations(pool: &DbPool) -> DbResult<()> {
    // Migration files in order
    let migrations = [
        include_str!("../migrations/001_initial.sql"),
        include_str!("../migrations/002_performance_enps.sql"),
        include_str!("../migrations/003_review_highlights.sql"),
        include_str!("../migrations/004_insight_canvas.sql"),
        include_str!("../migrations/005_dei_audit.sql"),
        include_str!("../migrations/006_drop_insight_canvas.sql"),
    ];

    for migration_sql in migrations {
        run_migration_sql(pool, migration_sql).await?;
    }

    Ok(())
}

/// Execute a single migration file's SQL statements
async fn run_migration_sql(pool: &DbPool, migration_sql: &str) -> DbResult<()> {
    // Parse statements carefully - handle BEGIN...END blocks (triggers)
    // These blocks contain semicolons that shouldn't split the statement
    let mut current_statement = String::new();
    let mut inside_begin_block = false;

    for line in migration_sql.lines() {
        let trimmed = line.trim();
        let upper = trimmed.to_uppercase();

        // Skip empty lines and comments
        if trimmed.is_empty() || trimmed.starts_with("--") {
            continue;
        }

        current_statement.push_str(line);
        current_statement.push('\n');

        // Track BEGIN...END blocks (used in triggers)
        if upper.contains(" BEGIN") || upper.ends_with(" BEGIN") {
            inside_begin_block = true;
        }

        // Check if this line ends a statement
        let is_end_of_block = upper.starts_with("END;") || upper == "END";

        if is_end_of_block && inside_begin_block {
            inside_begin_block = false;
        }

        // Only execute when we have a complete statement:
        // - Line ends with semicolon AND
        // - We're not inside a BEGIN...END block
        if trimmed.ends_with(';') && !inside_begin_block {
            let stmt = current_statement.trim();
            if !stmt.is_empty() {
                // Remove trailing semicolon for SQLx
                let stmt_without_semi = stmt.trim_end_matches(';').trim();
                if !stmt_without_semi.is_empty() {
                    let result = sqlx::query(stmt_without_semi).execute(pool).await;

                    // Handle expected errors gracefully:
                    // - "duplicate column" for ALTER TABLE ADD COLUMN (already exists)
                    // - "table already exists" (should be covered by IF NOT EXISTS, but just in case)
                    if let Err(e) = result {
                        let err_str = e.to_string().to_lowercase();
                        let is_duplicate_column = err_str.contains("duplicate column");
                        let is_table_exists = err_str.contains("already exists");

                        if !is_duplicate_column && !is_table_exists {
                            return Err(DbError::Migration(format!(
                                "Failed to execute: {}\nError: {}",
                                stmt_without_semi.chars().take(100).collect::<String>(),
                                e
                            )));
                        }
                        // Otherwise, silently continue - migration already applied
                    }
                }
            }
            current_statement.clear();
        }
    }

    Ok(())
}

/// Database state managed by Tauri
pub struct Database {
    pub pool: DbPool,
}

impl Database {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_migration_sql_is_valid() {
        // This test just ensures the SQL file can be included and parsed
        let sql = include_str!("../migrations/001_initial.sql");
        assert!(!sql.is_empty());
        assert!(sql.contains("CREATE TABLE"));
        assert!(sql.contains("employees"));
        assert!(sql.contains("conversations"));
        assert!(sql.contains("company"));
        assert!(sql.contains("settings"));
        assert!(sql.contains("audit_log"));
        assert!(sql.contains("conversations_fts"));
    }
}
