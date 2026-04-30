// People Partner - Document Ingestion Module
// Indexes user's HR document folder for contextual retrieval via FTS5
//
// Pipeline: Discover → Parse → Chunk → PII Redact → FTS Index → Retrieve

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::FromRow;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tauri::{AppHandle, Emitter};
use thiserror::Error;
use tokio::sync::Mutex as TokioMutex;

use crate::db::DbPool;
use crate::pii;

/// Global scan mutex — prevents concurrent watcher + manual scan from corrupting the chunk index
fn scan_lock() -> &'static TokioMutex<()> {
    static LOCK: OnceLock<TokioMutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| TokioMutex::new(()))
}

// ============================================================================
// Constants
// ============================================================================

/// Supported file extensions for indexing
const SUPPORTED_EXTENSIONS: &[&str] = &["md", "txt", "csv", "pdf", "docx", "xlsx", "xls"];

/// Maximum chunk size in characters (~1000 tokens at 4 chars/token)
const MAX_CHUNK_CHARS: usize = 4000;

/// Maximum tokens for document context in system prompt
pub const MAX_DOCUMENT_CONTEXT_TOKENS: usize = 4_000;

/// Characters per token (matches context.rs)
const CHARS_PER_TOKEN: usize = 4;

/// Maximum document context characters
const MAX_DOCUMENT_CONTEXT_CHARS: usize = MAX_DOCUMENT_CONTEXT_TOKENS * CHARS_PER_TOKEN;

/// Maximum chunks to include in context
const MAX_CHUNKS_IN_CONTEXT: usize = 5;

/// Maximum file size to index (50 MB)
const MAX_FILE_SIZE: u64 = 50 * 1024 * 1024;

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

/// A parsed chunk before indexing (pre-PII-redaction)
#[derive(Debug, Clone)]
pub struct RawChunk {
    pub section_title: Option<String>,
    pub content: String,
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
// File Discovery
// ============================================================================

/// Walk a directory and return all supported files
pub fn discover_files(folder_path: &Path) -> Result<Vec<PathBuf>, DocumentError> {
    let mut files = Vec::new();
    walk_dir(folder_path, &mut files)?;
    files.sort();
    Ok(files)
}

fn walk_dir(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), DocumentError> {
    if !dir.is_dir() {
        return Ok(());
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            log::warn!("[Documents] Cannot read directory {}: {}", dir.display(), e);
            return Ok(()); // Skip inaccessible directories gracefully
        }
    };
    for entry_result in entries {
        let entry = match entry_result {
            Ok(e) => e,
            Err(e) => {
                log::warn!("[Documents] Skipping unreadable entry in {}: {}", dir.display(), e);
                continue;
            }
        };
        let path = entry.path();
        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(e) => {
                log::warn!("[Documents] Cannot read metadata for {}: {}", path.display(), e);
                continue;
            }
        };

        // Skip symlinks
        if metadata.file_type().is_symlink() {
            continue;
        }

        if metadata.is_dir() {
            // Skip hidden directories
            if path.file_name().map_or(false, |n| n.to_string_lossy().starts_with('.')) {
                continue;
            }
            walk_dir(&path, files)?;
        } else if is_supported_file(&path) {
            // Skip oversized files
            if metadata.len() > MAX_FILE_SIZE {
                log::info!("[Documents] Skipping oversized file ({} bytes): {}", metadata.len(), path.display());
                continue;
            }
            files.push(path);
        }
    }
    Ok(())
}

fn is_supported_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| SUPPORTED_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// Compute SHA-256 hash of file contents
pub fn hash_file(path: &Path) -> Result<String, DocumentError> {
    let contents = std::fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&contents);
    Ok(hex::encode(hasher.finalize()))
}

/// Get the file extension as a lowercase string
pub fn file_type(path: &Path) -> String {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_lowercase())
        .unwrap_or_default()
}

// ============================================================================
// Parsers — Text-Based
// ============================================================================

/// Parse a markdown file into section-aware chunks.
/// Splits on ## headings. Content before first heading gets section_title = None.
pub fn parse_markdown(content: &str) -> Vec<RawChunk> {
    let mut chunks = Vec::new();
    let mut current_title: Option<String> = None;
    let mut current_content = String::new();

    for line in content.lines() {
        if line.starts_with("## ") || line.starts_with("# ") {
            // Flush previous chunk
            if !current_content.trim().is_empty() {
                chunks.push(RawChunk {
                    section_title: current_title.take(),
                    content: current_content.trim().to_string(),
                });
            }
            current_title = Some(line.trim_start_matches('#').trim().to_string());
            current_content = String::new();
        } else {
            current_content.push_str(line);
            current_content.push('\n');
        }
    }

    // Flush final chunk
    if !current_content.trim().is_empty() {
        chunks.push(RawChunk {
            section_title: current_title,
            content: current_content.trim().to_string(),
        });
    }

    // Sub-chunk any oversized sections
    split_oversized_chunks(chunks)
}

