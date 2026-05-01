-- Audit log append-only enforcement (issue #21).
--
-- Two changes, in one migration so the end state is coherent:
--
-- 1. Drop the audit_log.conversation_id FK entirely (previously
--    "FOREIGN KEY (conversation_id) REFERENCES conversations(id)" with
--    NO ACTION on delete). No code JOINs audit_log back to conversations
--    (verified via grep), so the FK was only enforcing a referential
--    link we don't rely on. Keeping the FK would force a choice between:
--      (a) NO ACTION — breaks conversation deletion once the conversation
--          has audit rows, which was the original problem delete_conversation()
--          worked around by manually wiping audit rows (itself a tampering
--          vector: delete conversation → audit trail vanishes), and
--      (b) SET NULL — fires as an implicit UPDATE on audit_log, which the
--          append-only trigger below would (correctly) block.
--    Dropping the FK means audit_log.conversation_id becomes a plain
--    identifier: it may point at a now-deleted conversation, but the
--    audit row survives and its record of what-was-asked is preserved.
--
-- 2. BEFORE UPDATE / BEFORE DELETE triggers that ABORT any mutation.
--    Defends against casual tampering via DB Browser / sqlite3 CLI /
--    other apps with write access. Filesystem-level tampering is out of
--    scope per the threat model (local-first, user owns the DB file) —
--    the backup-restore code path temporarily drops these triggers and
--    is gated by the backup encryption password.
--
-- SQLite doesn't support ALTER TABLE ... DROP CONSTRAINT, so dropping the
-- FK requires the standard rebuild-rename dance. No other table has a FK
-- pointing at audit_log (verified via grep), so PRAGMA foreign_keys=OFF
-- is not required.

CREATE TABLE audit_log_new (
    id TEXT PRIMARY KEY,
    conversation_id TEXT,
    request_redacted TEXT NOT NULL,
    response_text TEXT NOT NULL,
    context_used TEXT,
    created_at TEXT DEFAULT (datetime('now')),
    query_category TEXT
);

INSERT INTO audit_log_new (
    id, conversation_id, request_redacted, response_text,
    context_used, created_at, query_category
)
SELECT
    id, conversation_id, request_redacted, response_text,
    context_used, created_at, query_category
FROM audit_log;

DROP TABLE audit_log;

ALTER TABLE audit_log_new RENAME TO audit_log;

CREATE INDEX IF NOT EXISTS idx_audit_log_created ON audit_log(created_at);
CREATE INDEX IF NOT EXISTS idx_audit_log_conversation ON audit_log(conversation_id);
CREATE INDEX IF NOT EXISTS idx_audit_category ON audit_log(query_category);

-- BEGIN must appear at the end of a line for the migration runner's
-- statement-splitter to track the trigger body as a single statement. See
-- the BEGIN-detection logic in db::split_sql_statements.
CREATE TRIGGER IF NOT EXISTS audit_log_no_update BEFORE UPDATE ON audit_log BEGIN
    SELECT RAISE(ABORT, 'audit_log is append-only');
END;

CREATE TRIGGER IF NOT EXISTS audit_log_no_delete BEFORE DELETE ON audit_log BEGIN
    SELECT RAISE(ABORT, 'audit_log is append-only');
END;
