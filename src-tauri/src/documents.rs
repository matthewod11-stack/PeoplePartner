// HR Command Center - Document Ingestion Module
// Indexes user's HR document folder for contextual retrieval via FTS5
//
// Pipeline: Discover → Parse → Chunk → PII Redact → FTS Index → Retrieve

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::FromRow;
use std::path::{Path, PathBuf};
use thiserror::Error;

use crate::db::DbPool;
use crate::pii;

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentStats {
    pub total_files: u32,
    pub total_chunks: u32,
    pub files_with_pii: u32,
    pub files_with_errors: u32,
    pub files_by_type: std::collections::HashMap<String, u32>,
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

    // Upsert: deactivate all existing folders, insert/activate this one
    sqlx::query("UPDATE document_folders SET active = 0")
        .execute(pool)
        .await?;

    let folder = sqlx::query_as::<_, DocumentFolder>(
        "INSERT INTO document_folders (path, active) VALUES (?1, 1)
         ON CONFLICT(path) DO UPDATE SET active = 1, last_scanned_at = NULL
         RETURNING *"
    )
    .bind(&path_str)
    .fetch_one(pool)
    .await?;

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

    Ok(Some(DocumentFolderStats {
        path: folder.path,
        label: folder.label,
        file_count: file_count.0 as u32,
        chunk_count: chunk_count.0 as u32,
        pii_file_count: pii_count.0 as u32,
        error_file_count: error_count.0 as u32,
        last_scanned_at: folder.last_scanned_at,
    }))
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
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            // Skip hidden directories
            if path.file_name().map_or(false, |n| n.to_string_lossy().starts_with('.')) {
                continue;
            }
            walk_dir(&path, files)?;
        } else if is_supported_file(&path) {
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

/// Split any chunk larger than MAX_CHUNK_CHARS at paragraph boundaries
fn split_oversized_chunks(chunks: Vec<RawChunk>) -> Vec<RawChunk> {
    let mut result = Vec::new();
    for chunk in chunks {
        if chunk.content.len() <= MAX_CHUNK_CHARS {
            result.push(chunk);
        } else {
            // Split at paragraph boundaries within the oversized chunk
            let sub_chunks = parse_plaintext(&chunk.content);
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
    result
}

// ============================================================================
// Parsers — Binary Formats
// ============================================================================

/// Parse a PDF file. Extracts text per page, each page becomes a chunk.
pub fn parse_pdf(path: &Path) -> Result<Vec<RawChunk>, DocumentError> {
    let bytes = std::fs::read(path)?;
    let text = pdf_extract::extract_text_from_mem(&bytes).map_err(|e| DocumentError::Parse {
        file: path.display().to_string(),
        message: format!("PDF extraction failed: {}", e),
    })?;

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
pub fn parse_docx(path: &Path) -> Result<Vec<RawChunk>, DocumentError> {
    let bytes = std::fs::read(path)?;
    let doc = docx_rs::read_docx(&bytes).map_err(|e| DocumentError::Parse {
        file: path.display().to_string(),
        message: format!("DOCX parsing failed: {}", e),
    })?;

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
        if doc.content_hash.as_deref() == Some(&content_hash) {
            // File unchanged, skip re-indexing
            return Ok(doc.clone());
        }
        // File changed — delete old chunks (triggers handle FTS cleanup)
        sqlx::query("DELETE FROM document_chunks WHERE document_id = ?1")
            .bind(doc.id)
            .execute(pool)
            .await?;
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

    // PII scan + redact each chunk, then insert
    let mut pii_detected = false;
    let mut chunk_count = 0;

    // Get or create the document record first
    let doc = upsert_document(
        pool, folder_id, &file_path_str, &file_name, &ext,
        file_size, &content_hash, 0, false, None,
    ).await?;

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
            "INSERT INTO document_chunks (document_id, chunk_index, section_title, content, char_count)
             VALUES (?1, ?2, ?3, ?4, ?5)"
        )
        .bind(doc.id)
        .bind(i as i64)
        .bind(&chunk.section_title)
        .bind(&redacted_content)
        .bind(char_count)
        .execute(pool)
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
    .execute(pool)
    .await?;

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
            eprintln!("[Documents] Failed to index {}: {}", file_path.display(), e);
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
        if total_chars + content.len() > MAX_DOCUMENT_CONTEXT_CHARS {
            break;
        }
        if result.len() >= MAX_CHUNKS_IN_CONTEXT {
            break;
        }
        total_chars += content.len();
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
// File System Watcher
// ============================================================================

use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::sync::mpsc;
use std::time::Duration;

/// Start watching the active document folder for changes.
/// Runs in a background thread. Debounces events by 2 seconds.
pub fn start_watcher(pool: DbPool) -> Option<std::thread::JoinHandle<()>> {
    // We need a tokio runtime handle for async DB operations
    let rt = match tokio::runtime::Handle::try_current() {
        Ok(h) => h,
        Err(_) => return None,
    };

    let handle = std::thread::spawn(move || {
        let (tx, rx) = mpsc::channel::<notify::Result<Event>>();

        // Check if folder is configured
        let folder_path = match rt.block_on(async {
            let folder = get_document_folder(&pool).await.ok().flatten();
            folder.map(|f| f.path)
        }) {
            Some(p) => p,
            None => return, // No folder configured
        };

        let mut watcher = match RecommendedWatcher::new(tx, Config::default()) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("[Documents] Failed to create watcher: {}", e);
                return;
            }
        };

        if let Err(e) = watcher.watch(Path::new(&folder_path), RecursiveMode::Recursive) {
            eprintln!("[Documents] Failed to watch {}: {}", folder_path, e);
            return;
        }

        println!("[Documents] Watching: {}", folder_path);

        // Debounce: wait for 2 seconds of quiet before scanning
        loop {
            match rx.recv_timeout(Duration::from_secs(2)) {
                Ok(Ok(_event)) => {
                    // Got an event — keep draining for 2 seconds
                    while rx.recv_timeout(Duration::from_secs(2)).is_ok() {}
                    // Debounce period passed — scan
                    println!("[Documents] Changes detected, re-scanning...");
                    let pool_clone = pool.clone();
                    rt.block_on(async {
                        if let Err(e) = scan_folder(&pool_clone).await {
                            eprintln!("[Documents] Re-scan failed: {}", e);
                        }
                    });
                }
                Ok(Err(e)) => {
                    eprintln!("[Documents] Watch error: {}", e);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    // No events — continue waiting
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    println!("[Documents] Watcher channel closed, stopping");
                    break;
                }
            }
        }
    });

    Some(handle)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

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
}
