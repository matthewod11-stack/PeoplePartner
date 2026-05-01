//! Error + storage types, folder management, indexing pipeline, and FTS search.
//!
//! Holds the cross-section global `scan_lock` (prevents concurrent watcher +
//! manual scan from corrupting the chunk index) and all DB-touching code
//! paths.

use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use thiserror::Error;
use tokio::sync::Mutex as TokioMutex;

use crate::db::DbPool;
use crate::pii;

use super::chunker::{parse_file, RawChunk};
use super::discovery::{discover_files, file_type, hash_file};

/// Maximum tokens for document context in system prompt
pub const MAX_DOCUMENT_CONTEXT_TOKENS: usize = 4_000;

/// Characters per token (matches context.rs)
const CHARS_PER_TOKEN: usize = 4;

/// Maximum document context characters
const MAX_DOCUMENT_CONTEXT_CHARS: usize = MAX_DOCUMENT_CONTEXT_TOKENS * CHARS_PER_TOKEN;

/// Maximum chunks to include in context
const MAX_CHUNKS_IN_CONTEXT: usize = 5;

/// Global scan mutex — prevents concurrent watcher + manual scan from corrupting the chunk index
pub(super) fn scan_lock() -> &'static TokioMutex<()> {
    static LOCK: OnceLock<TokioMutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| TokioMutex::new(()))
}

// ============================================================================
// Errors
// ============================================================================

#[derive(Error, Debug)]
pub enum DocumentError {
    #[error("Database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Parse error for {file}: {message}")]
    Parse { file: String, message: String },
    #[error("Folder not found: {0}")]
    FolderNotFound(String),
    #[error("No document folder configured")]
    NoFolder,
}

// Serialize for Tauri command returns
impl Serialize for DocumentError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DocumentFolder {
    pub id: i64,
    pub path: String,
    pub label: Option<String>,
    pub active: bool,
    pub created_at: String,
    pub last_scanned_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Document {
    pub id: i64,
    pub folder_id: i64,
    pub file_path: String,
    pub file_name: String,
    pub file_type: String,
    pub file_size: Option<i64>,
    pub content_hash: Option<String>,
    pub chunk_count: i64,
    pub pii_detected: bool,
    pub indexed_at: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DocumentChunk {
    pub id: i64,
    pub document_id: i64,
    pub chunk_index: i64,
    pub section_title: Option<String>,
    pub content: String,
    pub char_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentFolderStats {
    pub path: String,
    pub label: Option<String>,
    pub file_count: u32,
    pub chunk_count: u32,
    pub pii_file_count: u32,
    pub error_file_count: u32,
    pub last_scanned_at: Option<String>,
    /// Whether the on-disk folder is currently accessible. False covers BOTH
    /// "folder was deleted/moved" AND "macOS revoked Full Disk Access" — the
    /// frontend treats it as a single "needs reselect" signal. (issue #38)
    pub folder_accessible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentStats {
    pub total_files: u32,
    pub total_chunks: u32,
    pub files_with_pii: u32,
    pub files_with_errors: u32,
    pub files_by_type: HashMap<String, u32>,
    pub last_scanned_at: Option<String>,
}

// ============================================================================
// Folder CRUD
// ============================================================================

/// Set the document folder path. Creates or updates the single folder record.
/// V3.0 supports one folder — the schema supports multiple for future use.
pub async fn set_document_folder(pool: &DbPool, path: &str) -> Result<DocumentFolder, DocumentError> {
    let path = Path::new(path);
    if !path.exists() || !path.is_dir() {
        return Err(DocumentError::FolderNotFound(path.display().to_string()));
    }

    let path_str = path.to_string_lossy().to_string();

    let mut tx = pool.begin().await?;

    // Delete all other folders (CASCADE removes their docs + chunks from FTS)
    sqlx::query("DELETE FROM document_folders WHERE path != ?1")
        .bind(&path_str)
        .execute(&mut *tx)
        .await?;

    let folder = sqlx::query_as::<_, DocumentFolder>(
        "INSERT INTO document_folders (path, active) VALUES (?1, 1)
         ON CONFLICT(path) DO UPDATE SET active = 1, last_scanned_at = NULL
         RETURNING *"
    )
    .bind(&path_str)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(folder)
}

/// Get the active document folder (if any)
pub async fn get_document_folder(pool: &DbPool) -> Result<Option<DocumentFolder>, DocumentError> {
    let folder = sqlx::query_as::<_, DocumentFolder>(
        "SELECT * FROM document_folders WHERE active = 1 LIMIT 1"
    )
    .fetch_optional(pool)
    .await?;

    Ok(folder)
}

/// Remove the active document folder and all its indexed data
pub async fn remove_document_folder(pool: &DbPool) -> Result<(), DocumentError> {
    // CASCADE deletes handle documents and chunks
    sqlx::query("DELETE FROM document_folders WHERE active = 1")
        .execute(pool)
        .await?;
    // Clean up any orphan inactive folders
    sqlx::query("DELETE FROM document_folders WHERE active = 0")
        .execute(pool)
        .await?;
    Ok(())
}

/// Get stats for the active folder
pub async fn get_folder_stats(pool: &DbPool) -> Result<Option<DocumentFolderStats>, DocumentError> {
    let folder = match get_document_folder(pool).await? {
        Some(f) => f,
        None => return Ok(None),
    };

    let file_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM documents WHERE folder_id = ?1"
    ).bind(folder.id).fetch_one(pool).await?;

    let chunk_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM document_chunks dc
         JOIN documents d ON dc.document_id = d.id
         WHERE d.folder_id = ?1"
    ).bind(folder.id).fetch_one(pool).await?;

    let pii_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM documents WHERE folder_id = ?1 AND pii_detected = 1"
    ).bind(folder.id).fetch_one(pool).await?;

    let error_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM documents WHERE folder_id = ?1 AND error IS NOT NULL"
    ).bind(folder.id).fetch_one(pool).await?;