/// Parse a plain text file into paragraph-based chunks.
/// Splits on double newlines (blank lines).
pub fn parse_plaintext(content: &str) -> Vec<RawChunk> {
    let paragraphs: Vec<&str> = content.split("\n\n").collect();
    let mut chunks = Vec::new();
    let mut current = String::new();

    for para in paragraphs {
        let trimmed = para.trim();
        if trimmed.is_empty() {
            continue;
        }
        if current.len() + trimmed.len() > MAX_CHUNK_CHARS && !current.is_empty() {
            chunks.push(RawChunk {
                section_title: None,
                content: current.trim().to_string(),
            });
            current = String::new();
        }
        if !current.is_empty() {
            current.push_str("\n\n");
        }
        current.push_str(trimmed);
    }

    if !current.trim().is_empty() {
        chunks.push(RawChunk {
            section_title: None,
            content: current.trim().to_string(),
        });
    }

    chunks
}

/// Parse a CSV file into row-group chunks.
/// Groups ~20 rows per chunk with the header row prepended.
pub fn parse_csv(content: &str) -> Vec<RawChunk> {
    let mut lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return vec![];
    }

    let header = lines.remove(0);
    let mut chunks = Vec::new();

    for (i, group) in lines.chunks(20).enumerate() {
        let mut chunk_content = format!("{}\n", header);
        for row in group {
            chunk_content.push_str(row);
            chunk_content.push('\n');
        }
        chunks.push(RawChunk {
            section_title: Some(format!("Rows {}-{}", i * 20 + 1, i * 20 + group.len())),
            content: chunk_content.trim().to_string(),
        });
    }

    chunks
}

/// Hard-split a single chunk of text that has no paragraph breaks.
/// Tries sentence boundaries (". ") → newlines → spaces → hard char limit.
fn hard_split_chunk(content: &str) -> Vec<String> {
    let mut pieces = Vec::new();
    let mut remaining = content;

    while remaining.len() > MAX_CHUNK_CHARS {
        let window = &remaining[..MAX_CHUNK_CHARS];

        // Try sentence boundary (". ")
        let split_pos = window.rfind(". ").map(|p| p + 2)
            // Try newline
            .or_else(|| window.rfind('\n').map(|p| p + 1))
            // Try space
            .or_else(|| window.rfind(' ').map(|p| p + 1))
            // Hard split at limit
            .unwrap_or(MAX_CHUNK_CHARS);

        pieces.push(remaining[..split_pos].trim().to_string());
        remaining = &remaining[split_pos..];
    }

    if !remaining.trim().is_empty() {
        pieces.push(remaining.trim().to_string());
    }

    pieces
}

/// Split any chunk larger than MAX_CHUNK_CHARS at paragraph boundaries,
/// falling back to hard_split_chunk for single-paragraph content.
fn split_oversized_chunks(chunks: Vec<RawChunk>) -> Vec<RawChunk> {
    let mut result = Vec::new();
    for chunk in chunks {
        if chunk.content.len() <= MAX_CHUNK_CHARS {
            result.push(chunk);
        } else {
            // Split at paragraph boundaries within the oversized chunk
            let sub_chunks = parse_plaintext(&chunk.content);

            // If paragraph splitting didn't help (single paragraph), use hard split
            let needs_hard_split = sub_chunks.iter().any(|c| c.content.len() > MAX_CHUNK_CHARS);

            if needs_hard_split {
                let pieces = hard_split_chunk(&chunk.content);
                for (i, piece) in pieces.into_iter().enumerate() {
                    let title = if i == 0 {
                        chunk.section_title.clone()
                    } else {
                        chunk.section_title.as_ref().map(|t| format!("{} (cont.)", t))
                    };
                    result.push(RawChunk {
                        section_title: title,
                        content: piece,
                    });
                }
            } else {
                for (i, mut sub) in sub_chunks.into_iter().enumerate() {
                    if i == 0 {
                        sub.section_title = chunk.section_title.clone();
                    } else {
                        sub.section_title = chunk.section_title.as_ref().map(|t| format!("{} (cont.)", t));
                    }
                    result.push(sub);
                }
            }
        }
    }
    result
}

