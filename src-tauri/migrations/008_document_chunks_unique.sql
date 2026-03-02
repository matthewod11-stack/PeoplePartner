-- Migration 008: Add UNIQUE constraint on document_chunks
-- Prevents duplicate chunks from concurrent scan/watcher races

CREATE UNIQUE INDEX IF NOT EXISTS idx_chunks_doc_index ON document_chunks(document_id, chunk_index);
