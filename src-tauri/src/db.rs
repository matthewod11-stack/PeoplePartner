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

/// SQLite filename used by every released version. Kept as a constant so
/// the sandbox-data migration code can reference it without duplicating
/// the literal.
pub const DB_FILENAME: &str = "hr_command_center.db";

/// Bundle identifier — pinned because the legacy sandbox container path
/// is named after it. Changing this would orphan every customer's data.
const BUNDLE_ID: &str = "com.peoplepartner.app";

/// Get the database file path in the app data directory.
///
/// Also runs the one-shot v0.2.0 → v0.2.2+ sandbox-to-userlib data
/// migration if applicable (see `migrate_legacy_sandbox_data_if_needed`).
/// Migration is best-effort and silent: failures fall through to the
/// fresh-install code path so the app always starts.
pub fn get_db_path(app: &AppHandle) -> DbResult<PathBuf> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| DbError::Migration(format!("Failed to get app data directory: {}", e)))?;

    // Ensure directory exists
    fs::create_dir_all(&app_data_dir)?;

    // Recover orphaned data left in the legacy sandbox container by signed
    // v0.2.0 installs (see issue #18 / v0.2.3 release notes). No-op if the
    // new path already has data, or if there's no legacy container to read.
    if let Err(e) = migrate_legacy_sandbox_data_if_needed(&app_data_dir, DB_FILENAME) {
        log::warn!(
            "sandbox-data migration failed; continuing with fresh data dir: {e}"
        );
    }

    Ok(app_data_dir.join(DB_FILENAME))
}

/// Compute the legacy sandbox-container path that mirrors `app_data_dir`.
///
/// macOS sandbox containers nest the user's home tree under
/// `~/Library/Containers/{bundle_id}/Data/`, so an app whose data dir is
/// `$HOME/Library/Application Support/{bundle_id}` had data at
/// `$HOME/Library/Containers/{bundle_id}/Data/Library/Application Support/{bundle_id}`
/// while sandboxed.
///
/// Returns None on non-macOS platforms, or if `app_data_dir` is not under
/// `$HOME` (which would mean we can't compute a sensible mirror path).
fn legacy_sandbox_data_dir(app_data_dir: &Path) -> Option<PathBuf> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    let home = std::env::var_os("HOME")?;
    let home = PathBuf::from(home);
    let inside_home = app_data_dir.strip_prefix(&home).ok()?;
    Some(
        home.join("Library")
            .join("Containers")
            .join(BUNDLE_ID)
            .join("Data")
            .join(inside_home),
    )
}

