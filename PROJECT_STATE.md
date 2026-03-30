# People Partner — Project State

> Cross-surface context document. Shared across Claude Chat, Claude Code, and Cowork sessions.
> Last updated: 2026-03-24

---

## Elevator Pitch

Most small companies can't afford an HR department, and the founders doing HR on the side are one compliance mistake away from a lawsuit. People Partner is a desktop app that gives any company a private, AI-powered HR assistant — it understands your specific employees, policies, and context, answers questions in plain English, and never sends your sensitive employee data to the cloud. Everything stays on your machine, with automatic PII redaction before anything touches an AI model.

## Project Overview

People Partner (formerly HR Command Center, rebranded 2026-03-03) is a macOS desktop app that gives HR professionals (solo practitioners, accidental HR people, founders without HR) a private, AI-powered assistant that understands their specific company context. It runs as a Tauri app with a React frontend and Rust backend, stores all employee data locally in SQLite, and auto-redacts PII before anything leaves the machine.

The app supports three AI providers (Anthropic Claude, OpenAI, Google Gemini) through a provider trait abstraction, with a frontend model picker for each. V3.0 document ingestion ships local file indexing with FTS5 search, section-aware chunking, and PII redaction on indexed content.

All feature development is complete. The analytics/chart system was intentionally removed to focus the product on conversational HR. Remaining before public launch: Apple Developer enrollment approval, first signed release, and full E2E verification.

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
| AI | Multi-provider (Anthropic, OpenAI, Gemini) | Configurable per-model |
| Markdown | react-markdown + rehype-sanitize + remark-gfm | 10.1 |
| Fuzzy search | Fuse.js | 7.1 |
| File parsing | csv 1.3, calamine 0.26, pdf-extract 0.7, docx-rs 0.4 (Rust) | |
| File watching | notify 7.x (FSEvents backend) | |
| Encryption | AES-256-GCM (aes-gcm), Argon2 | |
| Keychain | security-framework (macOS native) | 3.5 |
| Auto-update | tauri-plugin-updater + tauri-plugin-process | 2.x |
| Trial proxy | Cloudflare Workers + KV | Deployed |
| CI/CD | GitHub Actions (release.yml) | |
| Platform | macOS only (10.15+) | |

No test runner configured for frontend (TypeScript type-check only via `tsc --noEmit`). Rust has 382 passing tests.

---

## Architecture

### Directory Layout
```
src/                          # React frontend
  components/                 # 13 subdirectories: chat, company, conversations,
                              #   dev, employees, import, layout, onboarding,
                              #   settings, shared, trial, ui, CommandPalette.tsx
  contexts/                   # ConversationContext, EmployeeContext, LayoutContext, TrialContext
  hooks/                      # 10 custom hooks (import pipeline, data quality, network, etc.)
  lib/                        # Types, Tauri command wrappers, utility modules

src-tauri/src/                # Rust backend (~21,900 LOC)
  lib.rs                      # Tauri command exports (all IPC entry points)
  db.rs                       # SQLite connection + 8 migrations
  provider.rs                 # Provider trait + factory (Anthropic, OpenAI, Gemini)
  providers/                  # Provider implementations
    anthropic.rs              # Claude API client
    openai.rs                 # OpenAI-compatible API client
    gemini.rs                 # Google Gemini API client
    mod.rs                    # Provider module re-exports
  chat.rs                     # Streaming chat, dual-path (BYOK/proxy), conversation trimming
  context.rs                  # Context builder, query classification, Alex persona
  documents.rs                # V3.0: folder CRUD, file discovery, parsing, chunking, FTS, watcher
  employees.rs                # Employee CRUD + parameterized filters
  data_quality.rs             # Validation, dedupe, column mapping, HRIS presets (28 tests)
  highlights.rs               # Structured review extraction pipeline
  signals.rs                  # Attrition/sentiment heuristics
  dei.rs                      # DEI representation analysis + small-n suppression
  trial.rs                    # Trial mode detection, limits, message counting
  device_id.rs                # Stable UUID per install
  pii.rs                      # PII regex scanner + redaction
  audit.rs                    # Redacted audit logging
  memory.rs                   # Cross-conversation memory + summaries
  keyring.rs                  # macOS Keychain API key storage (service: com.peoplepartner.app)
  backup.rs                   # AES-256-GCM encrypted backup/restore

proxy/                        # Cloudflare Workers (trial API proxy)
  src/index.ts                # KV quota tracking, rate limiting, HMAC signing
```

### Data Flow
```
User Input -> PII Scan -> Context Builder (query classification + employee retrieval
  + org aggregates + review highlights + memory + document chunks) -> AI Provider -> Audit Log -> Response
```

For document-referenced answers: Context builder retrieves relevant FTS5 chunks from indexed local files, includes them in the system prompt with citation instructions.

### Database (9 tables + FTS)
employees, conversations, company, settings, audit_log, review_cycles, performance_ratings, performance_reviews, enps_responses. Plus: review_highlights (extraction cache), document_folders, documents, document_chunks + document_chunks_fts (V3.0). 8 migrations total.

