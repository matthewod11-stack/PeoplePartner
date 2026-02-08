# HR Command Center — Project State

> Cross-surface context document. Shared across Claude Chat, Claude Code, and Cowork sessions.
> **Last regenerated:** 2026-02-08 | **Generated from:** codebase scan

---

## Project Overview

HR Command Center is a macOS desktop app that gives HR professionals (solo practitioners, accidental HR people, founders without HR) a private, AI-powered assistant that understands their specific company context. It runs as a Tauri app with a React frontend and Rust backend, stores all employee data locally in SQLite, connects to Claude API for AI chat, and auto-redacts PII before anything leaves the machine. The app is feature-complete through Phase V2 (intelligence layer, analytics, data quality) and Phase 5.2 (trial infrastructure). It is not yet publicly released — license validation, payment integration, and beta distribution remain.

---

## Current Stack

| Layer | Technology | Version |
|-------|-----------|---------|
| Framework | Tauri | 2.x |
| Frontend | React + TypeScript | React 18.3, TS 5.6 |
| Build | Vite | 6.0 |
| Styling | Tailwind CSS | 3.4 |
| Backend | Rust | 1.92 (2021 edition) |
| Database | SQLite via SQLx | 0.8 |
| AI | Anthropic Claude API | claude-3-5-sonnet (configurable) |
| Charts | Recharts | 3.6 |
| Markdown | react-markdown + rehype-sanitize + remark-gfm | 10.1 |
| Fuzzy search | Fuse.js | 7.1 |
| File parsing | csv 1.3, calamine 0.26 (Rust) | |
| Encryption | AES-256-GCM (aes-gcm), Argon2 | |
| Keychain | security-framework (macOS native) | 3.5 |
| Auto-update | tauri-plugin-updater + tauri-plugin-process | 2.x |
| Trial proxy | Cloudflare Workers + KV | (not yet deployed) |
| CI/CD | GitHub Actions (release.yml) | |
| Platform | macOS only (10.15+) | |

No test runner configured for frontend (TypeScript type-check only via `tsc --noEmit`). Rust has 317 passing tests.

---

## Architecture

### Directory Layout
```
src/                          # React frontend
  components/                 # 15 subdirectories: analytics, chat, company, conversations,
                              #   dev, employees, import, insights, layout, onboarding,
                              #   settings, shared, trial, ui, CommandPalette.tsx
  contexts/                   # ConversationContext, EmployeeContext, LayoutContext, TrialContext
  hooks/                      # 10 custom hooks (import pipeline, data quality, network, etc.)
  lib/                        # Types, Tauri command wrappers, utility modules

src-tauri/src/                # Rust backend (30 modules, ~20,600 LOC)
  lib.rs                      # Tauri command exports (all IPC entry points)
  db.rs                       # SQLite connection + 5 migrations
  chat.rs                     # Claude API client, streaming, dual-path (BYOK/proxy)
  context.rs                  # Context builder, query classification, Alex persona
  employees.rs                # Employee CRUD + parameterized filters
  data_quality.rs             # Validation, dedupe, column mapping, HRIS presets (28 tests)
  highlights.rs               # Structured review extraction pipeline
  analytics.rs + analytics_templates.rs  # NL-to-SQL chart generation
  insight_canvas.rs           # Persistent chart boards
  signals.rs                  # Attrition/sentiment heuristics
  dei.rs                      # DEI representation analysis + small-n suppression
  trial.rs                    # Trial mode detection, limits, message counting
  device_id.rs                # Stable UUID per install
  pii.rs                      # PII regex scanner + redaction
  audit.rs                    # Redacted audit logging
  memory.rs                   # Cross-conversation memory + summaries
  keyring.rs                  # macOS Keychain API key storage
  backup.rs                   # AES-256-GCM encrypted backup/restore

proxy/                        # Cloudflare Workers (trial API proxy)
  src/index.ts                # KV quota tracking, rate limiting, HMAC signing
```

### Data Flow
```
User Input → PII Scan → Context Builder (query classification + employee retrieval
  + org aggregates + review highlights + memory) → Claude API → Audit Log → Response
```