/// One-shot migration: copy v0.2.0's sandbox-container data to the new
/// non-sandboxed location.
///
/// Why copy instead of move: leaves the sandbox container untouched as a
/// safety backup. If a future migration step is wrong, the original data
/// is still recoverable from `~/Library/Containers/{bundle_id}/Data/...`.
/// Disk cost is a few MB; the trade-off is overwhelmingly worth it.
///
/// Skip conditions (any one of these → no-op):
///   - new path already contains a DB (already migrated, or fresh install
///     in progress)
///   - not running on macOS
///   - HOME isn't set (shouldn't happen in practice)
///   - legacy sandbox path doesn't exist (truly fresh install)
///
/// Sidecar handling: SQLite WAL/SHM files are also copied if present.
/// They may exist if v0.2.0 last shut down without checkpointing.
///
/// Returns Ok(true) if migration ran, Ok(false) if skipped. Errors propagate
/// only if a copy starts and fails partway — caller should treat that as
/// recoverable (delete the partial new file, fall through to fresh install).
fn migrate_legacy_sandbox_data_if_needed(
    app_data_dir: &Path,
    db_filename: &str,
) -> std::io::Result<bool> {
    let new_db = app_data_dir.join(db_filename);
    if new_db.exists() {
        return Ok(false);
    }
    let Some(legacy_dir) = legacy_sandbox_data_dir(app_data_dir) else {
        return Ok(false);
    };
    let legacy_db = legacy_dir.join(db_filename);
    if !legacy_db.exists() {
        return Ok(false);
    }

    // Migrate. Create the new dir first (caller has already done so for
    // app_data_dir, but this function is also unit-testable in isolation).
    fs::create_dir_all(app_data_dir)?;
    fs::copy(&legacy_db, &new_db)?;

    // Best-effort sidecar copy. If a sidecar copy fails, log and continue —
    // SQLite will recover from a missing -wal/-shm on next open.
    for ext in &["-wal", "-shm"] {
        let sidecar_name = format!("{db_filename}{ext}");
        let legacy_sidecar = legacy_dir.join(&sidecar_name);
        if legacy_sidecar.exists() {
            if let Err(e) = fs::copy(&legacy_sidecar, app_data_dir.join(&sidecar_name)) {
                log::warn!("failed to copy sandbox sidecar {sidecar_name}: {e}");
            }
        }
    }

    log::info!(
        "Migrated legacy sandbox data: {} → {}",
        legacy_dir.display(),
        app_data_dir.display()
    );
    Ok(true)
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

/// Test-only accessor so sibling modules can build a fully-migrated in-memory pool.
#[cfg(test)]
pub(crate) async fn run_migrations_for_tests(pool: &DbPool) -> DbResult<()> {
    run_migrations(pool).await
}

/// Ordered migration inventory: (version, short_name, embedded_sql).
/// Versions must be dense and monotonically increasing from 1.
const MIGRATIONS: &[(i64, &str, &str)] = &[
    (1, "initial", include_str!("../migrations/001_initial.sql")),
    (2, "performance_enps", include_str!("../migrations/002_performance_enps.sql")),
    (3, "review_highlights", include_str!("../migrations/003_review_highlights.sql")),
    (4, "insight_canvas", include_str!("../migrations/004_insight_canvas.sql")),
    (5, "dei_audit", include_str!("../migrations/005_dei_audit.sql")),
    (6, "drop_insight_canvas", include_str!("../migrations/006_drop_insight_canvas.sql")),
    (7, "documents", include_str!("../migrations/007_documents.sql")),
    (8, "document_chunks_unique", include_str!("../migrations/008_document_chunks_unique.sql")),
    (9, "license_validation_cache", include_str!("../migrations/009_license_validation_cache.sql")),
    (10, "schema_migrations", include_str!("../migrations/010_schema_migrations.sql")),
    (11, "audit_log_append_only", include_str!("../migrations/011_audit_log_append_only.sql")),
    (12, "license_signed_token", include_str!("../migrations/012_license_signed_token.sql")),
];

/// Highest version that the pre-versioning runner may have applied. Used only
/// for the one-time legacy-DB backfill; bumping this would re-mark newer
/// migrations as "already applied" and is almost never what you want.
const LEGACY_LAST_VERSION: i64 = 9;

/// Run database migrations.
///
/// Versioned via `schema_migrations`. Each migration runs inside a transaction,
/// so a mid-file failure rolls back cleanly instead of leaving the DB in a
/// half-migrated state. Already-applied versions are skipped.
///
/// On first run against a DB created by the pre-versioning runner (identified
/// by the presence of tables from migration 001 without any `schema_migrations`
/// rows), versions 1..=LEGACY_LAST_VERSION are backfilled as applied before the
/// loop — otherwise every CREATE TABLE in migration 001 would fail.
async fn run_migrations(pool: &DbPool) -> DbResult<()> {
    bootstrap_schema_migrations_table(pool).await?;

    if is_legacy_unversioned_db(pool).await? {
        backfill_legacy_versions(pool).await?;
    }

    let applied = applied_versions(pool).await?;

    for (version, name, sql) in MIGRATIONS {
        if applied.contains(version) {
            continue;
        }
        apply_migration(pool, *version, name, sql).await?;
    }

    Ok(())
}

/// Create the `schema_migrations` table if missing. Runs outside any migration
/// transaction — idempotent, safe on every startup.
async fn bootstrap_schema_migrations_table(pool: &DbPool) -> DbResult<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            applied_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await
    .map_err(|e| DbError::Migration(format!("Bootstrap schema_migrations: {}", e)))?;
    Ok(())
}