    // read_dir().is_ok() catches BOTH the folder being deleted/moved AND
    // macOS Full Disk Access being revoked, where Path::exists() would still
    // return true. Single syscall — a more honest "can we actually read this
    // folder" signal than exists().
    let folder_accessible = std::fs::read_dir(&folder.path).is_ok();

    Ok(Some(DocumentFolderStats {
        path: folder.path,
        label: folder.label,
        file_count: file_count.0 as u32,
        chunk_count: chunk_count.0 as u32,
        pii_file_count: pii_count.0 as u32,
        error_file_count: error_count.0 as u32,
        last_scanned_at: folder.last_scanned_at,
        folder_accessible,
    }))
}

/// Get aggregate document indexing stats for the active folder.
/// Returns zeroed stats when no folder is configured.
pub async fn get_document_stats(pool: &DbPool) -> Result<DocumentStats, DocumentError> {
    let folder = get_document_folder(pool).await?;
    let Some(folder) = folder else {
        return Ok(DocumentStats {
            total_files: 0,
            total_chunks: 0,
            files_with_pii: 0,
            files_with_errors: 0,
            files_by_type: HashMap::new(),
            last_scanned_at: None,
        });
    };

    let total_files: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM documents WHERE folder_id = ?1"
    ).bind(folder.id).fetch_one(pool).await?;

    let total_chunks: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM document_chunks dc
         JOIN documents d ON dc.document_id = d.id
         WHERE d.folder_id = ?1"
    ).bind(folder.id).fetch_one(pool).await?;

    let files_with_pii: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM documents WHERE folder_id = ?1 AND pii_detected = 1"
    ).bind(folder.id).fetch_one(pool).await?;

    let files_with_errors: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM documents WHERE folder_id = ?1 AND error IS NOT NULL"
    ).bind(folder.id).fetch_one(pool).await?;

    let by_type_rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT file_type, COUNT(*) as count
         FROM documents
         WHERE folder_id = ?1
         GROUP BY file_type"
    ).bind(folder.id).fetch_all(pool).await?;

    let mut files_by_type = HashMap::new();
    for (file_type, count) in by_type_rows {
        files_by_type.insert(file_type, count as u32);
    }

    Ok(DocumentStats {
        total_files: total_files.0 as u32,
        total_chunks: total_chunks.0 as u32,
        files_with_pii: files_with_pii.0 as u32,
        files_with_errors: files_with_errors.0 as u32,
        files_by_type,
        last_scanned_at: folder.last_scanned_at,
    })
}