// ============================================================================
// Parsers — Binary Formats
// ============================================================================

/// Best-effort extraction of a panic message from the payload returned by
/// `catch_unwind`. Panics can carry either `&'static str` or `String`.
fn panic_payload_to_string(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&'static str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic payload".to_string()
    }
}

/// Parse a PDF file. Extracts text per page, each page becomes a chunk.
///
/// The underlying `pdf-extract` crate has historically panicked on malformed PDFs
/// (encrypted object streams, malformed xref, etc.). We wrap the extraction in
/// `catch_unwind` so a poisoned file in the watched folder cannot tear down the
/// watcher thread. Note: in release builds we compile with `panic = "abort"`, so
/// the catch is effective only in debug/test builds — the bump to pdf-extract
/// 0.10 is the primary defense in release.
pub fn parse_pdf(path: &Path) -> Result<Vec<RawChunk>, DocumentError> {
    let bytes = std::fs::read(path)?;
    let file_display = path.display().to_string();

    let extract_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        pdf_extract::extract_text_from_mem(&bytes)
    }));

    let text = match extract_result {
        Ok(Ok(text)) => text,
        Ok(Err(e)) => {
            return Err(DocumentError::Parse {
                file: file_display,
                message: format!("PDF extraction failed: {}", e),
            });
        }
        Err(panic_payload) => {
            let panic_msg = panic_payload_to_string(&panic_payload);
            return Err(DocumentError::Parse {
                file: file_display,
                message: format!("PDF extraction panicked: {}", panic_msg),
            });
        }
    };

    // pdf-extract returns all text concatenated; split on form feeds or large gaps
    // Fall back to paragraph-based chunking with page estimation
    let chunks = if text.contains('\u{0C}') {
        // Form feed characters indicate page breaks
        text.split('\u{0C}')
            .enumerate()
            .filter(|(_, page)| !page.trim().is_empty())
            .map(|(i, page)| RawChunk {
                section_title: Some(format!("Page {}", i + 1)),
                content: page.trim().to_string(),
            })
            .collect()
    } else {
        // No page breaks detected — use paragraph chunking
        parse_plaintext(&text)
    };

    Ok(split_oversized_chunks(chunks))
}

/// Parse a .docx file. Extracts paragraph text, splits on heading styles.
///
/// Wrapped in `catch_unwind` to defend the watcher thread against malformed
/// docx files that may cause the (unmaintained) docx-rs 0.4 parser to panic.
pub fn parse_docx(path: &Path) -> Result<Vec<RawChunk>, DocumentError> {
    let bytes = std::fs::read(path)?;
    let file_display = path.display().to_string();

    let read_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        docx_rs::read_docx(&bytes)
    }));

    let doc = match read_result {
        Ok(Ok(doc)) => doc,
        Ok(Err(e)) => {
            return Err(DocumentError::Parse {
                file: file_display,
                message: format!("DOCX parsing failed: {}", e),
            });
        }
        Err(panic_payload) => {
            let panic_msg = panic_payload_to_string(&panic_payload);
            return Err(DocumentError::Parse {
                file: file_display,
                message: format!("DOCX parsing panicked: {}", panic_msg),
            });
        }
    };

    let mut chunks = Vec::new();
    let mut current_title: Option<String> = None;
    let mut current_content = String::new();

    for child in doc.document.children {
        if let docx_rs::DocumentChild::Paragraph(para) = child {
            // Extract text from paragraph runs
            let mut para_text = String::new();
            for child in &para.children {
                if let docx_rs::ParagraphChild::Run(run) = child {
                    for child in &run.children {
                        if let docx_rs::RunChild::Text(text) = child {
                            para_text.push_str(&text.text);
                        }
                    }
                }
            }

            let trimmed = para_text.trim().to_string();
            if trimmed.is_empty() {
                continue;
            }

            // Check if this paragraph has a heading style
            let is_heading = para.property.style.as_ref().map_or(false, |s| {
                s.val.to_lowercase().starts_with("heading")
            });

            if is_heading {
                // Flush current chunk
                if !current_content.trim().is_empty() {
                    chunks.push(RawChunk {
                        section_title: current_title.take(),
                        content: current_content.trim().to_string(),
                    });
                }
                current_title = Some(trimmed);
                current_content = String::new();
            } else {
                current_content.push_str(&trimmed);
                current_content.push('\n');
            }
        }
    }

    // Flush final chunk
    if !current_content.trim().is_empty() {
        chunks.push(RawChunk {
            section_title: current_title,
            content: current_content.trim().to_string(),
        });
    }

    Ok(split_oversized_chunks(chunks))
}