/// Legacy DB = application tables from migration 001 exist, but
/// `schema_migrations` is empty. That state can only arise when the old
/// (pre-versioning) runner populated the DB and we just created
/// `schema_migrations` for the first time.
async fn is_legacy_unversioned_db(pool: &DbPool) -> DbResult<bool> {
    let (migration_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM schema_migrations")
            .fetch_one(pool)
            .await?;
    if migration_count > 0 {
        return Ok(false);
    }

    // `audit_log` is created in migration 001 and is never dropped. Its
    // presence on an empty `schema_migrations` is the legacy signal.
    let (audit_log_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'audit_log'",
    )
    .fetch_one(pool)
    .await?;

    Ok(audit_log_count > 0)
}

/// Record versions 1..=LEGACY_LAST_VERSION as applied in a single transaction.
/// Called exactly once, on first startup after upgrading to the versioned
/// runner against a pre-existing DB.
async fn backfill_legacy_versions(pool: &DbPool) -> DbResult<()> {
    let mut tx = pool.begin().await?;
    for (version, name, _) in MIGRATIONS.iter().filter(|(v, _, _)| *v <= LEGACY_LAST_VERSION) {
        sqlx::query("INSERT INTO schema_migrations (version, name) VALUES (?, ?)")
            .bind(version)
            .bind(name)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Fetch the set of already-applied migration versions.
async fn applied_versions(pool: &DbPool) -> DbResult<std::collections::HashSet<i64>> {
    let rows: Vec<(i64,)> = sqlx::query_as("SELECT version FROM schema_migrations")
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(|(v,)| v).collect())
}

/// Execute one migration atomically: all statements in one transaction, plus
/// the `schema_migrations` row. Any statement failing rolls the whole
/// transaction back — the DB is never left half-migrated.
async fn apply_migration(pool: &DbPool, version: i64, name: &str, sql: &str) -> DbResult<()> {
    let statements = split_sql_statements(sql);
    let mut tx = pool.begin().await?;

    for stmt in &statements {
        sqlx::query(stmt)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                DbError::Migration(format!(
                    "Migration {} ({}) failed on statement: {}\nError: {}",
                    version,
                    name,
                    stmt.chars().take(100).collect::<String>(),
                    e
                ))
            })?;
    }

    sqlx::query("INSERT INTO schema_migrations (version, name) VALUES (?, ?)")
        .bind(version)
        .bind(name)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok(())
}