// ============================================================================
// Indexing Pipeline
// ============================================================================

/// Index a single file: parse → chunk → PII redact → store in DB
async fn index_file(
    pool: &DbPool,
    folder_id: i64,
    path: &Path,
) -> Result<Document, DocumentError> {
    let file_name = path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let file_path_str = path.to_string_lossy().to_string();
    let ext = file_type(path);
    let file_size = std::fs::metadata(path)?.len() as i64;
    let content_hash = hash_file(path)?;

    // Check if already indexed with same hash
    let existing: Option<Document> = sqlx::query_as(
        "SELECT * FROM documents WHERE file_path = ?1"
    )
    .bind(&file_path_str)
    .fetch_optional(pool)
    .await?;

    if let Some(ref doc) = existing {
        // Skip ONLY when hash matches AND no prior error AND has chunks
        if doc.content_hash.as_deref() == Some(&content_hash)
            && doc.error.is_none()
            && doc.chunk_count > 0
        {
            return Ok(doc.clone());
        }
    }

    // Parse the file
    let chunks: Vec<RawChunk> = match parse_file(path) {
        Ok(c) => c,
        Err(e) => {
            // Store document record with error
            let doc = upsert_document(
                pool, folder_id, &file_path_str, &file_name, &ext,
                file_size, &content_hash, 0, false, Some(&e.to_string()),
            ).await?;
            return Ok(doc);
        }
    };

    // PII scan + redact each chunk, then insert — wrapped in a transaction
    let mut pii_detected = false;
    let mut chunk_count = 0;

    // Get or create the document record first (outside transaction for the ID)
    let doc = upsert_document(
        pool, folder_id, &file_path_str, &file_name, &ext,
        file_size, &content_hash, 0, false, None,
    ).await?;

    // Transaction: delete old chunks + insert new ones + update doc atomically
    let mut tx = pool.begin().await?;

    sqlx::query("DELETE FROM document_chunks WHERE document_id = ?1")
        .bind(doc.id)
        .execute(&mut *tx)
        .await?;

    for (i, chunk) in chunks.iter().enumerate() {
        if chunk.content.trim().is_empty() {
            continue;
        }

        // PII redaction
        let redaction = pii::scan_and_redact(&chunk.content);
        if redaction.had_pii {
            pii_detected = true;
        }

        let redacted_content = redaction.redacted_text;
        let char_count = redacted_content.len() as i64;

        sqlx::query(
            "INSERT OR REPLACE INTO document_chunks (document_id, chunk_index, section_title, content, char_count)
             VALUES (?1, ?2, ?3, ?4, ?5)"
        )
        .bind(doc.id)
        .bind(i as i64)
        .bind(&chunk.section_title)
        .bind(&redacted_content)
        .bind(char_count)
        .execute(&mut *tx)
        .await?;

        chunk_count += 1;
    }

    // Update document with final stats
    sqlx::query(
        "UPDATE documents SET chunk_count = ?1, pii_detected = ?2, error = NULL WHERE id = ?3"
    )
    .bind(chunk_count)
    .bind(pii_detected)
    .bind(doc.id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    // Return updated record
    let updated: Document = sqlx::query_as("SELECT * FROM documents WHERE id = ?1")
        .bind(doc.id)
        .fetch_one(pool)
        .await?;

    Ok(updated)
}

/// Upsert a document record
async fn upsert_document(
    pool: &DbPool,
    folder_id: i64,
    file_path: &str,
    file_name: &str,
    file_type: &str,
    file_size: i64,
    content_hash: &str,
    chunk_count: i64,
    pii_detected: bool,
    error: Option<&str>,
) -> Result<Document, DocumentError> {
    let doc = sqlx::query_as::<_, Document>(
        "INSERT INTO documents (folder_id, file_path, file_name, file_type, file_size, content_hash, chunk_count, pii_detected, error)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(file_path) DO UPDATE SET
            folder_id = ?1, file_name = ?3, file_type = ?4, file_size = ?5,
            content_hash = ?6, chunk_count = ?7, pii_detected = ?8, error = ?9,
            indexed_at = datetime('now')
         RETURNING *"
    )
    .bind(folder_id)
    .bind(file_path)
    .bind(file_name)
    .bind(file_type)
    .bind(file_size)
    .bind(content_hash)
    .bind(chunk_count)
    .bind(pii_detected)
    .bind(error)
    .fetch_one(pool)
    .await?;

    Ok(doc)
}

/// Scan the active folder: discover files, index new/changed, remove deleted
pub async fn scan_folder(pool: &DbPool) -> Result<DocumentFolderStats, DocumentError> {
    let _guard = scan_lock().lock().await;

    let folder = get_document_folder(pool).await?.ok_or(DocumentError::NoFolder)?;
    let folder_path = PathBuf::from(&folder.path);

    if !folder_path.exists() {
        return Err(DocumentError::FolderNotFound(folder.path));
    }

    // Discover all supported files
    let files = discover_files(&folder_path)?;
    let file_paths: std::collections::HashSet<String> = files
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    // Remove documents no longer on disk
    let existing_docs: Vec<Document> = sqlx::query_as(
        "SELECT * FROM documents WHERE folder_id = ?1"
    )
    .bind(folder.id)
    .fetch_all(pool)
    .await?;

    for doc in &existing_docs {
        if !file_paths.contains(&doc.file_path) {
            sqlx::query("DELETE FROM documents WHERE id = ?1")
                .bind(doc.id)
                .execute(pool)
                .await?;
        }
    }

    // Index each file (skips unchanged via hash check)
    for file_path in &files {
        if let Err(e) = index_file(pool, folder.id, file_path).await {
            log::warn!("[Documents] Failed to index {}: {}", file_path.display(), e);
        }
    }

    // Update last_scanned_at
    sqlx::query("UPDATE document_folders SET last_scanned_at = datetime('now') WHERE id = ?1")
        .bind(folder.id)
        .execute(pool)
        .await?;

    get_folder_stats(pool).await?.ok_or(DocumentError::NoFolder)
}

// ============================================================================
// FTS Retrieval
// ============================================================================

/// A retrieved document chunk with source metadata for citation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievedChunk {
    pub file_name: String,
    pub section_title: Option<String>,
    pub content: String,
    pub rank: f64,
}

