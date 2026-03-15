# People Partner — Known Issues & Parking Lot

> **Purpose:** Track issues, blockers, and deferred decisions.
> **Related Docs:** [ROADMAP.md](./ROADMAP.md) | [PROGRESS.md](./PROGRESS.md)
> **Full Decision Log:** [reference/DECISIONS-LOG.md](./reference/DECISIONS-LOG.md)

---

## How to Use This Document

**Add issues here when:**
- You encounter a bug that isn't blocking current work
- You discover something that needs investigation later
- A decision needs to be made but can wait
- You find edge cases that need handling eventually

**Format:**
```markdown
### [PHASE-X] Brief description
**Status:** Open | In Progress | Resolved | Deferred
**Severity:** Blocker | High | Medium | Low
**Discovered:** YYYY-MM-DD
**Description:** What happened / what's the issue
**Workaround:** (if any)
**Resolution:** (when resolved)
```

---

## Locked Architectural Decisions (V1)

These decisions were made during planning and should NOT be revisited during implementation:

| Area | Decision | Rationale |
|------|----------|-----------|
| DB Security | OS sandbox only | Trust macOS security, simpler stack |
| Context | Auto-include relevant employees | Smart retrieval, no confirmation friction |
| PII Action | Auto-redact and notify | No blocking modals, brief notification |
| PII Scope | Financial only (SSN, CC, bank) | Narrow scope, fewer false positives |
| Platform | macOS only | Focus on polish, native Keychain |
| Pricing | $99 one-time | Simple, honest, no subscriptions |
| Offline | Read-only mode | Browse history + employees when offline |
| Memory | Cross-conversation | Compounding value over time |
| Company Profile | Required: name + state | Minimal friction, ensures context |
| License | One-time online validation | Works offline forever after |
| Telemetry | Opt-in anonymous | Onboarding choice |
| Disclaimers | Onboarding + feature-specific consent | One-time onboarding + V2 feature first-use modals |
| Employee Updates | CSV re-import + individual edit | Both bulk and quick-fix supported |
| Work Locations | Single primary per employee | Defer multi-state to V2 |
| Audit Log | Standard (redacted content) | Balance compliance + privacy |
| Crash Reports | Opt-in anonymous telemetry | Respect user choice |
| Doc Ingestion | Not in V1 | Focus on employee context |
| Multi-Company | Not in V1 | Single company per install |

*For full rationale, see [reference/DECISIONS-LOG.md](./reference/DECISIONS-LOG.md)*

---

## Open Issues

### [REBRAND] Rename "HR Command Center" → "People Partner"
**Status:** Resolved
**Severity:** Medium
**Discovered:** 2026-03-01
**Resolved:** 2026-03-03
**Description:** Rebranding to "People Partner" (domain: peoplepartner.io secured). 180 occurrences across 116 files. Includes migration-sensitive areas: Keychain service name (7 refs in keyring.rs — existing users lose stored keys without migration), Tauri bundle ID/app name (tauri.conf.json), HRC- license prefix (tied to Stripe), proxy CORS/domain refs.
**Resolution:** Full rebrand completed in coordinated multi-agent session. Updated: tauri.conf.json (bundle ID, app name), all Rust module headers, all frontend UI strings, documentation (CLAUDE.md, README.md, ROADMAP.md, KNOWN_ISSUES.md, PROJECT_STATE.md, SESSION_PROTOCOL.md, features.json), release workflow, backup filenames, test data scripts. Keychain service name updated with migration logic for existing users.

### [PHASE-5.2] Trial mode never exits when API key is present
**Status:** Resolved  
**Severity:** High  
**Discovered:** 2026-02-07  
**Resolved:** 2026-02-07  
**Description:** Trial mode had no supported path to exit because `license_key` was required but never settable from app UI.  
**Workaround:** N/A (fixed)  
**Resolution:** Added license key commands + settings UI, switched gating to license-based trial mode, and enforced paid mode as "license + BYOK."

### [PHASE-5.2] Trial limit errors surface as generic failures and counters can desync
**Status:** Resolved  
**Severity:** Medium  
**Discovered:** 2026-02-07  
**Resolved:** 2026-02-07  
**Description:** Trial limit responses were mapped to generic chat failures, and local counters drifted from proxy counters.  
**Workaround:** N/A (fixed)  
**Resolution:** Added trial-specific frontend error mapping, proxy response usage headers (`X-Trial-Used`, `X-Trial-Limit`), backend counter sync from proxy metadata, and always-refresh behavior after chat attempts.

