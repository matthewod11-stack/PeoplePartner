-- Migration 009: License validation cache
-- Caches server-side license validation results for offline grace period support.
-- A license key must be validated at least once online before offline use is permitted.

CREATE TABLE IF NOT EXISTS license_validation_cache (
    license_key TEXT PRIMARY KEY,
    device_id TEXT NOT NULL,
    validated_at TEXT NOT NULL,
    server_status TEXT NOT NULL,
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT DEFAULT (datetime('now'))
);