**Removed tables (migration 006):** insight_boards, insight_items, insight_annotations (chart canvas system removed in Phase A).

---

## Current State

### Fully Built and Working
- **Chat:** Streaming responses, conversation history, auto-titles, FTS search, cross-conversation memory
- **Multi-provider AI:** Provider trait abstraction with Anthropic Claude, OpenAI, and Google Gemini support. Frontend model picker per provider. API keys stored per-provider in macOS Keychain.
- **Context engine:** Query-adaptive retrieval (individual/team/org classification), dynamic token budgets, selected-employee prioritization, org-level aggregates, document chunk inclusion
- **Employee management:** CRUD, CSV/XLSX/TSV import with merge-by-email, department/manager filters
- **Performance data:** Ratings (1.0-5.0), text reviews with FTS, eNPS scores, structured highlight extraction
- **Data quality center:** Column mapping UI, validation rules, dedupe detection, fix-and-retry workflow, HRIS preset detection (BambooHR, Gusto, Rippling)
- **Document ingestion (V3.0):** Local folder indexing with FTS5 search, section-aware chunking (paragraph boundaries, hard-split for oversized), PII redaction on indexed content, SHA-256 change detection, FSEvents file watching, PDF/DOCX/XLSX/MD/TXT/CSV parsing, context builder integration with citation instructions, scan mutex for concurrency safety
- **Intelligence layer:** Attrition/sentiment signals (team-level, opt-in), DEI fairness lens (representation, rating parity, promotion delta, small-n suppression)
- **Personas:** 5 switchable HR personas (Alex, Jordan, Sam, Morgan, Taylor)
- **Answer verification:** Parallel SQL ground-truth checks on numeric claims
- **Command palette:** Cmd+K with fuzzy search across actions/conversations/employees
- **Protection:** PII auto-redaction (SSN, CC, bank), audit logging, CSP, markdown sanitization
- **Onboarding:** 7-step wizard (welcome, API key guide, company, employee import, disclaimer, telemetry, first prompt)
- **Settings:** Provider/model selection, API key management per provider, company profile, persona selector, document folder config, data path, telemetry toggle, backup/restore
- **Upgrade wizard:** 4-step flow (purchase, license entry, provider setup, complete) in UpgradePrompt
- **Monday digest:** Anniversaries and new hires on weekly first launch
- **Offline mode:** Read-only browsing of employees and conversation history
- **Accessibility:** WCAG AA contrast, 40px touch targets, focus traps in modals, reduced motion support
- **Distribution infra:** Code signing identity set, notarization config, auto-updater plugin, GitHub Actions release workflow with 5 of 8 secrets configured
- **Trial infra:** Cloudflare Workers proxy deployed (`hrcommand-proxy.hrcommand.workers.dev`), 50-msg quota per device via KV, dual-path chat routing, trial banner, message counter, employee limit (10), upgrade prompts
- **License system:** Remote validation via `peoplepartner.io/api/validate-license`, device_id for seat-limit enforcement (2 devices max), fail-open offline, PP-XXXX format validation (32 chars), license key input UI with seat-limit error handling
- **Payment integration:** Stripe checkout ($99 one-time, live mode), license key generation (PP-XXXX format), idempotent webhook processing, refund/dispute handling, license email delivery via Resend
- **Landing page:** peoplepartner.io on Vercel with download page, purchase flow, legal/support/status pages
- **Website entitlement DB:** Vercel Postgres with licenses, license_activations, stripe_webhook_events tables
- **Brand:** Logo teal #2D9199 as primary palette (50-900 scale), logo.png in welcome screen, generated app icons (32/128/256/.icns/.ico)

### Intentionally Removed
- **Analytics panel:** NL-to-SQL chart templates, Recharts integration, bar/pie/line charts (removed in Phase A to focus on conversational HR)
- **Insight canvas:** Pin charts to boards, annotations, report export (removed alongside analytics)
- **Recharts dependency:** Fully removed from frontend

### Stubbed / Partially Built
- **Auto-updater:** Plugin wired + UI hook mounted. `tauri.conf.json` has real pubkey but GitHub endpoint still needs repo URL
- **HMAC request signing:** Implemented in proxy + app; `TRIAL_SIGNING_SECRET` configured in both Cloudflare and GitHub Actions

### Not Started
- Org chart view (deferred to post-launch)
- SQLCipher encryption at rest (deferred)

---

## Recent Decisions