### [PHASE-5.2] Trial import limit over-counts updates
**Status:** Resolved  
**Severity:** Medium  
**Discovered:** 2026-02-07  
**Resolved:** 2026-02-07  
**Description:** Trial import validation treated all rows as net-new employees, blocking update-only imports.  
**Workaround:** N/A (fixed)  
**Resolution:** Trial guard now computes net-new unique emails only before enforcing the 10-employee cap.

### [PHASE-5.2] Trial employee limit UI does not refresh after CRUD/import
**Status:** Resolved  
**Severity:** Low  
**Discovered:** 2026-02-07  
**Resolved:** 2026-02-07  
**Description:** Employee usage indicators could stay stale after imports and employee-count changes.  
**Workaround:** N/A (fixed)  
**Resolution:** Employee context now refreshes trial status when total employee count changes, and chat submit now refreshes trial status in a `finally` path.

### [PHASE-5.1] Auto-updater check is unused
**Status:** Resolved  
**Severity:** Medium  
**Discovered:** 2026-02-07  
**Resolved:** 2026-02-07  
**Description:** Updater hook existed but was never mounted in app UI.  
**Workaround:** N/A (fixed)  
**Resolution:** `useUpdateCheck` is now mounted in the app shell with an in-header "Update Available" action.

### [PHASE-5.2] Trial proxy is open to abuse
**Status:** In Progress  
**Severity:** Medium  
**Discovered:** 2026-02-07  
**Description:** Original proxy accepted any origin and any UUIDv4 `X-Device-ID`, enabling scripted quota bypass and cost abuse.  
**Workaround:** Configure `TRIAL_SIGNING_SECRET` in Worker secrets and match it in desktop app config to enforce signed requests.  
**Resolution:** Mitigated with origin allowlist, coarse per-IP throttling, trial usage headers, optional HMAC signature verification, and replay protection. Final hardening depends on production secret configuration.

### [PHASE-5.3] License key validation is local-only
**Status:** Open  
**Severity:** Medium  
**Discovered:** 2026-02-07  
**Description:** License entry currently performs only local format validation before storing in `settings`. There is no server-side license verification endpoint yet, so authenticity/revocation checks are not enforced.  
**Workaround:** Treat this as beta gating only; use trusted distribution and manual support verification for licenses.  
**Resolution:** Pending Phase 5.3.1/5.3.2 implementation (license validation API + online verification flow).

### [PHASE-5.1] Distribution config placeholders block release
**Status:** Open  
**Severity:** Medium  
**Discovered:** 2026-02-07  
**Description:** `tauri.conf.json` updater `pubkey` and GitHub endpoints are placeholders; `proxy/wrangler.toml` KV namespace IDs are stubbed; upgrade URLs still point to placeholders. Builds will sign but updater/proxy/upgrade flows will fail until real values are filled.  
**Workaround:** Populate real pubkey, repo URLs, and KV IDs before any release or trial deployment.  
**Resolution:** Pending config updates.