For analytics queries: Claude emits structured analytics request → Rust runs deterministic SQL → React renders chart from dataset. Claude never generates numbers directly.

### Database (9 tables + FTS)
employees, conversations, company, settings, audit_log, review_cycles, performance_ratings, performance_reviews, enps_responses. Plus: review_highlights, insight_boards, insight_items, insight_annotations (canvas system). 5 migrations total.

---

## Current State

### Fully Built & Working
- **Chat:** Streaming responses, conversation history, auto-titles, FTS search, cross-conversation memory
- **Context engine:** Query-adaptive retrieval (individual/team/org classification), dynamic token budgets, selected-employee prioritization, org-level aggregates
- **Employee management:** CRUD, CSV/XLSX/TSV import with merge-by-email, department/manager filters
- **Performance data:** Ratings (1.0-5.0), text reviews with FTS, eNPS scores, structured highlight extraction
- **Data quality center:** Column mapping UI, validation rules, dedupe detection, fix-and-retry workflow, HRIS preset detection (BambooHR, Gusto, Rippling)
- **Analytics panel:** NL-to-SQL templates (24 chart combinations), bar/pie/line charts via Recharts, filter captions
- **Insight canvas:** Pin charts to named boards, annotations, 1-page report export, chart drilldown to employee list
- **Intelligence layer:** Attrition/sentiment signals (team-level, opt-in), DEI fairness lens (representation, rating parity, promotion delta, small-n suppression)
- **Personas:** 5 switchable HR personas (Alex, Jordan, Sam, Morgan, Taylor)
- **Answer verification:** Parallel SQL ground-truth checks on numeric claims
- **Command palette:** Cmd+K with fuzzy search across actions/conversations/employees
- **Protection:** PII auto-redaction (SSN, CC, bank), audit logging, CSP, markdown sanitization
- **Onboarding:** 7-step wizard (welcome, API key guide, company, employee import, disclaimer, telemetry, first prompt)
- **Settings:** API key management, company profile, persona selector, data path, telemetry toggle, backup/restore
- **Monday digest:** Anniversaries and new hires on weekly first launch
- **Offline mode:** Read-only browsing of employees and conversation history
- **Accessibility:** WCAG AA contrast, 40px touch targets, focus traps in modals, screen reader chart alternatives, reduced motion support
- **Distribution infra:** Code signing, notarization config, auto-updater plugin, GitHub Actions release workflow
- **Trial infra:** Cloudflare Workers proxy (50-msg quota per device), dual-path chat routing, trial banner, message counter, employee limit (10), upgrade prompts, license key input UI

### Stubbed / Partially Built
- **License validation:** Local format-only check exists; no server-side verification API (Phase 5.3)
- **Auto-updater:** Plugin wired + UI hook mounted, but `tauri.conf.json` has placeholder pubkey and GitHub endpoint
- **Proxy deployment:** Code complete but not deployed; `wrangler.toml` has placeholder KV namespace IDs
- **HMAC request signing:** Optional path implemented in proxy + app; needs `TRIAL_SIGNING_SECRET` configured in production

### Not Started
- Server-side license validation API (5.3)
- Stripe payment integration (5.4)
- Landing page / hrcommandcenter.com (5.5)
- Beta distribution + feedback collection (5.6)
- Org chart view (deferred to post-launch)
- Document/PDF ingestion (parking lot)
- SQLCipher encryption at rest (deferred)

---

## Recent Decisions

