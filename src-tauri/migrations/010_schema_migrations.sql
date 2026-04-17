-- Schema migrations tracking table.
-- Introduced alongside the transactional migration runner.
-- Each successfully-applied migration inserts a row; the runner skips any
-- version whose row already exists.
--
-- The table itself is bootstrapped by the runner (CREATE TABLE IF NOT EXISTS)
-- before this migration file is evaluated — this file exists so the migration
-- is self-describing in the migrations/ directory, not because the DDL is
-- needed at runtime. On fresh installs this migration is a no-op; on legacy
-- upgrade the runner backfills versions 1..=9 as applied before this file runs.
CREATE TABLE IF NOT EXISTS schema_migrations (
    version INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    applied_at TEXT NOT NULL DEFAULT (datetime('now'))
);
