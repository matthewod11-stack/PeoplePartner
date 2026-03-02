# Document Ingestion Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Let users point HR Command Center at a folder of HR documents. Alex indexes content, redacts PII, and uses relevant chunks to answer questions with inline citations.

**Architecture:** New `documents.rs` Rust module handles file discovery, parsing (md/txt/csv/pdf/docx/xlsx), section-aware chunking, PII redaction via existing `pii.rs`, and FTS5 indexing. Context builder gets a new `RELEVANT DOCUMENTS` section. Settings UI gets a folder picker component. FSEvents watcher auto-indexes on file changes.

**Tech Stack:** Rust (SQLx, pdf-extract, docx-rs, calamine, sha2, notify), React/TypeScript, Tauri dialog plugin, SQLite FTS5.

**Design Doc:** `docs/plans/2026-03-02-document-ingestion-design.md`

---

## Task 1: Database Migration

**Files:**
- Create: `src-tauri/migrations/007_documents.sql`
- Modify: `src-tauri/src/db.rs:88-97` (add migration to array)

**Step 1: Create migration file**

```sql
-- Migration 007: Document ingestion tables
-- Adds document folder tracking, document index, chunks, and FTS5

CREATE TABLE IF NOT EXISTS document_folders (
    id INTEGER PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    label TEXT,
    active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    last_scanned_at TEXT
);

CREATE TABLE IF NOT EXISTS documents (
    id INTEGER PRIMARY KEY,
    folder_id INTEGER NOT NULL REFERENCES document_folders(id) ON DELETE CASCADE,
    file_path TEXT NOT NULL UNIQUE,
    file_name TEXT NOT NULL,
    file_type TEXT NOT NULL,
    file_size INTEGER,
    content_hash TEXT,
    chunk_count INTEGER NOT NULL DEFAULT 0,
    pii_detected INTEGER NOT NULL DEFAULT 0,
    indexed_at TEXT NOT NULL DEFAULT (datetime('now')),
    error TEXT
);

CREATE INDEX IF NOT EXISTS idx_documents_folder ON documents(folder_id);
CREATE INDEX IF NOT EXISTS idx_documents_hash ON documents(content_hash);

CREATE TABLE IF NOT EXISTS document_chunks (
    id INTEGER PRIMARY KEY,
    document_id INTEGER NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    chunk_index INTEGER NOT NULL,
    section_title TEXT,
    content TEXT NOT NULL,
    char_count INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_chunks_document ON document_chunks(document_id);

CREATE VIRTUAL TABLE IF NOT EXISTS document_chunks_fts USING fts5(
    content,
    section_title,
    content='document_chunks',
    content_rowid='id'
);

-- Triggers to keep FTS in sync with document_chunks
CREATE TRIGGER IF NOT EXISTS document_chunks_ai AFTER INSERT ON document_chunks BEGIN
    INSERT INTO document_chunks_fts(rowid, content, section_title)
    VALUES (new.id, new.content, new.section_title);
END;

CREATE TRIGGER IF NOT EXISTS document_chunks_ad AFTER DELETE ON document_chunks BEGIN
    INSERT INTO document_chunks_fts(document_chunks_fts, rowid, content, section_title)
    VALUES ('delete', old.id, old.content, old.section_title);
END;

CREATE TRIGGER IF NOT EXISTS document_chunks_au AFTER UPDATE ON document_chunks BEGIN
    INSERT INTO document_chunks_fts(document_chunks_fts, rowid, content, section_title)
    VALUES ('delete', old.id, old.content, old.section_title);
    INSERT INTO document_chunks_fts(rowid, content, section_title)
    VALUES (new.id, new.content, new.section_title);
END;
```

**Step 2: Register migration in db.rs**

In `src-tauri/src/db.rs`, add to the `migrations` array inside `run_migrations()` (line ~96):

