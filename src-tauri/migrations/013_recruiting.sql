-- Migration 013: Recruiting (Sourcerer) — skeleton schema.
--
-- recruiting_searches is the top-level "run" container. Candidates,
-- evidence, scores, and outputs hang off it in later migrations as the
-- module earns them.
--
-- Notes:
--   - Lives in its own migration so the module can be cleanly removed
--     later (DECISIONS.md architecture finding #1).
--   - seed_employee_id is the context-aware sourcing seam: "find people
--     like this employee". Nullable + ON DELETE SET NULL so deleting an
--     employee never cascades into recruiting history.
--   - status is TEXT, not a CHECK enum — the Rust enum is the source of
--     truth. Promote to a CHECK constraint once the value space settles.

CREATE TABLE IF NOT EXISTS recruiting_searches (
    id TEXT PRIMARY KEY,
    query TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    seed_employee_id TEXT REFERENCES employees(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_recruiting_searches_created_at
    ON recruiting_searches(created_at DESC);

CREATE INDEX IF NOT EXISTS idx_recruiting_searches_seed_employee
    ON recruiting_searches(seed_employee_id)
    WHERE seed_employee_id IS NOT NULL;