/// Parse an .xlsx/.xls file. Each sheet becomes chunks of grouped rows.
pub fn parse_xlsx(path: &Path) -> Result<Vec<RawChunk>, DocumentError> {
    use calamine::{open_workbook_auto, Reader, Data};

    let mut workbook = open_workbook_auto(path).map_err(|e| DocumentError::Parse {
        file: path.display().to_string(),
        message: format!("Excel parsing failed: {}", e),
    })?;

    let mut chunks = Vec::new();
    let sheet_names: Vec<String> = workbook.sheet_names().to_vec();

    for sheet_name in &sheet_names {
        if let Ok(range) = workbook.worksheet_range(sheet_name) {
            let rows: Vec<Vec<String>> = range
                .rows()
                .map(|row| {
                    row.iter()
                        .map(|cell| match cell {
                            Data::String(s) => s.clone(),
                            Data::Float(f) => f.to_string(),
                            Data::Int(i) => i.to_string(),
                            Data::Bool(b) => b.to_string(),
                            _ => String::new(),
                        })
                        .collect()
                })
                .collect();

            if rows.is_empty() {
                continue;
            }

            let header = rows[0].join(",");
            let data_rows = &rows[1..];

            for (i, group) in data_rows.chunks(20).enumerate() {
                let mut content = format!("{}\n", header);
                for row in group {
                    content.push_str(&row.join(","));
                    content.push('\n');
                }
                chunks.push(RawChunk {
                    section_title: Some(format!("{} — Rows {}-{}", sheet_name, i * 20 + 1, i * 20 + group.len())),
                    content: content.trim().to_string(),
                });
            }
        }
    }

    Ok(chunks)
}

