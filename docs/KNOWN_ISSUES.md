# HR Command Center — Known Issues & Parking Lot

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
**Status:** Deferred
**Severity:** Medium
**Discovered:** 2026-03-01
**Description:** Rebranding to "People Partner" (domain: peoplepartner.io secured). 180 occurrences across 116 files. Includes migration-sensitive areas: Keychain service name (7 refs in keyring.rs — existing users lose stored keys without migration), Tauri bundle ID/app name (tauri.conf.json), HRC- license prefix (tied to Stripe), proxy CORS/domain refs.
**Workaround:** Continue using "HR Command Center" internally until website establishes final branding.
**Resolution:** Dedicated rebrand session after peoplepartner.io website is live. Website work first to inform: final product name casing, logo, domain config, Stripe product rename. Then do app rebrand as single coordinated pass including Keychain migration logic.

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

*Last updated: February 2026*