### [PHASE-2.1] file_parser::tests::test_normalize_header test failure
**Status:** Resolved
**Severity:** Low
**Discovered:** 2025-12-17
**Resolved:** 2026-02-01
**Description:** The `test_normalize_header` test in file_parser.rs was failing. Test expected header normalization to produce "email" but received different value.
**Workaround:** N/A (fixed)
**Resolution:** Fixed in 2026-02-01 session — test now passes. Additional improvements may come with **Data Quality Center** (Feature #14).

### [DOCS] Documentation sync issues — BATCH RESOLVED
**Status:** Resolved
**Severity:** Medium
**Discovered:** 2026-02-04
**Resolved:** 2026-02-04
**Description:** Multiple documentation files had drifted out of sync:
- Roadmap linear checklist showed Phases 0–3 and V2.1/V2.2 as unchecked
- V2 "Promoted to Roadmap" table showed all features as "Not started"
- README showed V2.1.1 as current (was actually V2.4.2)
- features.json had pause-0a as "not-started"
- HR-Command-Center-Roadmap.md was out of sync with implementation status
- file_parser test status was inconsistent
- Disclaimers decision conflicted with V2 consent modals

**Resolution:** All issues addressed in 2026-02-04 documentation sync session:
- Updated README.md status to V2.4.2
- Checked off completed tasks in docs/ROADMAP.md linear checklist
- Updated V2 feature status table in this file
- Fixed features.json pause-0a status
- Added historical reference note to HR-Command-Center-Roadmap.md
- Updated disclaimers decision to reflect V2 evolution
- Marked file_parser test as resolved

### [SECURITY] Full database encryption-at-rest (SQLCipher) deferred
**Status:** Deferred
**Severity:** Medium
**Discovered:** 2026-02-06
**Description:** Audit review found the local employee database is not cryptographically encrypted at rest by default. Current posture is improved with strict file permissions (`0600`) and Keychain-backed API key storage, but the SQLite content remains plaintext if host-level filesystem access is obtained.
**Workaround:** Keep current mitigations (OS access controls + restrictive DB/WAL/SHM permissions) and treat host security as required.
**Resolution:** Deferred intentionally to a dedicated post-launch migration track so release-critical functionality is not destabilized. Revisit with a planned SQLCipher migration and compatibility testing (open/create/migrate/backup/restore).

---

## Code Review Audit (2026-03-13)

> **Source:** Comprehensive 4-agent code review (Rust quality, React/TS quality, Security, Silent failures)
> **Status:** Pending independent verification
> **Scope:** Desktop app (HRCommand) + Website (hr-command-center)

### Desktop App — Critical

### [AUDIT-C1] UTF-8 panics in PII scanner, audit log, and conversation title
**Status:** Open
**Severity:** Critical
**Discovered:** 2026-03-13
**Description:** Byte-index string slicing on lowercased text will panic on non-ASCII input (e.g., accented names like "Jose", "Muller"). In `pii.rs:365,393`, regex match offsets from original text are used to slice `text_lower`, which can have different byte lengths for non-ASCII chars. Same class of bug in `audit.rs:386` (`truncate_preview`) and `conversations.rs:364` (title truncation at byte 60).
**Workaround:** None — panics crash the operation.
**Fix:** Use `char_indices()` for all string slicing to find safe UTF-8 boundaries.

### [AUDIT-C2] Database init failure silently ignored — app launches broken
**Status:** Open
**Severity:** Critical
**Discovered:** 2026-03-13
**Description:** `lib.rs:2046-2050` — if `db::init_db()` fails, the app continues without a database pool. Every Tauri command then panics. Additionally, `db.rs:28,31` uses `.expect()` which will panic with no user-facing message if app data directory can't be created (permissions, full disk, sandbox issues).
**Workaround:** None — app appears to launch but nothing works.
**Fix:** Show error dialog and exit if DB init fails. Convert `get_db_path` to return `Result` instead of panicking.

### [AUDIT-C3] PII scanning fails open — unredacted financial data sent to AI
**Status:** Open
**Severity:** Critical
**Discovered:** 2026-03-13
**Description:** `ConversationContext.tsx:313-316` — if the PII scanner throws an error (backend crash, IPC failure), the catch block logs to console and sends the original unredacted message (potentially containing SSNs, credit cards, bank accounts) to Claude/OpenAI/Gemini. Comment says "fail open for usability."
**Workaround:** None — users are unaware their PII was not redacted.
**Fix:** Change to fail-closed: block the message and show an error to the user when PII scanning fails.

### [AUDIT-C4] Chat stream events silently dropped — UI hangs forever
**Status:** Open (Verification: OVERSTATED)
**Severity:** Medium (downgraded from Critical)
**Discovered:** 2026-03-13
**Description:** `chat.rs:269,284` — both text delta emission and the `done` signal use `let _ = app.emit(...)`. If the event channel breaks, text deltas vanish silently and the frontend remains in permanent loading state with no timeout or error indication.
**Verification note:** `app.emit()` in Tauri is in-process event dispatch — failure only occurs if the webview is destroyed, in which case there's no user to notify. Stream content is still accumulated in `full_response` for audit purposes. This is idiomatic Tauri. Consider adding a client-side stream timeout as a UX improvement, not a critical fix.

### [AUDIT-C5] Apple Developer credentials exposed on disk
**Status:** Open
**Severity:** Critical
**Discovered:** 2026-03-13
**Description:** The `.env` file contains live `APPLE_ID`, `APPLE_PASSWORD` (app-specific password), and `APPLE_TEAM_ID` in plaintext. While `.gitignore`'d and never committed, any process with filesystem access can harvest them.
**Workaround:** N/A
**Fix:** Rotate the app-specific password immediately. Move credentials to macOS Keychain or CI secrets.

### Desktop App — High

### [AUDIT-H1] Backup restore has no transaction — partial failure causes data loss
**Status:** Open
**Severity:** High
**Discovered:** 2026-03-13
**Description:** `backup.rs:961-996` — `import_backup` clears ALL tables then restores one by one without a transaction. If restore fails at table 5/9, the database is half-empty with no rollback. Additionally, FTS indexes are not rebuilt after restore — conversation search is permanently broken.
**Workaround:** Keep a manual backup before restoring.
**Fix:** Wrap clear + restore in `pool.begin()` / `tx.commit()`. Run FTS rebuild after restore.

### [AUDIT-H2] License validation fails open on network error
**Status:** Open
**Severity:** High
**Discovered:** 2026-03-13
**Description:** `lib.rs:272-283` — when the license validation server is unreachable, any key matching the `PP-XXXX-...` format is silently accepted. No logging, no user notification. Block the network = free app.
**Workaround:** None currently. Related to existing [PHASE-5.3] issue.
**Fix:** Implement cryptographic license key validation (ed25519 signatures) that works offline. Or cache validation result with periodic re-check.

### [AUDIT-H3] Backup exports device_id, license_key, and signing secrets
**Status:** Open (Verification: OVERSTATED)
**Severity:** Medium (downgraded from High)
**Discovered:** 2026-03-13
**Description:** `backup.rs:443-458` — the backup function dumps the entire `settings` table, including `device_id`, `license_key`, `proxy_signing_secret`, and `trial_messages_used`. Restoring to a different machine imports the original device_id and license_key.
**Verification note:** Backup files are AES-256-GCM encrypted with a user-provided password, so exposure requires the password. API keys are stored in macOS Keychain (NOT in settings table) and are NOT included in backups. The real concern is restore-correctness (two machines with same device_id), not security exposure.
**Fix:** Filter `device_id`, `trial_messages_used`, and `proxy_signing_secret` from backup export to prevent restore-correctness issues.

### [AUDIT-H4] Trial status failure defaults to "not trial" — bypasses all limits
**Status:** Open
**Severity:** High
**Discovered:** 2026-03-13
**Description:** `TrialContext.tsx:107-110` — if the trial status fetch fails, `isTrialMode` defaults to `false`. Free users silently get unlimited access.
**Workaround:** None.
**Fix:** Default to trial mode on error (fail-closed), or show error state.

### [AUDIT-H5] Stale closure on conversationId during streaming
**Status:** Open
**Severity:** High
**Discovered:** 2026-03-13
**Description:** `ConversationContext.tsx:304-491` — `sendMessage` captures `conversationId` in a closure. If the user switches conversations mid-stream, audit entries and conversation saves write to the wrong conversation.
**Workaround:** Don't switch conversations while streaming.
**Fix:** Capture `conversationId` at function start, not from closure. Use a ref for stable access.

### [AUDIT-H6] Modal scroll lock leaks with nested modals
**Status:** Open
**Severity:** High
**Discovered:** 2026-03-13
**Description:** `Modal.tsx:80-107` — closing an inner modal (e.g., SignalsDisclaimer on top of Settings) clears `overflow: hidden` from body while outer modal is still open, allowing background scrolling.
**Workaround:** None.
**Fix:** Use a modal counter or stack approach — only clear overflow when no modals remain.

### [AUDIT-H7] PII only scanned on user input, not on context sent to AI
**Status:** Open
**Severity:** High
**Discovered:** 2026-03-13
**Description:** `context.rs:2538` — the system prompt includes unredacted employee names, DOBs, gender, ethnicity, performance ratings, termination reasons. The PII scanner only runs on the user's typed message. The bulk of PII sent to AI APIs bypasses scanning. Note: this may be an intentional design trade-off (the AI needs employee context to function).
**Workaround:** Documented architectural decision — PII scope is "financial only" per locked decisions.
**Fix:** Document this clearly for compliance. Consider excluding DOB, ethnicity, gender from AI context unless needed. Add data processing disclosure.

### [AUDIT-H8] Trial message counter TOCTOU race condition
**Status:** Open (Verification: OVERSTATED)
**Severity:** Low (downgraded from High)
**Discovered:** 2026-03-13
**Description:** `trial.rs:127-132` — read-modify-write without atomic update. Concurrent requests can under-count, allowing extra trial messages.
**Verification note:** This is a single-user desktop app (macOS only, single window). `sendMessage` sets `isLoading(true)` which disables the input, preventing concurrent calls through normal use. The proxy is the authoritative counter. No practical attack vector exists.
**Workaround:** Proxy is the authoritative counter; local is a fallback.
**Fix:** Low priority. Could use atomic SQL update if desired.

### [AUDIT-H9] Import dedup uses wrong email casing for DB lookup
**Status:** Open
**Severity:** High
**Discovered:** 2026-03-13
**Description:** `lib.rs:661-688` — normalizes emails to lowercase for uniqueness check but queries DB with original case. SQLite's `=` is case-sensitive by default, so `Alice@corp.com` != `alice@corp.com`, allowing trial employee limit bypass.
**Workaround:** None.
**Fix:** Use `normalized_email` in the database lookup.

### Desktop App — Medium

### [AUDIT-M1] Memory search uses only first keyword
**Status:** Open
**Severity:** Medium
**Discovered:** 2026-03-13
**Description:** `memory.rs:233` — `search_summaries_only` uses only `keywords[0]`, discarding all other keywords. "Sarah performance review" only searches for "sarah".
**Fix:** Build a LIKE OR-chain for all keywords, or use FTS5.

### [AUDIT-M2] Migration parser swallows "table already exists" too broadly
**Status:** Open
**Severity:** Medium
**Discovered:** 2026-03-13
**Description:** `db.rs:109-173` — the homegrown SQL migration parser catches all "already exists" errors, not just `ALTER TABLE ADD COLUMN`. Could mask genuine schema bugs on fresh installs.
**Fix:** Only suppress "duplicate column" for ALTER TABLE migrations.

### [AUDIT-M3] Auto-save failure silently loses conversation data
**Status:** Open
**Severity:** Medium
**Discovered:** 2026-03-13
**Description:** `ConversationContext.tsx:252-255` — auto-save failure logs to console only. If the user closes the app, the entire conversation is lost.
**Fix:** Show a non-intrusive notification on save failure.

### [AUDIT-M4] Keyboard Enter bypasses offline check
**Status:** Open
**Severity:** Medium
**Discovered:** 2026-03-13
**Description:** `ChatInput.tsx:74` — the submit button disables on `isOffline`, but the Enter keypress handler doesn't check it. Users can send messages via keyboard while offline.
**Fix:** Add `isOffline` check to `handleSubmit`.

### [AUDIT-M5] Settings load failures silently default values
**Status:** Open
**Severity:** Medium
**Discovered:** 2026-03-13
**Description:** `SettingsPanel.tsx:62-89` — seven `.catch()` handlers silently set defaults (e.g., `setSignalsEnabled(false)`) without logging. If the database is corrupted, a feature the user enabled could be silently disabled.
**Fix:** Log errors and show a warning banner in Settings.

### [AUDIT-M6] 5x Regex::new().unwrap() in production verify_response path
**Status:** Open
**Severity:** Medium
**Discovered:** 2026-03-13
**Description:** `context.rs:2953-3036` — five regex compilations use `.unwrap()` in the `verify_response()` function. If a regex library bug causes failure, the entire response verification crashes.
**Fix:** Use `lazy_static!` or `OnceLock` to compile regexes once at startup.

### [AUDIT-M7] Background highlight extraction failures invisible
**Status:** Open
**Severity:** Medium
**Discovered:** 2026-03-13
**Description:** `bulk_import.rs:278-289` and `performance_reviews.rs:116-125` — fire-and-forget `tokio::spawn` tasks for highlight extraction. Failures log to stderr only. No retry mechanism or user notification.
**Fix:** Track failed extractions and surface incomplete highlights to the user.

### [AUDIT-M8] FTS index corruption after backup restore
**Status:** Open
**Severity:** Medium
**Discovered:** 2026-03-13
**Description:** `backup.rs:606-637` — FTS tables are cleared before main tables, and after restore, no triggers re-fire to populate the FTS index from restored data. Conversation search is broken post-restore.
**Fix:** Run `INSERT INTO conversations_fts(conversations_fts) VALUES('rebuild')` after restore.

### Website (hr-command-center) — High

### [AUDIT-W1] License key exposed in Stripe checkout session metadata
**Status:** Open
**Severity:** High
**Discovered:** 2026-03-13
**Description:** `api/checkout/route.ts:39` — the license key is stored in Stripe session metadata. Anyone with Stripe dashboard access can see every customer's license key. The `license_id` alone is sufficient for the webhook to activate the license.
**Workaround:** N/A.
**Fix:** Remove `license_key` from metadata. Webhook resolves by `checkout_session_id` or `license_id`.

### [AUDIT-W2] No rate limiting on API routes
**Status:** Open
**Severity:** High
**Discovered:** 2026-03-13
**Description:** `/api/checkout`, `/api/validate-license`, and `/api/webhook` have no rate limiting. An attacker could spam checkout to create thousands of pending licenses, or brute-force license validation.
**Fix:** Add Vercel edge rate limiting or IP-based limiter middleware.

### [AUDIT-W3] License key displayed without authentication
**Status:** Open
**Severity:** High
**Discovered:** 2026-03-13
**Description:** `purchase/success/page.tsx:27` — the success page takes `?session_id=...` from the URL and displays the full license key with no auth. If the URL is shared, bookmarked, or in browser history, the key is exposed.
**Fix:** Only show last 4 characters, require email verification for full key, or add a one-time-use token.

### Website — Medium

### [AUDIT-W4] Tracking pixels loaded without cookie consent
**Status:** Open
**Severity:** Medium
**Discovered:** 2026-03-13
**Description:** `components/TrackingPixels.tsx` — LinkedIn, Reddit, Meta, and Google Analytics pixels load unconditionally on every page. GDPR (EU visitors) and CCPA require consent before tracking.
**Fix:** Add cookie consent banner; conditionally load pixels.

### [AUDIT-W5] No security headers configured
**Status:** Open
**Severity:** Medium
**Discovered:** 2026-03-13
**Description:** No `middleware.ts` or `next.config.ts` headers for CSP, HSTS, X-Frame-Options, X-Content-Type-Options, or Referrer-Policy. Vercel adds some defaults but explicit headers would harden the site.
**Fix:** Add security headers via Next.js middleware or config.

---

## Resolved Issues

### [PHASE-2.6] Selected employee not prioritized in context
**Status:** Resolved
**Severity:** Medium
**Discovered:** 2025-12-18
**Resolved:** 2025-12-18
**Description:** When a user selects an employee from the People tab and asks a question about them (e.g., "How does Amanda compare to the team?"), the context builder didn't know about the selection. It extracted "Amanda" from the query and returned all employees with that name instead of prioritizing the selected one.
**Resolution:** Task 2.7.0 implemented — `selected_employee_id` is now passed from UI → ConversationContext → getSystemPrompt → build_chat_context → find_relevant_employees. The selected employee is always fetched first and prepended to the context results.

---

## V2 Features

> **Note:** High and medium impact V2 features have been promoted to **Phase V2** in [ROADMAP.md](./ROADMAP.md).
> This section tracks remaining low-priority features and future ideas.

---

### Promoted to Roadmap (Phase V2)

The following features are now tracked in `docs/ROADMAP.md` under **Phase V2: Intelligence & Visualization**:

| Feature | Roadmap Section | Status |
|---------|-----------------|--------|
| API Key Setup Guide (Enhanced) | V2.1.1 | ✅ Complete |
| Command Palette + Shortcuts | V2.1.2 | ✅ Complete |
| Persona Switcher | V2.1.3 | ✅ Complete |
| Answer Verification Mode | V2.1.4 | ✅ Complete |
| Structured Data Extraction (Review Highlights) | V2.2.1 | ✅ Complete |
| Query-Adaptive Retrieval v2 | V2.2.2 | ✅ Complete |
| Interactive Analytics Panel + Insight Canvas | V2.3.2 | ✅ Complete |
| Attrition & Sentiment Signals | V2.4.1 | ✅ Complete |
| DEI & Fairness Lens | V2.4.2 | ✅ Complete |
| Org Chart View + Heatmap Overlay | V2.3.1 | Deferred (parking lot) |
| Data Quality Center | V2.5.1 | Not started |

---

### V2 Parking Lot (Lower Priority)

Features deferred until demand is established. Track user requests.

#### Data & Platform

| Feature | Impact | Complexity | Notes |
|---------|--------|------------|-------|
| Document/PDF Ingestion | ⚡ Medium | High | Phase 1: FTS only, Phase 2: embeddings |
| Compensation Data | ⚡ Medium | High | Requires sensitive mode + encryption |
| Multi-State Locations | 💡 Low | Medium | Location history with effective dates |
| Multi-Company/Workspaces | 💡 Low | Medium | Separate SQLite DBs per company |
| Windows/Linux Support | 💡 Low | High | Keyring abstraction, packaging matrix |

#### Security Enhancements

| Feature | Impact | Complexity | Notes |
|---------|--------|------------|-------|
| Expanded PII Detection | 💡 Low | Medium | Medical, immigration, DL numbers |
| Safe Share Packs | ⚡ Medium | Medium | Redacted exports with watermarking |
| Tamper-Evident Audit | 💡 Low | Medium | Hash-chained audit log entries |
| Optional Local DB Encryption | 💡 Low | Medium | SQLCipher for comp-enabled installs |

#### Import/Export Enhancements

| Feature | Impact | Complexity | Notes |
|---------|--------|------------|-------|
| HRIS Templates | ⚡ Medium | Medium | BambooHR, Gusto, Rippling mappings |
| Bulk Actions & Backfills | ⚡ Medium | Medium | Post-import fix workflows |

#### Visualization

| Feature | Impact | Complexity | Notes |
|---------|--------|------------|-------|
| Org Chart View + Heatmap | 🔥 High | Medium | Deferred from V2.3.1 — tree visualization, expand/collapse, attention scores |

#### UX & Accessibility

| Feature | Impact | Complexity | Notes |
|---------|--------|------------|-------|
| Keyboard Navigation Complete | 💡 Low | Low | WCAG AA, screen reader support |
| Branding & Theming | 💡 Low | Low | Company logo, accent colors |

**Legend:** 🔥 High impact | ⚡ Medium | 💡 Low priority

---

### Parking Lot Feature Details

<details>
<summary><strong>Document/PDF Ingestion</strong> (⚡ Medium impact, High complexity)</summary>

V1 supports CSV, Excel, TSV. This adds PDF/DOCX for policy documents.

**Use Cases:**
- Ask questions about company policies/handbooks
- Reference employee handbook during conversations
- Search across policy documents

**Phased Approach:**
1. Phase 1: Text-only DOCX/PDF → FTS indexing
2. Phase 2: Section-aware chunking, embeddings for semantic search

**Enhancements:**
- Section-aware chunking respecting document structure
- Citations to page/section ("See Employee Handbook, Section 4.2")
- Policy-tag filters for targeted context

</details>

<details>
<summary><strong>Compensation Data</strong> (⚡ Medium impact, High complexity)</summary>

Add salary, bonus, and equity data with enhanced security.

**Would Add:**
- Salary history and current compensation
- Bonus targets and payouts
- Equity grants and vesting schedules
- Pay equity analysis capabilities

**Security Requirements:**
- "Sensitive mode" requiring explicit unlock
- Guardrailed pay equity templates
- Banding/bucketing (ranges, not exact figures)
- AES-at-rest for comp tables only
- Audit trail for all comp data access

</details>

<details>
<summary><strong>Multi-State Locations</strong> (💡 Low impact, Medium complexity)</summary>

Remote workers may work from multiple states.

**Implementation:**
- Location history table with effective dates
- Compliance calendar using latest state
- UI timeline on Employee detail
- Import support for location history

</details>

<details>
<summary><strong>Multi-Company/Workspaces</strong> (💡 Low impact, Medium complexity)</summary>

HR consultants with multiple clients.

**Implementation:**
- Separate SQLite databases per company
- Explicit workspace switcher in UI
- Separate settings/export per workspace
- No cross-workspace search (data isolation)

</details>

<details>
<summary><strong>Windows/Linux Support</strong> (💡 Low impact, High complexity)</summary>

macOS only for V1. Cross-platform adds significant complexity.

**Challenges:**
- Keyring abstraction (Keychain → Credential Manager → Secret Service)
- Path differences for app data
- Platform-specific auto-update mechanisms
- Packaging matrix (.dmg, .msi/.exe, .deb/.AppImage)

**Recommendation:** Defer until demand is real. Track requests.

</details>

<details>
<summary><strong>Expanded PII Detection</strong> (💡 Low impact, Medium complexity)</summary>

V1 detects financial PII only (SSN, CC, bank).

**Could Add:**
- Medical record numbers
- Immigration document numbers (visa, I-9)
- Driver's license numbers

**Enhancements:**
- Confidence scoring for each detection
- Preview mask before sending
- Domain-specific patterns (opt-in)

</details>

<details>
<summary><strong>Safe Share Packs</strong> (⚡ Medium impact, Medium complexity)</summary>

One-click redacted exports for sharing.

**Use Cases:**
- Share employee brief with manager (no PII)
- Export team report for leadership
- Prepare materials for legal/compliance

**Features:**
- Redacted employee briefs
- Team reports with aggregate data only
- Watermarking with user/date
- Export logs

</details>

<details>
<summary><strong>HRIS Templates</strong> (⚡ Medium impact, Medium complexity)</summary>

Pre-built mappings for common HRIS exports.

**Supported HRIS:**
- BambooHR, Gusto, Rippling
- ADP (basic), Workday (basic)

**Features:**
- Guided import templates
- Header auto-detection for known HRIS
- Validation rules per HRIS

*Note: Basic HRIS header mappings included in V2.5.1 Data Quality Center.*

</details>

<details>
<summary><strong>Bulk Actions & Backfills</strong> (⚡ Medium impact, Medium complexity)</summary>

"Fix common issues" quick actions post-import.

**Quick Actions:**
- Add missing managers by inference
- Standardize titles ("Sr. Engineer" → "Senior Engineer")
- Normalize locations ("CA" → "California")
- Fix date format inconsistencies

</details>

<details>
<summary><strong>Org Chart View + Heatmap Overlay</strong> (🔥 High impact, Medium complexity)</summary>

Interactive hierarchy visualization with signal overlays. Deferred from V2.3.1 to focus on Analytics Panel for launch.

**Core Features:**
- Tree visualization from manager_id relationships
- Expand/collapse for direct reports
- Click-to-select (syncs with People panel)
- Search/filter within tree
- Zoom/pan for large orgs
- Department color coding

**Heatmap Overlay:**
- "Attention score" composite calculation
- Team/department coloring based on risk signals
- Click-to-drill into anonymized themes
- Toggle off by default

**Attention Score Factors:**
- High turnover (YTD terminations / headcount)
- Low eNPS (team average below org average)
- Negative review themes (from structured extraction)

**Why Deferred:** Adds visual complexity without core query/analytics value. Can be added post-launch based on user demand.

</details>

---

## Edge Cases to Handle

| Case | Phase | Priority | Notes |
|------|-------|----------|-------|
| CSV with 1000+ employees | 2 | Medium | May need pagination/lazy loading |
| Very long conversation history | 2 | Medium | Memory retrieval may slow down |
| Offline during onboarding | 4 | Low | Can't validate API key offline |
| License server unreachable | 5 | Medium | Need grace period strategy |
| Long performance reviews in context | 2.7 | Medium | See note below |

### Performance Review Length vs Context Budget

**Discovered:** 2025-12-19
**Status:** Planned → See ROADMAP.md V2.2.1

Current test data has 1-2 sentence performance reviews. Real-world reviews could be 500-2000+ words each.

**Solution:** Implement **Structured Data Extraction** (ROADMAP.md V2.2.1)
- Extract structured entities (strengths, opportunities, quotes, themes)
- Precompute per-employee profile summaries
- Use highlights in context, full reviews on-demand

**Additional Mitigations:**
- Token budgets by query type (V2.2.2)
- Dynamic excerpting for relevant sentences

---

## UI Polish (Future)

### Conversation Sidebar Title Truncation

**Discovered:** 2025-12-19
**Status:** Low priority polish item

The conversation card title text in the sidebar is too large/not adaptive. Titles almost never fit in the available space, resulting in ellipsis truncation ("Betty's Performance ...").

**Potential Fixes:**
- Reduce title font size (currently appears to be text-lg/font-semibold)
- Use smaller font with more lines (2-line clamp instead of 1)
- Show full title on hover (tooltip)
- Auto-generate shorter titles (currently using Claude, could request brevity)
- Responsive font size based on title length

**Component:** `src/components/conversations/ConversationCard.tsx`
**Revisit When:** Phase 4 Polish

---

## Technical Debt

*(Track technical shortcuts that need revisiting)*

| Item | Phase Created | Priority | Notes |
|------|---------------|----------|-------|
| *(none yet)* | | | |

---

*Last updated: March 2026*