1. **Decision:** Remove analytics/charts entirely (Phase A) — **Reason:** Conversational HR is the core value prop; chart generation added complexity without proportional user value
2. **Decision:** Provider trait abstraction for multi-model support — **Reason:** Users bring their own API keys; supporting multiple providers reduces lock-in and broadens appeal
3. **Decision:** FTS5 for document retrieval (not vector embeddings) — **Reason:** No external embedding API needed, works fully offline, simpler architecture, good enough for HR document sizes
4. **Decision:** Section-aware chunking with hard-split fallback — **Reason:** Paragraph boundaries preserve context; hard-split at sentence/word boundaries handles edge cases like giant single paragraphs
5. **Decision:** PII scan-and-redact on indexed document content — **Reason:** Same privacy guarantee as chat; redacted content never leaves the machine
6. **Decision:** FSEvents watcher with scan mutex — **Reason:** Real-time re-indexing on file changes; mutex prevents concurrent scan corruption
7. **Decision:** Full rebrand HR Command Center to People Partner — **Reason:** Better name for the product category; completed across 94 files including license prefix (HRC- to PP-), bundle IDs, URLs, Keychain service names
8. **Decision:** Brand color #2D9199 (logo teal) replaces #0D9488 — **Reason:** Align app palette with actual logo and website; generated full 50-900 Tailwind scale
9. **Decision:** $99 one-time pricing, BYOK after purchase — **Reason:** Simple, honest pricing; trial proxy funds initial 50 messages
10. **Decision:** All intelligence features (attrition signals, DEI lens) require opt-in + disclaimers — **Reason:** Heuristic outputs must not be mistaken for predictions

---

## Known Issues and Debt

| Issue | Severity | Status |
|-------|----------|--------|
| Apple Developer enrollment pending (24-48h approval) | High | Blocking signed release |
| 3 GitHub Actions secrets missing (APPLE_CERTIFICATE, CERTIFICATE_PASSWORD, TEAM_ID) | High | Blocked on Apple approval |
| `tauri.conf.json` updater GitHub endpoint needs repo URL | Medium | Open |
| No frontend test runner (Jest/Vitest) | Low | Technical debt |
| No E2E manual verification of full purchase, license, seat limit flow | Medium | Pending first release |
| License revocation not detected by desktop (validate-once model) | Low | Conscious deferral |

---

## What's Next

**Immediate (blocked on Apple Developer approval):**
1. Complete Apple Developer enrollment, create Developer ID certificate, export .p12
2. Add remaining 3 GitHub Actions secrets (APPLE_CERTIFICATE, APPLE_CERTIFICATE_PASSWORD, APPLE_TEAM_ID)
3. Tag `v0.1.0`, push to trigger GitHub Actions release build, verify .dmg artifacts

**Before launch (E2E verification):**
4. Full E2E test: visit peoplepartner.io, buy, receive license email, download .dmg, install, enter license, verify trial lifts, provider setup, document indexing
5. Verify Stripe webhook fires and license email arrives via Resend
6. Verify 3rd device seat-limit rejection and offline resilience

**Post-launch:**
7. Monitor license revocation manually (validate-once model)
8. Consider adding Vitest for frontend component tests
9. Org chart view (parking lot)
10. ~~Consider renaming project directory~~ DONE — now at `~/Desktop/peoplepartner/app/`

---

## Cross-Surface Notes

- **Rebrand (2026-03-03):** Full rebrand from HR Command Center to People Partner landed across 94 files. License prefix changed HRC- to PP- (32 chars total). Keychain service is `com.peoplepartner.app` with 5 fallback paths for migration. Website URL is `peoplepartner.io`. Proxy URL unchanged (`hrcommand-proxy.hrcommand.workers.dev`).
- **Multi-provider (2026-02-27 to 2026-03-02):** Provider trait extracted from monolithic chat.rs. Three providers implemented (Anthropic, OpenAI, Gemini). Frontend provider picker and model selector added. API keys stored per-provider in Keychain.
- **Chart removal (2026-02-26):** Analytics panel, insight canvas, and Recharts fully removed. Migration 006 drops canvas tables. This was Phase A of launch prep.
- **Document ingestion (2026-03-02):** V3.0 feature. Migration 007 adds document_folders/documents/document_chunks + FTS5. Migration 008 adds unique constraint on chunks. ~600 LOC in documents.rs with 17 tests. Watcher uses FSEvents via notify crate.
- **E2E infrastructure (2026-03-04):** Tauri signing keys generated, Cloudflare proxy deployed and working, Stripe price ID corrected to live mode, download page points to GitHub releases, 5 of 8 GitHub Actions secrets set. Blocked on Apple Developer enrollment for the final 3.
- **Business repo:** All workstreams consolidated at `~/Desktop/peoplepartner/`. Desktop app at `app/` (Tauri/Rust), website at `site/` (Vercel/Next.js), demo video at `demo/`, marketing materials at `marketing/`. Each code subfolder has its own git. Parent repo tracks business docs.
- **Trial architecture:** Message-count only (50 msgs via Cloudflare proxy). Website's time-based trial system was removed during earlier hardening.
- **Seat limits:** Enforced server-side. Desktop sends UUID v4 device_id to validate-license. Website tracks activations in DB. 3rd device rejected with `SEAT_LIMIT_EXCEEDED`.
- **Test count:** 382 Rust tests passing. No frontend test framework.

---

*This file is the single source of truth for external Claude sessions. Update it at the end of any session with meaningful changes.*