/// Dispatch to the correct parser based on file type
pub fn parse_file(path: &Path) -> Result<Vec<RawChunk>, DocumentError> {
    let ext = file_type(path);
    let content_result = || -> Result<String, DocumentError> {
        Ok(std::fs::read_to_string(path)?)
    };

    match ext.as_str() {
        "md" => Ok(parse_markdown(&content_result()?)),
        "txt" => Ok(parse_plaintext(&content_result()?)),
        "csv" => Ok(parse_csv(&content_result()?)),
        "pdf" => parse_pdf(path),
        "docx" => parse_docx(path),
        "xlsx" | "xls" => parse_xlsx(path),
        _ => Err(DocumentError::Parse {
            file: path.display().to_string(),
            message: format!("Unsupported file type: {}", ext),
        }),
    }
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
    let chunks = match parse_file(path) {
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

// ============================================================================
// File System Watcher (issue #38: async refactor + folder-missing handling +
// real debouncing via notify-debouncer-full)
// ============================================================================
//
// Why async + tokio:
// - Old design used std::thread + std::sync::Mutex<JoinHandle>; start() called
//   stop() (which join()s the thread) while holding the mutex → deadlock risk
//   when called from a Tauri tokio command. New design uses tokio::sync::Mutex
//   and tokio::task::JoinHandle so stop() yields rather than blocks.
// - Old loop block-on'd a tokio runtime handle from inside a std thread to run
//   the async DB scan. New loop is itself a tokio task — scan_folder() is just
//   awaited directly.
//
// Why notify-debouncer-full:
// - Old "debounce" was recv_timeout(2s) with an inner drain loop that resets
//   every 2s. Under continuous churn (cloud sync, git ops), the drain never
//   ends and a scan never fires; under bursty churn the drain may fire mid-
//   sequence. The official debouncer guarantees a fixed timeout window after
//   the last event before delivering a coalesced batch.
// - DEBOUNCE_INTERVAL = 1500ms catches OneDrive/Dropbox/iCloud bursts (~500ms
//   to 1s typical) and Word/Excel save cycles without feeling laggy on a
//   single drag-drop.

use notify::{event::RemoveKind, EventKind, RecursiveMode};
use notify_debouncer_full::{new_debouncer, DebounceEventResult};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

/// Debounce window — see module-level comment for the choice rationale.
const DEBOUNCE_INTERVAL: Duration = Duration::from_millis(1500);

/// Bounded wait when stopping a running watcher. The inner task may be in the
/// middle of a `scan_folder` await that doesn't observe `cancel.cancelled()`
/// until the scan finishes. We don't want to hang the UI thread that called
/// `set_document_folder` or `remove_document_folder` if a scan is genuinely
/// stuck — log a warning and move on. The cancel signal still fires when the
/// scan eventually finishes, so the task exits without leaking the debouncer.
const WATCHER_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

/// Tauri event name emitted when the watcher detects the watched root has
/// disappeared (or after a rescan attempt finds it missing).
pub const FOLDER_MISSING_EVENT: &str = "documents-folder-missing";

/// Manages the lifecycle of the file-system watcher task.
/// Allows stopping/restarting when the watched folder changes.
pub struct WatcherState {
    inner: TokioMutex<Option<RunningWatcher>>,
    app_handle: AppHandle,
}

struct RunningWatcher {
    handle: tokio::task::JoinHandle<()>,
    cancel: CancellationToken,
}

impl WatcherState {
    pub fn new(app_handle: AppHandle) -> Self {
        Self {
            inner: TokioMutex::new(None),
            app_handle,
        }
    }

    /// Stop the running watcher (if any). Returns when the task observes the
    /// cancel and exits, or after `WATCHER_SHUTDOWN_TIMEOUT`. Non-blocking on
    /// the tokio runtime.
    pub async fn stop(&self) {
        let mut guard = self.inner.lock().await;
        if let Some(rw) = guard.take() {
            rw.cancel.cancel();
            match tokio::time::timeout(WATCHER_SHUTDOWN_TIMEOUT, rw.handle).await {
                Ok(_) => {}
                Err(_) => {
                    // Hung scan. The cancel signal is still set, so the task
                    // will exit after the in-flight scan returns. We don't
                    // abort here because the JoinHandle was consumed by
                    // tokio::time::timeout — and abort() on a task mid-
                    // sqlx-transaction risks DB pool corruption anyway.
                    log::warn!(
                        "[Documents] Watcher did not exit within {:?}; cancel signal sent, task will exit on next yield",
                        WATCHER_SHUTDOWN_TIMEOUT
                    );
                }
            }
        }
    }

    /// Stop any existing watcher, then start a new one for the active folder.
    pub async fn start(&self, pool: DbPool) {
        self.stop().await;
        let cancel = CancellationToken::new();
        let app_handle = self.app_handle.clone();
        if let Some(handle) = start_watcher_inner(pool, app_handle, cancel.clone()).await {
            *self.inner.lock().await = Some(RunningWatcher { handle, cancel });
        }
    }
}

/// Spawn the watcher task. Returns None if no folder is configured or the
/// debouncer fails to attach to the path.
async fn start_watcher_inner(
    pool: DbPool,
    app_handle: AppHandle,
    cancel: CancellationToken,
) -> Option<tokio::task::JoinHandle<()>> {
    let folder_path_str = get_document_folder(&pool).await.ok().flatten()?.path;

    // Canonicalize so we can later compare notify event paths against the
    // watched root reliably. macOS FSEvents may surface paths through
    // `/private/var/...` even when the user selected `/var/...`.
    let watched_root = match std::fs::canonicalize(&folder_path_str) {
        Ok(p) => p,
        Err(e) => {
            log::warn!(
                "[Documents] Cannot canonicalize {}: {}; folder not watched",
                folder_path_str,
                e
            );
            return None;
        }
    };

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<DebounceEventResult>();
    let mut debouncer = match new_debouncer(DEBOUNCE_INTERVAL, None, move |result: DebounceEventResult| {
        // Send is non-blocking on unbounded; ignore errors (rx dropped → task ended).
        let _ = tx.send(result);
    }) {
        Ok(d) => d,
        Err(e) => {
            log::error!("[Documents] Failed to create debouncer: {}", e);
            return None;
        }
    };

    if let Err(e) = debouncer.watch(&watched_root, RecursiveMode::Recursive) {
        log::error!(
            "[Documents] Failed to watch {}: {}",
            watched_root.display(),
            e
        );
        return None;
    }

    log::info!("[Documents] Watching: {}", watched_root.display());

    let handle = tokio::spawn(async move {
        // Keep the debouncer alive for the duration of the task; dropping it
        // would unsubscribe from FSEvents.
        let _debouncer = debouncer;

        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    log::info!("[Documents] Watcher cancelled");
                    return;
                }
                msg = rx.recv() => {
                    let Some(result) = msg else {
                        // Sender dropped — should only happen on shutdown.
                        return;
                    };
                    match result {
                        Ok(events) => {
                            // 1. Root-removal detection: any Remove(Folder|Any)
                            //    event whose path canonicalizes to the watched
                            //    root means the folder itself is gone.
                            let root_event = events.iter().any(|de| {
                                matches!(
                                    de.event.kind,
                                    EventKind::Remove(RemoveKind::Folder | RemoveKind::Any)
                                ) && de.event.paths.iter().any(|p| {
                                    std::fs::canonicalize(p).ok().as_deref()
                                        == Some(&watched_root)
                                })
                            });

                            // 2. Belt-and-suspenders: re-check exists() AFTER
                            //    the debounce window. Catches cases where the
                            //    Remove event was filtered or missed (e.g.,
                            //    quick unmount/remount), and avoids false
                            //    positives on atomic-replace renames where a
                            //    Create immediately follows the Remove.
                            if root_event && !watched_root.exists() {
                                log::info!(
                                    "[Documents] Watched root no longer accessible: {}",
                                    watched_root.display()
                                );
                                let _ = app_handle.emit(
                                    FOLDER_MISSING_EVENT,
                                    serde_json::json!({
                                        "path": watched_root.to_string_lossy(),
                                    }),
                                );
                                return;
                            }

                            // 3. Even if no Remove event, a missing folder
                            //    means we shouldn't rescan (would just emit a
                            //    failed-scan event). Surface as folder-missing.
                            if !watched_root.exists() {
                                let _ = app_handle.emit(
                                    FOLDER_MISSING_EVENT,
                                    serde_json::json!({
                                        "path": watched_root.to_string_lossy(),
                                    }),
                                );
                                return;
                            }

                            // 4. need_rescan() = the debouncer dropped events
                            //    under load and recommends a full sync. We
                            //    already do a full scan_folder, so this is
                            //    informational; logged for forensics.
                            if events.iter().any(|de| de.event.need_rescan()) {
                                log::info!("[Documents] Debouncer requested rescan (event drop)");
                            }

                            run_rescan(&pool, &app_handle).await;
                        }
                        Err(errors) => {
                            for e in errors {
                                log::error!("[Documents] Watch error: {}", e);
                            }
                        }
                    }
                }
            }
        }
    });

    Some(handle)
}

