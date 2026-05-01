//! Document Ingestion Module — split into submodules per #40b.
//!
//! Pipeline: Discover → Parse → Chunk → PII Redact → FTS Index → Retrieve.
//! The split is structural only — all behavior, public types, and registered
//! Tauri command names are unchanged from the pre-split single file.
//!
//! - `discovery` — file discovery + hashing
//! - `chunker` — text/binary parsers and chunk splitting
//! - `ingest` — error types, data types, folder mgmt, index pipeline, FTS search
//! - `watcher` — `WatcherState` + folder-missing detection (issue #38)

mod chunker;
mod discovery;
mod ingest;
mod watcher;

// ---- Public surface preserved from the pre-split file ----

pub use chunker::{
    parse_csv, parse_docx, parse_file, parse_markdown, parse_pdf, parse_plaintext, parse_xlsx,
    RawChunk,
};
pub use discovery::{discover_files, file_type, hash_file};
pub use ingest::{
    format_document_context, get_document_folder, get_document_stats, get_folder_stats,
    remove_document_folder, scan_folder, search_documents, set_document_folder, Document,
    DocumentChunk, DocumentError, DocumentFolder, DocumentFolderStats, DocumentStats,
    RetrievedChunk, MAX_DOCUMENT_CONTEXT_TOKENS,
};
pub use watcher::{start_watcher, WatcherState, FOLDER_MISSING_EVENT};
