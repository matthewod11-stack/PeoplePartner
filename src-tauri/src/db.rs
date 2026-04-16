// People Partner - Database Module
// SQLite connection management and migrations

use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous,
};
use sqlx::{Pool, Sqlite};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
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
pub fn get_db_path(app: &AppHandle) -> DbResult<PathBuf> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| DbError::Migration(format!("Failed to get app data directory: {}", e)))?;

    // Ensure directory exists
    fs::create_dir_all(&app_data_dir)?;

    Ok(app_data_dir.join("hr_command_center.db"))
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

/// Build SQLite connect options with the PRAGMAs we require everywhere.
///
/// `foreign_keys` and `busy_timeout` are connection-scoped in SQLite, so they
/// must be applied per-connection. `SqliteConnectOptions` handles that via an
/// internal after-connect hook. `journal_mode` (WAL) is database-scoped and
/// persists across connections once set.
fn connect_options(path: &Path) -> SqliteConnectOptions {
    SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        // Without this, every ON DELETE CASCADE / SET NULL in our migrations is
        // silently ignored. SQLite defaults this PRAGMA to OFF.
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(Duration::from_secs(5))
}

/// Initialize the database connection pool
pub async fn init_db(app: &AppHandle) -> DbResult<DbPool> {
    let db_path = get_db_path(app)?;
    harden_db_file_permissions(&db_path)?;

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(connect_options(&db_path))
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
        include_str!("../migrations/007_documents.sql"),
        include_str!("../migrations/008_document_chunks_unique.sql"),
        include_str!("../migrations/009_license_validation_cache.sql"),
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
                    // Only suppress "already exists" for ALTER TABLE statements;
                    // CREATE TABLE should use IF NOT EXISTS, so propagate those errors.
                    if let Err(e) = result {
                        let err_str = e.to_string().to_lowercase();
                        let stmt_upper = stmt_without_semi.trim().to_uppercase();
                        let is_alter_table = stmt_upper.starts_with("ALTER TABLE");
                        let is_duplicate_column = err_str.contains("duplicate column");
                        let is_already_exists = err_str.contains("already exists");

                        if is_alter_table && (is_duplicate_column || is_already_exists) {
                            // Expected on re-runs: ALTER TABLE ADD COLUMN for columns that already exist
                        } else {
                            return Err(DbError::Migration(format!(
                                "Failed to execute: {}\nError: {}",
                                stmt_without_semi.chars().take(100).collect::<String>(),
                                e
                            )));
                        }
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
    use sqlx::Row;

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

    /// Build an in-memory test pool using the same connect_options as production,
    /// except WAL mode (not supported on :memory: databases).
    async fn test_pool() -> DbPool {
        let options = SqliteConnectOptions::new()
            .filename(":memory:")
            .create_if_missing(true)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(5));

        let pool = SqlitePoolOptions::new()
            .max_connections(1) // :memory: is per-connection; must be 1 to share state
            .connect_with(options)
            .await
            .expect("connect to :memory: pool");

        run_migrations(&pool).await.expect("run migrations");
        pool
    }

    #[tokio::test]
    async fn pragma_foreign_keys_is_on() {
        let pool = test_pool().await;

        let row = sqlx::query("PRAGMA foreign_keys")
            .fetch_one(&pool)
            .await
            .expect("query PRAGMA foreign_keys");
        let enabled: i64 = row.get(0);
        assert_eq!(
            enabled, 1,
            "foreign_keys PRAGMA must be ON; otherwise every CASCADE is silently ignored"
        );
    }

    #[tokio::test]
    async fn cascade_delete_fires_on_employee_delete() {
        let pool = test_pool().await;

        // Insert an employee and a dependent enps_response (FK → employees ON DELETE CASCADE)
        sqlx::query(
            "INSERT INTO employees (id, email, full_name) VALUES ('emp-1', 'a@b.com', 'Test Employee')",
        )
        .execute(&pool)
        .await
        .expect("insert employee");

        sqlx::query(
            "INSERT INTO enps_responses (id, employee_id, score, survey_date) \
             VALUES ('enps-1', 'emp-1', 8, '2026-01-01')",
        )
        .execute(&pool)
        .await
        .expect("insert enps_response");

        let before: i64 = sqlx::query("SELECT COUNT(*) FROM enps_responses WHERE employee_id = 'emp-1'")
            .fetch_one(&pool)
            .await
            .unwrap()
            .get(0);
        assert_eq!(before, 1);

        // Deleting the employee should CASCADE-delete the enps_response.
        // Before this PR (foreign_keys OFF by default), the response would remain orphaned.
        sqlx::query("DELETE FROM employees WHERE id = 'emp-1'")
            .execute(&pool)
            .await
            .expect("delete employee");

        let after: i64 = sqlx::query("SELECT COUNT(*) FROM enps_responses WHERE employee_id = 'emp-1'")
            .fetch_one(&pool)
            .await
            .unwrap()
            .get(0);
        assert_eq!(
            after, 0,
            "CASCADE delete did not fire — foreign_keys PRAGMA likely not enforced"
        );
    }
}