/// Split a migration file into individual SQL statements.
///
/// Semicolons inside trigger `BEGIN...END` blocks don't terminate statements,
/// so we track nesting. Empty lines and `--` comments are discarded.
fn split_sql_statements(sql: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut inside_begin_block = false;

    for line in sql.lines() {
        let trimmed = line.trim();
        let upper = trimmed.to_uppercase();

        if trimmed.is_empty() || trimmed.starts_with("--") {
            continue;
        }

        current.push_str(line);
        current.push('\n');

        if upper.contains(" BEGIN") || upper.ends_with(" BEGIN") {
            inside_begin_block = true;
        }

        let is_end_of_block = upper.starts_with("END;") || upper == "END";
        if is_end_of_block && inside_begin_block {
            inside_begin_block = false;
        }

        if trimmed.ends_with(';') && !inside_begin_block {
            let stmt = current.trim().trim_end_matches(';').trim().to_string();
            if !stmt.is_empty() {
                statements.push(stmt);
            }
            current.clear();
        }
    }

    statements
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

    // ========================================================================
    // Versioned-migration runner tests (issue #26).
    // Cover the three properties the rewrite has to preserve:
    //   1. schema_migrations is populated after a fresh install
    //   2. run_migrations is idempotent (second call is a no-op)
    //   3. a failing migration rolls back cleanly (no partial DDL, no row)
    //   4. legacy DBs are backfilled rather than re-run
    // ========================================================================

    /// Empty :memory: pool with the same connect_options as production (minus WAL,
    /// unsupported on :memory:). Does NOT run migrations — callers drive the
    /// runner directly to test its behavior.
    async fn empty_pool() -> DbPool {
        let options = SqliteConnectOptions::new()
            .filename(":memory:")
            .create_if_missing(true)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(5));

        SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("connect to :memory: pool")
    }

    #[tokio::test]
    async fn fresh_install_records_every_migration() {
        let pool = empty_pool().await;
        run_migrations(&pool).await.expect("fresh migration run");

        let versions: Vec<i64> = sqlx::query_scalar("SELECT version FROM schema_migrations ORDER BY version")
            .fetch_all(&pool)
            .await
            .expect("read schema_migrations");

        let expected: Vec<i64> = MIGRATIONS.iter().map(|(v, _, _)| *v).collect();
        assert_eq!(versions, expected, "every migration in MIGRATIONS must appear exactly once");
    }

    #[tokio::test]
    async fn second_run_is_noop() {
        let pool = empty_pool().await;
        run_migrations(&pool).await.expect("first run");
        run_migrations(&pool).await.expect("second run must succeed");

        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM schema_migrations")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, MIGRATIONS.len() as i64, "second run must not duplicate rows");
    }

    #[tokio::test]
    async fn failed_migration_rolls_back_ddl_and_row() {
        let pool = empty_pool().await;
        bootstrap_schema_migrations_table(&pool).await.unwrap();

        // First statement creates a table; second is invalid. If the transaction
        // honors atomicity, the `rollback_me` table must not exist afterwards
        // and no row should appear in schema_migrations.
        let bad_sql = "CREATE TABLE rollback_me (id INTEGER);\nNOT VALID SQL;";
        let result = apply_migration(&pool, 9999, "bad_fixture", bad_sql).await;
        assert!(result.is_err(), "bad migration must surface as an error");

        let (table_count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'rollback_me'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(table_count, 0, "partial DDL must be rolled back");

        let (row_count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM schema_migrations WHERE version = 9999")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(row_count, 0, "schema_migrations row must not be committed on failure");
    }

    #[tokio::test]
    async fn legacy_db_is_backfilled_not_rerun() {
        let pool = empty_pool().await;

        // Simulate a DB left by the pre-versioning runner: audit_log exists
        // (with the post-005 production shape, since that's what any real
        // pre-upgrade DB would have) but schema_migrations does not. Later
        // migrations like 011 read from audit_log, so the schema must match
        // or the post-backfill migrations will fail. If the runner mistakenly
        // tries to re-run 001 we'll see a CREATE TABLE IF NOT EXISTS no-op
        // followed by ALTER TABLE in 002 failing with "duplicate column"
        // (the error-swallow that used to hide this is now gone).
        sqlx::query(
            "CREATE TABLE audit_log (
                id TEXT PRIMARY KEY,
                conversation_id TEXT,
                request_redacted TEXT NOT NULL,
                response_text TEXT NOT NULL,
                context_used TEXT,
                created_at TEXT DEFAULT (datetime('now')),
                query_category TEXT
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        // license_validation_cache is created by migration 009 (pre-legacy);
        // migration 012 ALTERs it, so it must exist for that ALTER to
        // succeed when we skip 009 via backfill.
        sqlx::query(
            "CREATE TABLE license_validation_cache (
                license_key TEXT PRIMARY KEY,
                device_id TEXT NOT NULL,
                validated_at TEXT NOT NULL,
                server_status TEXT NOT NULL,
                created_at TEXT DEFAULT (datetime('now')),
                updated_at TEXT DEFAULT (datetime('now'))
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        // Seed one row so we can prove migration 011 (which rebuilds the
        // table) preserves data rather than silently wiping it.
        sqlx::query(
            "INSERT INTO audit_log (id, request_redacted, response_text, created_at)
             VALUES ('legacy-row', 'req', 'resp', datetime('now'))",
        )
        .execute(&pool)
        .await
        .unwrap();

        run_migrations(&pool).await.expect("legacy path must not re-run 001");

        // Versions 1..=LEGACY_LAST_VERSION must be marked applied (backfilled),
        // and migrations LEGACY_LAST_VERSION+1 onward must have run fresh.
        let versions: Vec<i64> = sqlx::query_scalar("SELECT version FROM schema_migrations ORDER BY version")
            .fetch_all(&pool)
            .await
            .unwrap();
        let expected: Vec<i64> = MIGRATIONS.iter().map(|(v, _, _)| *v).collect();
        assert_eq!(versions, expected, "backfill + newer migrations must populate every version");

        // `employees` table (created by 001) must NOT exist — proves backfill
        // really skipped 001 instead of executing it.
        let (employees_count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'employees'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(employees_count, 0, "001 was re-run instead of backfilled");

        // Migration 011's rebuild must preserve existing audit rows.
        let (legacy_row_count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM audit_log WHERE id = 'legacy-row'")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(legacy_row_count, 1, "migration 011 must not drop existing audit rows");
    }

    #[tokio::test]
    async fn backfill_detection_skips_fresh_install() {
        let pool = empty_pool().await;
        bootstrap_schema_migrations_table(&pool).await.unwrap();

        assert!(
            !is_legacy_unversioned_db(&pool).await.unwrap(),
            "empty DB must not be treated as legacy"
        );
    }

    // ========================================================================
    // Sandbox-data migration tests (v0.2.3 / issue #18 follow-up).
    // The actual migration code is path-driven, so these tests work with
    // tempdirs and don't require a real sandbox container.
    // ========================================================================

    #[test]
    fn migration_copies_db_when_new_path_empty_and_legacy_has_data() {
        let tmp = tempfile::tempdir().unwrap();
        let app_data = tmp.path().join("new");
        let legacy = tmp.path().join("legacy");
        fs::create_dir_all(&app_data).unwrap();
        fs::create_dir_all(&legacy).unwrap();
        fs::write(legacy.join("hr_command_center.db"), b"v0.2.0 db payload").unwrap();
        fs::write(legacy.join("hr_command_center.db-wal"), b"wal").unwrap();

        // Run migration with explicit paths (so the test doesn't depend on
        // the cfg!(macos) check inside legacy_sandbox_data_dir).
        copy_legacy_data(&legacy, &app_data, "hr_command_center.db").unwrap();

        let migrated = fs::read(app_data.join("hr_command_center.db")).unwrap();
        assert_eq!(migrated, b"v0.2.0 db payload");
        let migrated_wal = fs::read(app_data.join("hr_command_center.db-wal")).unwrap();
        assert_eq!(migrated_wal, b"wal");
    }

    #[test]
    fn migration_skips_when_new_path_has_db() {
        let tmp = tempfile::tempdir().unwrap();
        let app_data = tmp.path().join("new");
        fs::create_dir_all(&app_data).unwrap();
        fs::write(app_data.join("hr_command_center.db"), b"existing v0.2.2 db").unwrap();

        let ran = migrate_legacy_sandbox_data_if_needed(&app_data, "hr_command_center.db").unwrap();
        assert!(!ran, "must skip when new path already has a DB");

        // Existing DB must NOT be overwritten.
        let still_there = fs::read(app_data.join("hr_command_center.db")).unwrap();
        assert_eq!(still_there, b"existing v0.2.2 db");
    }

    #[test]
    fn migration_skips_when_legacy_path_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let app_data = tmp.path().join("new");
        fs::create_dir_all(&app_data).unwrap();

        // Without HOME or sandbox container, the migration is a no-op.
        let ran = migrate_legacy_sandbox_data_if_needed(&app_data, "hr_command_center.db").unwrap();
        assert!(!ran, "must skip on fresh install with no sandbox container");
        assert!(!app_data.join("hr_command_center.db").exists());
    }

    #[test]
    fn migration_leaves_sandbox_container_untouched() {
        // Copy-not-move guarantee: if a future migration version does the
        // wrong thing, the user's original data is still in the sandbox.
        let tmp = tempfile::tempdir().unwrap();
        let app_data = tmp.path().join("new");
        let legacy = tmp.path().join("legacy");
        fs::create_dir_all(&app_data).unwrap();
        fs::create_dir_all(&legacy).unwrap();
        fs::write(legacy.join("hr_command_center.db"), b"original").unwrap();

        copy_legacy_data(&legacy, &app_data, "hr_command_center.db").unwrap();

        // Source untouched
        assert!(legacy.join("hr_command_center.db").exists());
        let original = fs::read(legacy.join("hr_command_center.db")).unwrap();
        assert_eq!(original, b"original");
    }

    /// Test helper: bypasses the cfg!(macos) and HOME-derivation logic so
    /// tests can drive the copy operation directly with arbitrary paths.
    fn copy_legacy_data(
        legacy_dir: &Path,
        new_dir: &Path,
        db_filename: &str,
    ) -> std::io::Result<()> {
        fs::create_dir_all(new_dir)?;
        fs::copy(legacy_dir.join(db_filename), new_dir.join(db_filename))?;
        for ext in &["-wal", "-shm"] {
            let sidecar = format!("{db_filename}{ext}");
            let src = legacy_dir.join(&sidecar);
            if src.exists() {
                fs::copy(&src, new_dir.join(&sidecar))?;
            }
        }
        Ok(())
    }

    #[tokio::test]
    async fn split_sql_preserves_trigger_begin_end_blocks() {
        // Regression guard: the FTS triggers in 001_initial use multi-line
        // BEGIN...END blocks with internal semicolons. The splitter must
        // treat each trigger as a single statement.
        let sql = "CREATE TABLE t (id INT);\n\
                   CREATE TRIGGER trg AFTER INSERT ON t BEGIN\n\
                   INSERT INTO t VALUES (1);\n\
                   INSERT INTO t VALUES (2);\n\
                   END;";
        let stmts = split_sql_statements(sql);
        assert_eq!(stmts.len(), 2, "trigger body must not be split: got {:?}", stmts);
        assert!(stmts[1].contains("BEGIN"));
        assert!(stmts[1].contains("INSERT INTO t VALUES (1)"));
        assert!(stmts[1].contains("INSERT INTO t VALUES (2)"));
    }
}