```rust
    let migrations = [
        include_str!("../migrations/001_initial.sql"),
        include_str!("../migrations/002_performance_enps.sql"),
        include_str!("../migrations/003_review_highlights.sql"),
        include_str!("../migrations/004_insight_canvas.sql"),
        include_str!("../migrations/005_dei_audit.sql"),
        include_str!("../migrations/006_drop_insight_canvas.sql"),
        include_str!("../migrations/007_documents.sql"),  // Document ingestion
    ];
```

**Step 3: Verify migration runs**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: All 354 tests pass (migration runs during test DB init)

**Step 4: Commit**

```bash
git add src-tauri/migrations/007_documents.sql src-tauri/src/db.rs
git commit -m "feat(docs): Add migration 007 for document ingestion tables"
```

---

## Task 2: Add Rust Dependencies

**Files:**
- Modify: `src-tauri/Cargo.toml`

**Step 1: Add new crates**

In `src-tauri/Cargo.toml`, add under `[dependencies]`:

```toml
# Document parsing
pdf-extract = "0.7"
docx-rs = "0.4"
notify = { version = "7", features = ["macos_fsevent"] }
```

Note: `sha2`, `calamine`, and `csv` are already in the dependency list. `notify` needs to be added explicitly — Tauri depends on it transitively but we need it directly.

**Step 2: Verify build**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: Compiles without errors

**Step 3: Commit**

```bash
git add src-tauri/Cargo.toml
git commit -m "feat(docs): Add pdf-extract, docx-rs, notify dependencies"
```

---

## Task 3: Documents Module — Types and Folder CRUD

**Files:**
- Create: `src-tauri/src/documents.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod documents;`)

This task creates the module with types and folder management. Parsing and indexing come in later tasks.

**Step 1: Create documents.rs with types and folder CRUD**

Create `src-tauri/src/documents.rs`:

```rust
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
}
```

**Step 2: Register module in lib.rs**

In `src-tauri/src/lib.rs`, add `mod documents;` alongside the other module declarations.

**Step 3: Run tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml documents`
Expected: 5 tests pass (is_supported_file, discover_files, discover_files_nested, hash_file, file_type)

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: 359+ tests pass (354 existing + 5 new)

**Step 4: Commit**

```bash
git add src-tauri/src/documents.rs src-tauri/src/lib.rs
git commit -m "feat(docs): Add documents module with types, folder CRUD, file discovery"
```

---

## Task 4: File Parsers — Text-Based (.md, .txt, .csv)

**Files:**
- Modify: `src-tauri/src/documents.rs`

**Step 1: Add text-based parsers and section-aware chunker**

Append to `documents.rs` after the file discovery section:

```rust
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
```

**Step 2: Add parser tests**

```rust
// Add to #[cfg(test)] mod tests:

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
        // Create a chunk larger than MAX_CHUNK_CHARS
        let big_content = "A ".repeat(MAX_CHUNK_CHARS + 1000);
        let chunks = vec![RawChunk {
            section_title: Some("Big Section".to_string()),
            content: big_content,
        }];
        let result = split_oversized_chunks(chunks);
        assert!(result.len() > 1);
        assert_eq!(result[0].section_title.as_deref(), Some("Big Section"));
    }
```

**Step 3: Run tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml documents`
Expected: 10 tests pass (5 existing + 5 new)

**Step 4: Commit**

```bash
git add src-tauri/src/documents.rs
git commit -m "feat(docs): Add text-based parsers (markdown, plaintext, CSV)"
```

---

## Task 5: File Parsers — Binary Formats (.pdf, .docx, .xlsx)

**Files:**
- Modify: `src-tauri/src/documents.rs`

**Step 1: Add binary format parsers**

Append to `documents.rs`:

```rust
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

    if chunks.is_empty() {
        // Fallback: treat entire extracted text as one chunk
        let all_text: String = chunks.iter().map(|c| c.content.as_str()).collect::<Vec<_>>().join("\n");
        if !all_text.trim().is_empty() {
            chunks.push(RawChunk {
                section_title: None,
                content: all_text.trim().to_string(),
            });
        }
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
```