/// Search indexed documents using FTS5. Returns top chunks ranked by relevance,
/// capped to fit within the document context token budget.
pub async fn search_documents(
    pool: &DbPool,
    query: &str,
) -> Result<Vec<RetrievedChunk>, DocumentError> {
    if query.trim().is_empty() {
        return Ok(vec![]);
    }

    // Build FTS query: split words, join with OR for broad matching
    let fts_query = query
        .split_whitespace()
        .filter(|w| w.len() > 2) // Skip tiny words
        .map(|w| format!("\"{}\"", w.replace('"', ""))) // Quote each term
        .collect::<Vec<_>>()
        .join(" OR ");

    if fts_query.is_empty() {
        return Ok(vec![]);
    }

    let rows: Vec<(String, Option<String>, String, f64)> = sqlx::query_as(
        "SELECT d.file_name, dc.section_title, dc.content, rank
         FROM document_chunks_fts fts
         JOIN document_chunks dc ON dc.id = fts.rowid
         JOIN documents d ON d.id = dc.document_id
         JOIN document_folders df ON df.id = d.folder_id AND df.active = 1
         WHERE document_chunks_fts MATCH ?1
         ORDER BY rank
         LIMIT 10"
    )
    .bind(&fts_query)
    .fetch_all(pool)
    .await?;

    // Take chunks that fit within token budget
    let mut result = Vec::new();
    let mut total_chars = 0;

    for (file_name, section_title, content, rank) in rows {
        if result.len() >= MAX_CHUNKS_IN_CONTEXT {
            break;
        }
        // Account for header overhead: "[From: file_name — section_title]\n"
        let section_len = section_title.as_ref().map_or(0, |s| s.len());
        let chunk_size = content.len() + file_name.len() + section_len + 15;
        if total_chars + chunk_size > MAX_DOCUMENT_CONTEXT_CHARS {
            continue; // Skip this chunk, try smaller ones
        }
        total_chars += chunk_size;
        result.push(RetrievedChunk {
            file_name,
            section_title,
            content,
            rank,
        });
    }

    Ok(result)
}

