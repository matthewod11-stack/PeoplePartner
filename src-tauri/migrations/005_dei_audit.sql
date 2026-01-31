-- Migration 005: DEI Audit Trail
-- V2.4.2: Adds query_category column to audit_log for filtering DEI queries

-- ============================================================================
-- 1. ADD QUERY_CATEGORY COLUMN
-- ============================================================================
-- Allows categorizing audit entries by query type (e.g., 'dei', 'general')

ALTER TABLE audit_log ADD COLUMN query_category TEXT;

-- ============================================================================
-- 2. INDEX FOR EFFICIENT CATEGORY FILTERING
-- ============================================================================
-- Enables fast queries like: SELECT * FROM audit_log WHERE query_category = 'dei'

CREATE INDEX IF NOT EXISTS idx_audit_category ON audit_log(query_category);