**Step 2: Add parser dispatch test**

```rust
// Add to tests module:

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
```

**Step 3: Run tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml documents`
Expected: 11 tests pass

**Step 4: Commit**

```bash
git add src-tauri/src/documents.rs
git commit -m "feat(docs): Add binary format parsers (PDF, DOCX, XLSX) + dispatch"
```

---

## Task 6: Indexing Pipeline — Scan, Parse, Redact, Store

**Files:**
- Modify: `src-tauri/src/documents.rs`

**Step 1: Add the indexing pipeline**

Append to `documents.rs`:

```rust
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
```

**Step 2: Run full test suite**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: All tests pass (existing + new)

**Step 3: Commit**

```bash
git add src-tauri/src/documents.rs
git commit -m "feat(docs): Add indexing pipeline — scan, parse, PII redact, FTS store"
```

---

## Task 7: FTS Retrieval for Context Builder

**Files:**
- Modify: `src-tauri/src/documents.rs`

**Step 1: Add FTS retrieval function**

Append to `documents.rs`:

```rust
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
```

**Step 2: Add retrieval tests**

```rust
// Add to tests module:

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
```

**Step 3: Run tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml documents`
Expected: 13+ tests pass

**Step 4: Commit**

```bash
git add src-tauri/src/documents.rs
git commit -m "feat(docs): Add FTS5 retrieval and document context formatter"
```

---

## Task 8: Context Builder Integration

**Files:**
- Modify: `src-tauri/src/context.rs`

**Step 1: Add document retrieval to `build_chat_context()`**

In `context.rs`, after the memory lookup (around line 2756), add document retrieval:

```rust
    // Step 6: Find relevant document chunks (resilient - don't fail if lookup errors)
    let document_chunks: Vec<crate::documents::RetrievedChunk> =
        match crate::documents::search_documents(pool, user_message).await {
            Ok(chunks) => chunks,
            Err(e) => {
                eprintln!("Warning: Failed to search documents: {}", e);
                Vec::new()
            }
        };
```

**Step 2: Add `document_chunks` field to `ChatContext` struct**

In `context.rs`, add to the `ChatContext` struct (around line 500):

```rust
pub struct ChatContext {
    pub company: Option<CompanyContext>,
    pub aggregates: Option<OrgAggregates>,
    pub query_type: QueryType,
    pub employees: Vec<EmployeeContext>,
    pub employee_summaries: Vec<EmployeeSummary>,
    pub employee_ids_used: Vec<String>,
    pub memory_summaries: Vec<String>,
    pub document_chunks: Vec<crate::documents::RetrievedChunk>,  // V3.0: Document context
    pub metrics: RetrievalMetrics,
}
```

**Step 3: Include document_chunks in ChatContext construction**

In `build_chat_context()`, add `document_chunks` to the `Ok(ChatContext { ... })` return.

**Step 4: Add document context to `get_system_prompt_for_message()`**

After the employee context formatting (around line 2844), add:

```rust
    // Build document context (V3.0)
    let document_context = crate::documents::format_document_context(&context.document_chunks);
```

**Step 5: Update `build_system_prompt()` signature and body**

Add `document_context: &str` parameter and include in the prompt template:

```rust
pub fn build_system_prompt(
    company: Option<&CompanyContext>,
    aggregates: Option<&OrgAggregates>,
    employee_context: &str,
    document_context: &str,  // V3.0
    memory_summaries: &[String],
    user_name: Option<&str>,
    persona_id: Option<&str>,
) -> String {
```

Add to the format string, between `{employee_section}` and `RELEVANT PAST CONVERSATIONS`:

```rust
    let document_section = if document_context.is_empty() {
        String::new()
    } else {
        format!("\nRELEVANT DOCUMENTS:\n{}", document_context)
    };
```

And add citation instructions to the CONTEXT AWARENESS section:

```
- When answering from company documents, cite the source naturally (e.g., "According to your Employee Handbook...")
- If document content conflicts with general knowledge, prefer the company's documented policy
```

**Step 6: Update `build_system_prompt()` call site**

In `get_system_prompt_for_message()`, update the call:

```rust
    let system_prompt = build_system_prompt(
        context.company.as_ref(),
        context.aggregates.as_ref(),
        &employee_context,
        &document_context,
        &context.memory_summaries,
        user_name.as_deref(),
        persona_id.as_deref(),
    );
```

**Step 7: Update existing tests that call `build_system_prompt()`**

Search for all test calls to `build_system_prompt` and add the new `document_context` parameter (pass `""` for existing tests).

**Step 8: Run tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: All tests pass (may need to fix test calls with new parameter)

**Step 9: Commit**

```bash
git add src-tauri/src/context.rs
git commit -m "feat(docs): Wire document retrieval into context builder + system prompt"
```

---

## Task 9: Tauri Commands

**Files:**
- Modify: `src-tauri/src/lib.rs`

**Step 1: Add 5 Tauri commands**

Add to `lib.rs`, in the commands section:

```rust
// ============================================================================
// Document Ingestion Commands (V3.0)
// ============================================================================

#[tauri::command]
async fn set_document_folder(
    state: tauri::State<'_, crate::db::DbPool>,
    path: String,
) -> Result<documents::DocumentFolderStats, String> {
    let pool = state.inner();
    documents::set_document_folder(pool, &path)
        .await
        .map_err(|e| e.to_string())?;
    documents::scan_folder(pool)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn remove_document_folder(
    state: tauri::State<'_, crate::db::DbPool>,
) -> Result<(), String> {
    documents::remove_document_folder(state.inner())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_document_folder(
    state: tauri::State<'_, crate::db::DbPool>,
) -> Result<Option<documents::DocumentFolderStats>, String> {
    documents::get_folder_stats(state.inner())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn rescan_documents(
    state: tauri::State<'_, crate::db::DbPool>,
) -> Result<documents::DocumentFolderStats, String> {
    documents::scan_folder(state.inner())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_document_stats(
    state: tauri::State<'_, crate::db::DbPool>,
) -> Result<Option<documents::DocumentFolderStats>, String> {
    documents::get_folder_stats(state.inner())
        .await
        .map_err(|e| e.to_string())
}
```

**Step 2: Register in generate_handler!**

Add to the `generate_handler!` macro call:

```rust
            // Document ingestion
            set_document_folder,
            remove_document_folder,
            get_document_folder,
            rescan_documents,
            get_document_stats,
```

**Step 3: Verify build**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: Compiles cleanly

**Step 4: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat(docs): Register 5 document ingestion Tauri commands"
```

---

## Task 10: TypeScript Wrappers + Types

**Files:**
- Modify: `src/lib/tauri-commands.ts`
- Modify: `src/lib/types.ts`

**Step 1: Add types to types.ts**

```typescript
/** Stats for the configured document folder */
export interface DocumentFolderStats {
  path: string;
  label: string | null;
  file_count: number;
  chunk_count: number;
  pii_file_count: number;
  error_file_count: number;
  last_scanned_at: string | null;
}
```

**Step 2: Add command wrappers to tauri-commands.ts**

```typescript
// =============================================================================
// Document Ingestion (V3.0)
// =============================================================================

/** Set the document folder path and trigger initial scan */
export async function setDocumentFolder(path: string): Promise<DocumentFolderStats> {
  return invoke('set_document_folder', { path });
}

/** Remove the document folder and all indexed data */
export async function removeDocumentFolder(): Promise<void> {
  return invoke('remove_document_folder');
}

/** Get the current document folder stats (null if none configured) */
export async function getDocumentFolder(): Promise<DocumentFolderStats | null> {
  return invoke('get_document_folder');
}

