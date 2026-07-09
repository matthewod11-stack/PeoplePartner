-- Migration 015: audit_log source + status columns (issue #112)
--
-- Backend-initiated LLM egress (memory summaries, review highlights, title
-- generation) now writes audit rows from the chat seam instead of a single
-- frontend fire-and-forget call. Two additive nullable columns record:
--
--   source — where the egress originated: 'interactive', 'memory_summary',
--            'highlight_extraction', 'highlight_summary', 'title_generation'
--   status — how the attempt ended: 'ok', 'error', 'cancelled'
--            (error/cancelled rows are partial: the request left the machine,
--            the response did not complete)
--
-- Pre-015 rows keep NULL in both columns (written by the retired frontend
-- path, all interactive + success by construction). The append-only triggers
-- from migration 011 are unaffected — ALTER TABLE ADD COLUMN is DDL and the
-- UPDATE/DELETE blocks still apply to the new columns.

ALTER TABLE audit_log ADD COLUMN source TEXT;
ALTER TABLE audit_log ADD COLUMN status TEXT;