/// Format retrieved chunks as a section for the system prompt
pub fn format_document_context(chunks: &[RetrievedChunk]) -> String {
    if chunks.is_empty() {
        return String::new();
    }

    let mut output = String::new();
    for chunk in chunks {
        let source = match &chunk.section_title {
            Some(title) => format!("[From: {} — {}]", chunk.file_name, title),
            None => format!("[From: {}]", chunk.file_name),
        };
        output.push_str(&source);
        output.push('\n');
        output.push_str(&chunk.content);
        output.push_str("\n\n");
    }

    output.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;
    use tempfile::TempDir;

    async fn run_migration_sql(pool: &DbPool, migration_sql: &str) {
        let mut current_statement = String::new();
        let mut inside_begin_block = false;

        for line in migration_sql.lines() {
            let trimmed = line.trim();
            let upper = trimmed.to_uppercase();

            if trimmed.is_empty() || trimmed.starts_with("--") {
                continue;
            }

            current_statement.push_str(line);
            current_statement.push('\n');

            if upper.contains(" BEGIN") || upper.ends_with(" BEGIN") {
                inside_begin_block = true;
            }

            let is_end_of_block = upper.starts_with("END;") || upper == "END";
            if is_end_of_block && inside_begin_block {
                inside_begin_block = false;
            }

            if trimmed.ends_with(';') && !inside_begin_block {
                sqlx::query(&current_statement).execute(pool).await.unwrap();
                current_statement.clear();
            }
        }

        if !current_statement.trim().is_empty() {
            sqlx::query(&current_statement).execute(pool).await.unwrap();
        }
    }

    async fn setup_documents_test_db() -> DbPool {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        run_migration_sql(&pool, include_str!("../../migrations/007_documents.sql")).await;
        run_migration_sql(&pool, include_str!("../../migrations/008_document_chunks_unique.sql")).await;
        pool
    }

    #[test]
    fn test_format_document_context() {
        let chunks = vec![
            RetrievedChunk {
                file_name: "Handbook.pdf".to_string(),
                section_title: Some("Leave Policies".to_string()),
                content: "Parental leave is 12 weeks.".to_string(),
                rank: -1.0,
            },
            RetrievedChunk {
                file_name: "PTO Policy.docx".to_string(),
                section_title: None,
                content: "PTO accrues at 1.5 days per month.".to_string(),
                rank: -0.5,
            },
        ];

        let formatted = format_document_context(&chunks);
        assert!(formatted.contains("[From: Handbook.pdf — Leave Policies]"));
        assert!(formatted.contains("[From: PTO Policy.docx]"));
        assert!(formatted.contains("Parental leave is 12 weeks."));
    }

    #[test]
    fn test_format_document_context_empty() {
        let formatted = format_document_context(&[]);
        assert!(formatted.is_empty());
    }

    #[tokio::test]
    async fn test_get_document_stats_zero_when_no_folder() {
        let pool = setup_documents_test_db().await;
        let stats = get_document_stats(&pool).await.unwrap();
        assert_eq!(stats.total_files, 0);
        assert_eq!(stats.total_chunks, 0);
        assert_eq!(stats.files_with_pii, 0);
        assert_eq!(stats.files_with_errors, 0);
        assert!(stats.files_by_type.is_empty());
        assert!(stats.last_scanned_at.is_none());
    }

    #[tokio::test]
    async fn test_search_documents_only_returns_active_folder_chunks() {
        let pool = setup_documents_test_db().await;

        sqlx::query("INSERT INTO document_folders (id, path, active) VALUES (1, '/active', 1)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO document_folders (id, path, active) VALUES (2, '/inactive', 0)")
            .execute(&pool)
            .await
            .unwrap();

        sqlx::query(
            "INSERT INTO documents (id, folder_id, file_path, file_name, file_type, file_size, content_hash, chunk_count, pii_detected, error)
             VALUES (10, 1, '/active/policy.md', 'policy.md', 'md', 100, 'h1', 1, 0, NULL)"
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO documents (id, folder_id, file_path, file_name, file_type, file_size, content_hash, chunk_count, pii_detected, error)
             VALUES (20, 2, '/inactive/old-policy.md', 'old-policy.md', 'md', 100, 'h2', 1, 0, NULL)"
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO document_chunks (document_id, chunk_index, section_title, content, char_count)
             VALUES (10, 0, 'Leave', 'Policy says active folder leave is 20 days.', 41)"
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO document_chunks (document_id, chunk_index, section_title, content, char_count)
             VALUES (20, 0, 'Leave', 'Policy says inactive folder leave is 99 days.', 43)"
        )
        .execute(&pool)
        .await
        .unwrap();

        let results = search_documents(&pool, "policy leave").await.unwrap();
        assert_eq!(results.len(), 1, "Only active folder chunks should be returned");
        assert_eq!(results[0].file_name, "policy.md");
        assert!(results[0].content.contains("20 days"));
    }

    // ========================================
    // folder_accessible field (issue #38)
    // ========================================
    //
    // Backstop the load-bearing assumption that drives the
    // documents-folder-missing UX: `get_folder_stats` must report
    // `folder_accessible = false` for any path that can't be opened with
    // `read_dir`. The frontend banner in DocumentFolderConfig.tsx renders
    // off this field, so a regression here surfaces as "watcher fires the
    // event but the user reopens Settings and the banner doesn't show."

    #[tokio::test]
    async fn get_folder_stats_reports_inaccessible_when_path_missing() {
        let pool = setup_documents_test_db().await;
        // Path that definitely doesn't exist on any reasonable test machine.
        sqlx::query(
            "INSERT INTO document_folders (id, path, active) VALUES (1, '/nonexistent/people-partner/test/path', 1)",
        )
        .execute(&pool)
        .await
        .unwrap();

        let stats = get_folder_stats(&pool)
            .await
            .expect("get_folder_stats should succeed even when folder is missing")
            .expect("active folder row exists, so Some is expected");

        assert!(
            !stats.folder_accessible,
            "missing folder must report folder_accessible = false (backs the missing-folder banner)"
        );
        // Sanity: stats are still well-formed for an empty folder.
        assert_eq!(stats.file_count, 0);
        assert_eq!(stats.chunk_count, 0);
    }

    #[tokio::test]
    async fn get_folder_stats_reports_accessible_for_real_dir() {
        let pool = setup_documents_test_db().await;
        let dir = TempDir::new().unwrap();
        let path = dir.path().to_string_lossy().to_string();

        sqlx::query("INSERT INTO document_folders (id, path, active) VALUES (1, ?1, 1)")
            .bind(&path)
            .execute(&pool)
            .await
            .unwrap();

        let stats = get_folder_stats(&pool)
            .await
            .unwrap()
            .expect("active folder row exists");

        assert!(
            stats.folder_accessible,
            "real readable folder must report folder_accessible = true"
        );
    }
}