/** Trigger a manual re-scan of the document folder */
export async function rescanDocuments(): Promise<DocumentFolderStats> {
  return invoke('rescan_documents');
}

/** Get document indexing stats */
export async function getDocumentStats(): Promise<DocumentFolderStats | null> {
  return invoke('get_document_stats');
}
```

**Step 3: Verify types**

Run: `npx tsc --noEmit`
Expected: Clean

**Step 4: Commit**

```bash
git add src/lib/tauri-commands.ts src/lib/types.ts
git commit -m "feat(docs): Add TypeScript types and Tauri command wrappers for documents"
```

---

## Task 11: Settings UI — DocumentFolderConfig Component

**Files:**
- Create: `src/components/settings/DocumentFolderConfig.tsx`
- Modify: `src/components/settings/SettingsPanel.tsx`

**Step 1: Create DocumentFolderConfig component**

Create `src/components/settings/DocumentFolderConfig.tsx`:

```typescript
import { useState, useCallback, useEffect } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import {
  getDocumentFolder,
  setDocumentFolder,
  removeDocumentFolder,
  rescanDocuments,
} from '../../lib/tauri-commands';
import type { DocumentFolderStats } from '../../lib/types';

interface DocumentFolderConfigProps {
  compact?: boolean;
}

export function DocumentFolderConfig({ compact = false }: DocumentFolderConfigProps) {
  const [stats, setStats] = useState<DocumentFolderStats | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [isScanning, setIsScanning] = useState(false);
  const [error, setError] = useState('');

  useEffect(() => {
    setIsLoading(true);
    getDocumentFolder()
      .then(setStats)
      .catch(() => setStats(null))
      .finally(() => setIsLoading(false));
  }, []);

  const handleChooseFolder = useCallback(async () => {
    try {
      const selected = await open({ directory: true, multiple: false });
      if (!selected) return; // User cancelled

      setIsScanning(true);
      setError('');
      const result = await setDocumentFolder(selected as string);
      setStats(result);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsScanning(false);
    }
  }, []);

  const handleRescan = useCallback(async () => {
    setIsScanning(true);
    setError('');
    try {
      const result = await rescanDocuments();
      setStats(result);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsScanning(false);
    }
  }, []);

  const handleRemove = useCallback(async () => {
    try {
      await removeDocumentFolder();
      setStats(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }, []);

  if (isLoading) {
    return <div className="p-4 text-sm text-stone-500">Loading...</div>;
  }

  // Scanning state
  if (isScanning) {
    return (
      <div className="p-4 bg-stone-50 border border-stone-200 rounded-xl">
        <div className="flex items-center gap-3">
          <svg className="w-5 h-5 text-primary-500 animate-spin" fill="none" viewBox="0 0 24 24">
            <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
            <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
          </svg>
          <p className="text-sm text-stone-600">Indexing documents...</p>
        </div>
      </div>
    );
  }

  // No folder configured — empty state
  if (!stats) {
    return (
      <div className="p-4 bg-stone-50 border border-stone-200 rounded-xl">
        <div className="text-center space-y-3">
          <div className="w-10 h-10 mx-auto rounded-full bg-stone-200 flex items-center justify-center">
            <svg className="w-5 h-5 text-stone-500" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
            </svg>
          </div>
          <div>
            <p className="text-sm text-stone-700">
              Point Alex at a folder of HR documents — policies, handbooks, meeting notes —
              and Alex will use them to answer your questions.
            </p>
            <p className="text-xs text-stone-500 mt-1">
              Files stay on your machine. Sensitive data is automatically redacted.
            </p>
          </div>
          <button
            type="button"
            onClick={handleChooseFolder}
            className="px-4 py-2 text-sm font-medium text-white bg-primary-500 hover:bg-primary-600 rounded-lg transition-colors"
          >
            Choose Folder
          </button>
        </div>
        {error && <p className="mt-2 text-sm text-red-600">{error}</p>}
      </div>
    );
  }

  // Folder configured — show stats
  const folderName = stats.path.split('/').pop() || stats.path;

  // Format last scanned time
  const lastScan = stats.last_scanned_at
    ? new Date(stats.last_scanned_at + 'Z').toLocaleString()
    : 'Never';

  return (
    <div className="p-4 bg-stone-50 border border-stone-200 rounded-xl space-y-3">
      <div className="flex items-center justify-between gap-3">
        <div className="flex items-center gap-3 min-w-0">
          <div className="w-8 h-8 flex-shrink-0 flex items-center justify-center rounded-full bg-primary-100">
            <svg className="w-4 h-4 text-primary-600" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
            </svg>
          </div>
          <div className="min-w-0">
            <p className="text-sm font-medium text-stone-700 truncate" title={stats.path}>
              {folderName}
            </p>
            <p className="text-xs text-stone-500">
              {stats.file_count} files indexed
              {stats.chunk_count > 0 && ` · ${stats.chunk_count} sections`}
              {stats.last_scanned_at && ` · ${lastScan}`}
            </p>
          </div>
        </div>
        <button
          type="button"
          onClick={handleChooseFolder}
          className="flex-shrink-0 text-sm text-primary-600 hover:text-primary-700"
        >
          Change
        </button>
      </div>

      {/* PII warning */}
      {stats.pii_file_count > 0 && (
        <div className="flex items-center gap-2 px-3 py-2 bg-amber-50 border border-amber-200 rounded-lg">
          <svg className="w-4 h-4 text-amber-600 flex-shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
          </svg>
          <p className="text-xs text-amber-800">
            {stats.pii_file_count} file{stats.pii_file_count > 1 ? 's' : ''} contained sensitive data (auto-redacted)
          </p>
        </div>
      )}

      {/* Error warning */}
      {stats.error_file_count > 0 && (
        <div className="flex items-center gap-2 px-3 py-2 bg-red-50 border border-red-200 rounded-lg">
          <svg className="w-4 h-4 text-red-600 flex-shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
            <path strokeLinecap="round" strokeLinejoin="round" d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
          <p className="text-xs text-red-800">
            {stats.error_file_count} file{stats.error_file_count > 1 ? 's' : ''} could not be parsed
          </p>
        </div>
      )}

      {/* Actions */}
      <div className="flex gap-2">
        <button
          type="button"
          onClick={handleRescan}
          className="px-3 py-1.5 text-sm text-stone-600 hover:text-stone-800 hover:bg-stone-100 rounded-lg transition-colors"
        >
          Re-scan Now
        </button>
        <button
          type="button"
          onClick={handleRemove}
          className="px-3 py-1.5 text-sm text-red-600 hover:text-red-700 hover:bg-red-50 rounded-lg transition-colors"
        >
          Remove
        </button>
      </div>

      {error && <p className="text-sm text-red-600">{error}</p>}
    </div>
  );
}

export default DocumentFolderConfig;
```

**Step 2: Add Documents section to SettingsPanel**

In `src/components/settings/SettingsPanel.tsx`, add import:

```typescript
import { DocumentFolderConfig } from './DocumentFolderConfig';
```

Add between the "AI Provider" section and "Company Profile" section:

```tsx
          {/* Documents Section (V3.0) */}
          <section>
            <h3 className="text-sm font-medium text-stone-500 uppercase tracking-wider mb-3">
              Documents
            </h3>
            <DocumentFolderConfig compact />
          </section>
```

**Step 3: Verify build**

Run: `npx tsc --noEmit && npm run build`
Expected: Both pass

**Step 4: Commit**

```bash
git add src/components/settings/DocumentFolderConfig.tsx src/components/settings/SettingsPanel.tsx
git commit -m "feat(docs): Add DocumentFolderConfig UI component + wire into Settings"
```

---

## Task 12: FSEvents Watcher

**Files:**
- Modify: `src-tauri/src/documents.rs`
- Modify: `src-tauri/src/lib.rs` (start watcher on app launch)

**Step 1: Add watcher to documents.rs**

Append to `documents.rs`:

```rust
// ============================================================================
// File System Watcher
// ============================================================================

use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::sync::mpsc;
use std::time::Duration;

/// Start watching the active document folder for changes.
/// Runs in a background thread. Debounces events by 2 seconds.
/// Returns a handle that stops the watcher when dropped.
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
```

**Step 2: Start watcher on app launch**

In `src-tauri/src/lib.rs`, in the `run()` function, after the DB pool is initialized and managed, start the watcher:

```rust
    // Start document folder watcher (V3.0)
    let watcher_pool = pool.clone();
    documents::start_watcher(watcher_pool);
```

This will need to be placed in the appropriate setup hook. Check `lib.rs` for the existing pattern of how the pool is set up and managed.

**Step 3: Verify build**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`
Expected: Compiles cleanly

**Step 4: Commit**

```bash
git add src-tauri/src/documents.rs src-tauri/src/lib.rs
git commit -m "feat(docs): Add FSEvents watcher with 2-second debounce"
```

---

## Task 13: Tauri Dialog Plugin Setup

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/capabilities/` or `tauri.conf.json` (permissions)

**Step 1: Add dialog plugin dependency**

Check if `tauri-plugin-dialog` is already present. If not, add to `Cargo.toml`:

```toml
tauri-plugin-dialog = "2"
```

**Step 2: Register plugin in lib.rs**

In the `tauri::Builder` chain, add:

```rust
.plugin(tauri_plugin_dialog::init())
```

**Step 3: Add dialog permissions**

In the Tauri capabilities/permissions config, ensure `dialog:default` is allowed. Check `src-tauri/capabilities/` for the correct file.

**Step 4: Verify build**

Run: `cargo check --manifest-path src-tauri/Cargo.toml && npx tsc --noEmit`
Expected: Both pass

**Step 5: Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/lib.rs
git commit -m "feat(docs): Add tauri-plugin-dialog for native folder picker"
```

---

## Task 14: Full Integration Verification

**Files:**
- None (verification only)

**Step 1: Run full test suite**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: All tests pass (354 existing + ~15 new document tests)

**Step 2: Type check**

Run: `npx tsc --noEmit`
Expected: Clean

**Step 3: Production build**

Run: `npm run build`
Expected: Build succeeds

**Step 4: Manual E2E test plan**

Run `cargo tauri dev` and verify:
1. Open Settings — Documents section shows "Choose Folder" empty state
2. Click "Choose Folder" — native macOS folder picker appears
3. Select a test folder with .md, .txt, .pdf files — scanning indicator shows
4. After scan — shows file count, chunk count, last scan time
5. If folder has PII — amber warning shows "X files contained sensitive data (auto-redacted)"
6. Click "Re-scan Now" — re-indexes successfully
7. Click "Remove" — returns to empty state
8. Ask Alex a question about document content — Alex cites the source document
9. Ask Alex about an employee AND a policy — both employee data and document content appear

**Step 5: Commit verification results**

```bash
git commit -m "verify: Document ingestion E2E testing complete"
```

---

## Task 15: Update Tracking Files

**Files:**
- Modify: `docs/PROGRESS.md`
- Modify: `features.json`
- Modify: `ROADMAP_LAUNCH_PREP.md` or create V3 roadmap section

**Step 1: Add features.json entry**

```json
"document-ingestion": {
  "status": "pass",
  "notes": "V3.0: Folder/file access — .md/.txt/.csv/.pdf/.docx/.xlsx, FTS5 retrieval, PII redaction, FSEvents watcher, Settings UI, context builder integration."
}
```

**Step 2: Add PROGRESS.md session entry**

Document all completed work, files modified, test counts.

**Step 3: Commit**

```bash
git add docs/PROGRESS.md features.json
git commit -m "docs: Update tracking files for document ingestion feature"
```
