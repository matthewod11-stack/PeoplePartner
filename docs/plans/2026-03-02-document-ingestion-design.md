# Document Ingestion — Design Document

> **Created:** 2026-03-02
> **Status:** Approved
> **Phase:** V3.0 — Pre-launch feature
> **Summary:** Let users point HR Command Center at a folder of HR documents (policies, handbooks, meeting notes, 1:1 notes). Alex indexes the content and uses it to answer questions with inline citations.

---

## Decisions Made

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Retrieval method | FTS5 now, embeddings later | Zero dependencies, HR docs use predictable terminology, proven pattern in codebase |
| File types | .md, .txt, .csv, .pdf, .docx, .xlsx | Covers ~99% of real HR doc folders |
| Folder sync | Auto-watch (FSEvents) + manual re-scan button | Best UX — drop a file and it's available within seconds |
| UI placement | Settings only (for now) | Ship capability, not chrome. Sidebar tab can come later |
| Citations | Inline via prompt instruction | "According to your Employee Handbook..." — no special UI needed |
| PII handling | Scan-and-redact on index | Consistent with chat PII behavior. Original files untouched. PII never reaches LLM context |
| Chunking | Section-aware | Split on headings/pages for natural boundaries and better citations |

---

## Architecture

### Data Flow

```
File detected (FSEvents or manual scan)
        │
        ▼
  ┌─ Change Detection ──────┐
  │  SHA-256 hash file       │
  │  Compare to documents.   │
  │  content_hash            │
  │  Skip if unchanged       │
  └────────┬────────────────┘
           ▼
  ┌─ Parser (by file type) ─┐
  │  .md  → markdown parser  │
  │  .txt → plain text       │
  │  .csv → row grouping     │
  │  .pdf → pdf-extract      │
  │  .docx → docx-rs         │
  │  .xlsx → calamine         │
  └────────┬────────────────┘
           ▼
  ┌─ Section-Aware Chunker ─┐
  │  Split on headings/pages │
  │  Track section titles    │
  │  Cap at ~1000 tokens     │
  └────────┬────────────────┘
           ▼
  ┌─ PII Scanner ───────────┐
  │  Reuse existing scanner  │
  │  Redact SSN, CC, bank#   │
  │  in chunk content        │
  │  Flag doc if PII found   │
  └────────┬────────────────┘
           ▼
  ┌─ FTS Indexer ───────────┐
  │  DELETE old chunks       │
  │  INSERT redacted chunks  │
  │  UPDATE FTS5 index       │
  │  UPDATE document hash    │
  └─────────────────────────┘
```

### Context Integration

During chat, the context builder:
1. Extracts keywords from user query (existing `extract_query_intent()`)
2. Runs FTS5 query against `document_chunks_fts`
3. Ranks by relevance, takes top 3-5 chunks within 4K token budget
4. Inserts as `RELEVANT DOCUMENTS:` section in system prompt
5. Prompt instruction tells Alex to cite sources naturally

### System Prompt Structure (Updated)

```
PERSONA PREAMBLE
COMMUNICATION STYLE
COMPANY CONTEXT
ORGANIZATION DATA
CONTEXT AWARENESS          ← add citation instructions
BOUNDARIES
RELEVANT EMPLOYEES         ← 4K token budget (unchanged)
RELEVANT DOCUMENTS         ← 4K token budget (NEW)
RELEVANT PAST CONVERSATIONS
```

---

## Data Model

### New Tables (via migration)

```sql
-- Watched folder configuration
CREATE TABLE document_folders (
    id INTEGER PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    label TEXT,
    active INTEGER DEFAULT 1,
    created_at TEXT DEFAULT (datetime('now')),
    last_scanned_at TEXT
);

-- Indexed documents (one row per file)
CREATE TABLE documents (
    id INTEGER PRIMARY KEY,
    folder_id INTEGER NOT NULL REFERENCES document_folders(id),
    file_path TEXT NOT NULL UNIQUE,
    file_name TEXT NOT NULL,
    file_type TEXT NOT NULL,  -- 'md', 'pdf', 'docx', 'xlsx', 'csv', 'txt'
    file_size INTEGER,
    content_hash TEXT,        -- SHA-256 for change detection
    chunk_count INTEGER DEFAULT 0,
    pii_detected INTEGER DEFAULT 0,
    indexed_at TEXT DEFAULT (datetime('now')),
    error TEXT
);

-- Document chunks for FTS retrieval
CREATE TABLE document_chunks (
    id INTEGER PRIMARY KEY,
    document_id INTEGER NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    chunk_index INTEGER NOT NULL,
    section_title TEXT,
    content TEXT NOT NULL,
    char_count INTEGER NOT NULL
);

-- FTS5 virtual table
CREATE VIRTUAL TABLE document_chunks_fts USING fts5(
    content,
    section_title,
    content='document_chunks',
    content_rowid='id'
);
```

---

## Ingestion Pipeline Details

### Change Detection
- Compute SHA-256 of file content on scan
- Compare to `documents.content_hash`
- Skip re-indexing if hash matches
- Detect deleted files (in DB but not on disk) and remove their chunks

