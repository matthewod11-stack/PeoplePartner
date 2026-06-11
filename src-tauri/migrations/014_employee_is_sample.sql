-- #106: Tag sample/test-data rows so the trial employee limit counts only
-- real employees. Onboarding auto-loads 100 Acme Corp sample employees
-- through the bulk-import path; counting them tripped an undismissable
-- "Trial complete" paywall on every fresh trial install.
ALTER TABLE employees ADD COLUMN is_sample INTEGER NOT NULL DEFAULT 0;

-- Heal installs that already loaded sample data before this fix: every
-- bundled sample row uses the fictional @acmecorp.com domain.
UPDATE employees SET is_sample = 1 WHERE email LIKE '%@acmecorp.com';