/// Trigger a full rescan and emit the corresponding lifecycle events.
/// Extracted from the watcher loop for readability + reuse from tests.
async fn run_rescan(pool: &DbPool, app_handle: &AppHandle) {
    log::debug!("[Documents] Rescan triggered");
    let _ = app_handle.emit(
        "documents-scan",
        serde_json::json!({ "status": "started", "source": "watcher" }),
    );
    match scan_folder(pool).await {
        Ok(stats) => {
            let _ = app_handle.emit(
                "documents-scan",
                serde_json::json!({
                    "status": "completed",
                    "source": "watcher",
                    "file_count": stats.file_count,
                    "chunk_count": stats.chunk_count,
                    "last_scanned_at": stats.last_scanned_at,
                }),
            );
        }
        Err(e) => {
            log::error!("[Documents] Re-scan failed: {}", e);
            let _ = app_handle.emit(
                "documents-scan",
                serde_json::json!({
                    "status": "failed",
                    "source": "watcher",
                    "error": e.to_string(),
                }),
            );
        }
    }
}

/// Start watching the active document folder for changes.
/// Returns a WatcherState that can be used to stop/restart the watcher.
pub async fn start_watcher(pool: DbPool, app_handle: AppHandle) -> WatcherState {
    let state = WatcherState::new(app_handle);
    state.start(pool).await;
    state
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
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
        run_migration_sql(&pool, include_str!("../migrations/007_documents.sql")).await;
        run_migration_sql(&pool, include_str!("../migrations/008_document_chunks_unique.sql")).await;
        pool
    }

    #[test]
    fn test_is_supported_file() {
        assert!(is_supported_file(Path::new("test.md")));
        assert!(is_supported_file(Path::new("test.pdf")));
        assert!(is_supported_file(Path::new("test.docx")));
        assert!(is_supported_file(Path::new("test.xlsx")));
        assert!(is_supported_file(Path::new("test.csv")));
        assert!(is_supported_file(Path::new("test.txt")));
        assert!(!is_supported_file(Path::new("test.jpg")));
        assert!(!is_supported_file(Path::new("test.exe")));
        assert!(!is_supported_file(Path::new("noextension")));
    }

    #[test]
    fn test_discover_files() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("policy.md"), "# Policy").unwrap();
        fs::write(dir.path().join("handbook.pdf"), "fake pdf").unwrap();
        fs::write(dir.path().join("photo.jpg"), "fake jpg").unwrap();
        fs::create_dir(dir.path().join(".hidden")).unwrap();
        fs::write(dir.path().join(".hidden/secret.md"), "hidden").unwrap();

        let files = discover_files(dir.path()).unwrap();
        assert_eq!(files.len(), 2); // .md and .pdf, not .jpg or hidden
    }

    #[test]
    fn test_discover_files_nested() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("subfolder")).unwrap();
        fs::write(dir.path().join("root.md"), "root").unwrap();
        fs::write(dir.path().join("subfolder/nested.txt"), "nested").unwrap();

        let files = discover_files(dir.path()).unwrap();
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn test_hash_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.txt");
        fs::write(&path, "hello world").unwrap();

        let hash1 = hash_file(&path).unwrap();
        let hash2 = hash_file(&path).unwrap();
        assert_eq!(hash1, hash2); // Deterministic

        fs::write(&path, "changed content").unwrap();
        let hash3 = hash_file(&path).unwrap();
        assert_ne!(hash1, hash3); // Different content = different hash
    }

    #[test]
    fn test_file_type() {
        assert_eq!(file_type(Path::new("test.md")), "md");
        assert_eq!(file_type(Path::new("test.PDF")), "pdf");
        assert_eq!(file_type(Path::new("noext")), "");
    }

    #[test]
    fn test_parse_markdown_sections() {
        let md = "# Intro\nWelcome to the handbook.\n\n## Leave Policies\nWe offer PTO.\n\n## Benefits\nHealth insurance available.";
        let chunks = parse_markdown(md);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].section_title.as_deref(), Some("Intro"));
        assert!(chunks[0].content.contains("Welcome"));
        assert_eq!(chunks[1].section_title.as_deref(), Some("Leave Policies"));
        assert_eq!(chunks[2].section_title.as_deref(), Some("Benefits"));
    }

    #[test]
    fn test_parse_markdown_no_headings() {
        let md = "Just plain text\nwith multiple lines\nand no headings.";
        let chunks = parse_markdown(md);
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].section_title.is_none());
    }

    #[test]
    fn test_parse_plaintext_paragraphs() {
        let text = "First paragraph.\n\nSecond paragraph.\n\nThird paragraph.";
        let chunks = parse_plaintext(text);
        assert_eq!(chunks.len(), 1); // All fit in one chunk
        assert!(chunks[0].content.contains("First"));
        assert!(chunks[0].content.contains("Third"));
    }

    #[test]
    fn test_parse_csv_groups_rows() {
        let mut csv = String::from("Name,Department,Title\n");
        for i in 1..=45 {
            csv.push_str(&format!("Employee{},Engineering,Dev\n", i));
        }
        let chunks = parse_csv(&csv);
        assert_eq!(chunks.len(), 3); // 20 + 20 + 5
        assert_eq!(chunks[0].section_title.as_deref(), Some("Rows 1-20"));
        assert_eq!(chunks[2].section_title.as_deref(), Some("Rows 41-45"));
        assert!(chunks[0].content.starts_with("Name,Department,Title")); // Header prepended
    }

    #[test]
    fn test_split_oversized_chunks() {
        // Create a chunk larger than MAX_CHUNK_CHARS using paragraphs
        let paragraph = "This is a paragraph with enough content to test splitting behavior for documents.\n\n";
        let repeat_count = (MAX_CHUNK_CHARS / paragraph.len()) + 10;
        let big_content = paragraph.repeat(repeat_count);
        let chunks = vec![RawChunk {
            section_title: Some("Big Section".to_string()),
            content: big_content,
        }];
        let result = split_oversized_chunks(chunks);
        assert!(result.len() > 1);
        assert_eq!(result[0].section_title.as_deref(), Some("Big Section"));
    }

    #[test]
    fn test_parse_file_dispatches_by_extension() {
        let dir = TempDir::new().unwrap();

        let md_path = dir.path().join("test.md");
        fs::write(&md_path, "# Title\nContent here").unwrap();
        let chunks = parse_file(&md_path).unwrap();
        assert!(!chunks.is_empty());

        let txt_path = dir.path().join("test.txt");
        fs::write(&txt_path, "Some plain text content.").unwrap();
        let chunks = parse_file(&txt_path).unwrap();
        assert!(!chunks.is_empty());

        let csv_path = dir.path().join("test.csv");
        fs::write(&csv_path, "Name,Role\nAlice,Dev\nBob,PM").unwrap();
        let chunks = parse_file(&csv_path).unwrap();
        assert!(!chunks.is_empty());
    }

    // Regression coverage for #29 — malformed binary input must return an
    // `Err` instead of panicking, so the watcher thread survives a poisoned
    // file dropped into the watched folder.
    #[test]
    fn test_parse_pdf_malformed_returns_error() {
        let dir = TempDir::new().unwrap();
        let pdf_path = dir.path().join("not-really.pdf");
        // Random bytes masquerading as a PDF — no valid header, xref, or objects.
        fs::write(&pdf_path, b"\x00\x01\x02not a pdf at all\xff\xfe").unwrap();

        let result = parse_pdf(&pdf_path);
        assert!(result.is_err(), "malformed PDF must surface as Err, not panic");
        if let Err(DocumentError::Parse { file, .. }) = result {
            assert!(file.ends_with("not-really.pdf"));
        } else {
            panic!("expected DocumentError::Parse for malformed PDF");
        }
    }

    #[test]
    fn test_parse_docx_malformed_returns_error() {
        let dir = TempDir::new().unwrap();
        let docx_path = dir.path().join("fake.docx");
        // docx is a zip container; arbitrary bytes must not make the parser panic.
        fs::write(&docx_path, b"PK\x03\x04not a real docx payload").unwrap();

        let result = parse_docx(&docx_path);
        assert!(result.is_err(), "malformed DOCX must surface as Err, not panic");
        if let Err(DocumentError::Parse { file, .. }) = result {
            assert!(file.ends_with("fake.docx"));
        } else {
            panic!("expected DocumentError::Parse for malformed DOCX");
        }
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

    #[test]
    fn test_hard_split_chunk_at_sentence() {
        // Build a long string with sentence boundaries
        let sentence = "This is a test sentence with enough words to fill some space. ";
        let repeat_count = (MAX_CHUNK_CHARS / sentence.len()) + 5;
        let big = sentence.repeat(repeat_count);
        assert!(big.len() > MAX_CHUNK_CHARS);

        let pieces = hard_split_chunk(&big);
        assert!(pieces.len() > 1, "Should split into multiple pieces");
        for piece in &pieces {
            assert!(piece.len() <= MAX_CHUNK_CHARS, "Each piece should fit within limit");
        }
        // Verify content is preserved (no data loss)
        let rejoined: String = pieces.join(" ");
        // The original sentences should all be present
        assert!(rejoined.contains("This is a test sentence"));
    }

    #[test]
    fn test_hard_split_chunk_no_boundaries() {
        // A long string with no sentence boundaries, newlines, or spaces
        let big = "x".repeat(MAX_CHUNK_CHARS * 2 + 100);
        let pieces = hard_split_chunk(&big);
        assert!(pieces.len() >= 2, "Should still split via hard char limit");
        for piece in &pieces {
            assert!(piece.len() <= MAX_CHUNK_CHARS, "Each piece must be within limit");
        }
    }

    #[test]
    fn test_walk_dir_skips_symlinks() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("real.md"), "# Real file").unwrap();

        // Create a symlink to a file
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(
                dir.path().join("real.md"),
                dir.path().join("link.md"),
            ).unwrap();
        }

        let files = discover_files(dir.path()).unwrap();
        // Should only find real.md, not the symlink
        assert_eq!(files.len(), 1);
        assert!(files[0].file_name().unwrap().to_str().unwrap() == "real.md");
    }

    #[test]
    fn test_split_oversized_single_paragraph() {
        // A single oversized "paragraph" with no blank lines — only sentence boundaries
        let sentence = "This is a sentence in a very long document without paragraph breaks. ";
        let repeat_count = (MAX_CHUNK_CHARS / sentence.len()) + 5;
        let big_content = sentence.repeat(repeat_count);
        assert!(big_content.len() > MAX_CHUNK_CHARS);
        assert!(!big_content.contains("\n\n")); // No paragraph breaks

        let chunks = vec![RawChunk {
            section_title: Some("Monolith".to_string()),
            content: big_content,
        }];
        let result = split_oversized_chunks(chunks);
        assert!(result.len() > 1, "Single paragraph should be hard-split");
        assert_eq!(result[0].section_title.as_deref(), Some("Monolith"));
        for chunk in &result {
            assert!(chunk.content.len() <= MAX_CHUNK_CHARS, "All chunks must fit within limit");
        }
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