### Parser Dispatch
| Extension | Crate | Chunking strategy |
|-----------|-------|-------------------|
| `.md` | Built-in | Split on `##` headings |
| `.txt` | Built-in | Split on blank-line paragraphs |
| `.csv` | Built-in | Group rows (e.g., 20 per chunk) |
| `.pdf` | `pdf-extract` | Split on page boundaries |
| `.docx` | `docx-rs` | Split on heading styles |
| `.xlsx` | `calamine` | Sheet name as section, group rows |

### PII Scanning
- Reuse existing `pii.rs` scanner on each chunk
- Replace PII matches with `[REDACTED-SSN]`, `[REDACTED-CC]`, etc.
- Set `documents.pii_detected = 1` if any chunk had PII
- Original files are never modified

### FSEvents Watcher
- Use `notify` crate (already in Tauri dependency tree)
- Debounce: wait 2 seconds of quiet after last filesystem event before re-scanning
- Runs on background thread, never blocks UI
- Emits Tauri events for progress updates
- Starts on app launch if a folder is configured; stops if folder is removed

---

## Tauri Commands

| Command | Signature | Purpose |
|---------|-----------|---------|
| `set_document_folder` | `(path: String) → DocumentFolderStats` | Set watched folder, trigger initial scan |
| `remove_document_folder` | `() → ()` | Remove folder and all indexed data |
| `get_document_folder` | `() → Option<DocumentFolderStats>` | Get current folder config + stats |
| `rescan_documents` | `() → DocumentFolderStats` | Manual re-index trigger |
| `get_document_stats` | `() → DocumentStats` | File count, last scan, PII count, errors |

### Types

```rust
struct DocumentFolderStats {
    path: String,
    label: Option<String>,
    file_count: u32,
    chunk_count: u32,
    pii_file_count: u32,
    error_file_count: u32,
    last_scanned_at: Option<String>,
}

struct DocumentStats {
    total_files: u32,
    total_chunks: u32,
    files_with_pii: u32,
    files_with_errors: u32,
    files_by_type: HashMap<String, u32>,
    last_scanned_at: Option<String>,
}
```

---

## UI Design

### Settings Panel — Documents Section

**Empty state (no folder configured):**
```
┌─────────────────────────────────────────┐
│  Point Alex at a folder of HR           │
│  documents — policies, handbooks,       │
│  meeting notes — and Alex will use      │
│  them to answer your questions.         │
│                                         │
│  Files stay on your machine. Sensitive  │
│  data is automatically redacted.        │
│                                         │
│           [Choose Folder]               │
└─────────────────────────────────────────┘
```

**Indexed state:**
```
┌─────────────────────────────────────────┐
│  📁 /Users/me/HR Docs        [Change]  │
│                                         │
│  23 files indexed • 2 min ago           │
│  ⚠ 2 files had PII (auto-redacted)     │
│                                         │
│  [Re-scan Now]              [Remove]    │
└─────────────────────────────────────────┘
```

**Indexing state:**
```
┌─────────────────────────────────────────┐
│  📁 /Users/me/HR Docs                  │
│                                         │
│  ◐ Indexing 12 of 23 files...           │
└─────────────────────────────────────────┘
```

### No onboarding step
Documents are optional — not part of the setup wizard.

---

## Module Structure

### New Files
| File | Purpose |
|------|---------|
| `src-tauri/src/documents.rs` | Core module (~800-1000 LOC) |
| `src/components/settings/DocumentFolderConfig.tsx` | Settings UI component |

### Modified Files
| File | Change |
|------|--------|
| `src-tauri/src/lib.rs` | Register 5 new Tauri commands |
| `src-tauri/src/context.rs` | Add document retrieval + RELEVANT DOCUMENTS prompt section |
| `src-tauri/src/db.rs` | New migration for 3 tables |
| `src-tauri/Cargo.toml` | Add pdf-extract, docx-rs, calamine, sha2 |
| `src/lib/tauri-commands.ts` | 5 new command wrappers + types |
| `src/components/settings/SettingsPanel.tsx` | New Documents section |

### New Rust Dependencies
| Crate | Purpose |
|-------|---------|
| `pdf-extract` | PDF text extraction |
| `docx-rs` | .docx parsing |
| `calamine` | .xlsx/.xls reading |
| `sha2` | File content hashing |

---

## Token Budget

| Section | Budget | Notes |
|---------|--------|-------|
| Persona + company + org data | ~8K | Unchanged |
| Employee context | 4K | Unchanged |
| Document context | 4K | NEW — top 3-5 chunks |
| Memories | ~4K | Unchanged |
| **Total system prompt** | **~20K** | Within existing budget |

---

## Future Enhancements (Not in V3.0)

- **Embeddings + vector search** — hybrid retrieval (FTS + semantic reranking)
- **Sidebar documents tab** — browse indexed files, see what Alex knows
- **Obsidian-aware parsing** — parse YAML frontmatter, wikilinks, tags as structured metadata
- **Transcript intelligence** — meeting transcript analysis, action item extraction, employee timeline
- **Multiple folders** — schema supports it, UI limits to one for V3.0