1. **Decision:** Trial mode gated on license presence, not API key absence — **Reason:** API key alone couldn't distinguish trial from paid; license provides a clean binary state
2. **Decision:** Proxy counter is authoritative, local counter syncs from `X-Trial-Used`/`X-Trial-Limit` headers — **Reason:** Prevents client-side counter drift and ensures consistent quota enforcement
3. **Decision:** Trial import cap enforces net-new unique emails, not raw row count — **Reason:** Update-only imports (re-importing existing employees) shouldn't count against the 10-employee cap
4. **Decision:** HMAC request signing on proxy is optional (off by default) — **Reason:** Enables local dev without secrets while allowing production hardening
5. **Decision:** Org Chart deferred to post-launch — **Reason:** Analytics panel delivers more core value; org chart adds visual complexity without powering queries
6. **Decision:** SQLCipher encryption at rest deferred — **Reason:** Current mitigations (0600 file permissions, Keychain for API keys) acceptable for launch; SQLCipher migration risks destabilizing release
7. **Decision:** `signingIdentity: null` in tauri.conf.json — **Reason:** Defers to environment variable at build time; avoids hardcoding developer identity
8. **Decision:** ConversationContext split into Data + Actions contexts — **Reason:** Streaming chat was causing full sidebar/search re-renders; split isolates update frequency
9. **Decision:** All intelligence features (attrition signals, DEI lens) require opt-in + disclaimers — **Reason:** Heuristic outputs must not be mistaken for predictions; ethical guardrails required
10. **Decision:** $99 one-time pricing, BYOK after purchase — **Reason:** Simple, honest pricing; trial proxy funds initial 50 messages, then user brings own Claude API key

---

## Known Issues & Debt

| Issue | Severity | Status |
|-------|----------|--------|
| `tauri.conf.json` updater pubkey + GitHub endpoint are placeholders | Medium | Open — blocks real auto-update |
| `proxy/wrangler.toml` KV namespace IDs are stubs | Medium | Open — blocks proxy deployment |
| Upgrade URLs in trial UI point to placeholder | Low | Open — needs real purchase page |
| License validation is local format-only (no server check) | Medium | Open — Phase 5.3 |
| Proxy abuse mitigation is partial (origin allowlist + IP throttle + optional HMAC) | Medium | In progress |
| No frontend test runner (Jest/Vitest) | Low | Technical debt |
| Conversation sidebar title truncation (titles overflow) | Low | UI polish |
| README project status table is stale (says V2.4 current, actually through 5.2) | Low | Needs update |
| `features.json` "Data Quality Center" status table still says "Not started" in KNOWN_ISSUES.md | Low | Doc drift |
| No E2E manual verification of Pause Points V2.5 or 5B yet | Medium | Pending |

---

## What's Next

**Immediate (next 1-3 sessions):**
1. Populate production placeholders: updater pubkey, GitHub repo URL, Cloudflare KV namespace IDs, upgrade purchase URL
2. Deploy Cloudflare Workers proxy and test end-to-end trial flow
3. Configure `TRIAL_SIGNING_SECRET` for production HMAC signing
4. Manual E2E verification of Pause Points V2.5 and 5B

**Short-term (next 3-6 sessions):**
5. Phase 5.3: Server-side license validation API
6. Phase 5.4: Stripe payment integration ($99 one-time)
7. Phase 5.5: Landing page (hrcommandcenter.com)

**Medium-term:**
8. Phase 5.6: Beta distribution (5-10 users, feedback collection)
9. Update README to reflect Phase 5.2 completion
10. Consider adding Vitest for frontend component tests

---

## Cross-Surface Notes

- **Data Quality Center status:** KNOWN_ISSUES.md "Promoted to Roadmap" table still shows V2.5.1 as "Not started" but it is fully implemented (backend + frontend + all 4 import types refactored). The backend has 28 tests and 9 Tauri commands.
- **README drift:** README says "Current: V2.4.2" but the project is actually through Phase 5.2 (trial infrastructure complete).
- **Trial architecture divergence:** Original Claude Chat planning discussed a simpler "API key = paid" model. Implementation evolved to license-based gating (license + BYOK = paid, no license + no key = trial) after discovering the original model had no exit path from trial mode.
- **Proxy security:** Claude Chat discussions may reference a simple open proxy. The current implementation includes origin allowlist, per-IP throttling, optional HMAC signature verification with replay protection, and proxy-authoritative usage headers.
- **Phase numbering:** V2.5.1 (Data Quality Center) and Phase 5.1/5.2 (Distribution/Trial) were built in parallel across sessions. The roadmap numbering can be confusing — V2.x are feature phases, 5.x are launch infrastructure phases.
- **Test count:** 317 Rust tests passing (includes 28 data quality + 9 trial/device_id tests added in recent sessions). No frontend test framework — type-checking only.

---

*This file is the single source of truth for external Claude sessions. Update it at the end of any session with meaningful changes.*
