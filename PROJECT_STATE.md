# HR Command Center — Project State

> Cross-surface context document. Shared across Claude Chat, Claude Code, and Cowork sessions.
> **Last regenerated:** 2026-02-26 | **Generated from:** codebase scan

---

## Project Overview

HR Command Center is a macOS desktop app that gives HR professionals (solo practitioners, accidental HR people, founders without HR) a private, AI-powered assistant that understands their specific company context. It runs as a Tauri app with a React frontend and Rust backend, stores all employee data locally in SQLite, connects to Claude API for AI chat, and auto-redacts PII before anything leaves the machine. The app is feature-complete through Phase V2 (intelligence layer, analytics, data quality) and Phase 5 (launch infrastructure: distribution, trial, license + seat limits, payment, landing page). Remaining before public launch: switch Stripe to live mode and E2E verification.

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

- **License system (5.3):** Remote validation via `hrcommandcenter.com/api/validate-license`, device_id sent for seat-limit enforcement (2 devices max), fail-open offline, strict HRC-XXXX×6 format validation, license key input UI in Settings with seat-limit error handling
- **Payment integration (5.4):** Stripe checkout ($99 one-time), license key generation (HRC-XXXX format), idempotent webhook processing, refund/dispute → REVOKED, license email delivery via Resend
- **Landing page (5.5):** hrcommandcenter.com on Vercel with download page, purchase flow, legal/support/status pages
- **Website entitlement DB:** Vercel Postgres with licenses, license_activations, stripe_webhook_events tables. Unused trial_devices and entitlement/check endpoint removed during launch hardening.

### Stubbed / Partially Built
- **Auto-updater:** Plugin wired + UI hook mounted, but `tauri.conf.json` has placeholder pubkey and GitHub endpoint
- **Proxy deployment:** Code complete but not deployed; `wrangler.toml` has placeholder KV namespace IDs
- **HMAC request signing:** Optional path implemented in proxy + app; needs `TRIAL_SIGNING_SECRET` configured in production
- **Stripe live mode:** Currently running test keys; 5.5.5 tasks to switch to live mode pending

### Not Started
- Org chart view (deferred to post-launch)
- Document/PDF ingestion (parking lot)
- SQLCipher encryption at rest (deferred)

---

## Recent Decisions

1. **Decision:** Message-count trial model (50 msgs via proxy), not time-based — **Reason:** Proxy-based tracking already works; time-based trials (website's trial_devices) were incompatible and removed
2. **Decision:** UUID v4 device identity, not hardware fingerprinting — **Reason:** Sufficient for seat tracking; hardware fingerprints add complexity and privacy concerns
3. **Decision:** Validate license once at entry, fail-open offline — **Reason:** Simpler than periodic re-validation; handle revocations manually via support for now
4. **Decision:** Seat limits enforced server-side (2 devices per license) — **Reason:** Desktop sends device_id to validate-license; website tracks activations in DB
5. **Decision:** Removed website's `evaluateEntitlement()` state machine → replaced with `validateLicense()` — **Reason:** Only validate-license endpoint uses it after deleting entitlement/check; 120 lines of trial/entitlement code removed
6. **Decision:** Website `isValidDeviceIdentifier()` accepts both SHA-256 hash and UUID v4 — **Reason:** Backward compatibility with any future hash-based clients while supporting desktop's UUID v4
7. **Decision:** Desktop `store_license_key()` uses `unwrap_or_default()` for device_id fallback — **Reason:** Empty string still lets validation proceed (server decides); better than skipping validation entirely
8. **Decision:** Proxy counter is authoritative, local counter syncs from `X-Trial-Used`/`X-Trial-Limit` headers — **Reason:** Prevents client-side counter drift
9. **Decision:** All intelligence features (attrition signals, DEI lens) require opt-in + disclaimers — **Reason:** Heuristic outputs must not be mistaken for predictions
10. **Decision:** $99 one-time pricing, BYOK after purchase — **Reason:** Simple, honest pricing; trial proxy funds initial 50 messages

---

## Known Issues & Debt

| Issue | Severity | Status |
|-------|----------|--------|
| `tauri.conf.json` updater pubkey + GitHub endpoint are placeholders | Medium | Open — blocks real auto-update |
| `proxy/wrangler.toml` KV namespace IDs are stubs | Medium | Open — blocks proxy deployment |
| Stripe still in test mode (5.5.5) | High | Open — blocks real payments |
| Proxy abuse mitigation is partial (origin allowlist + IP throttle + optional HMAC) | Medium | In progress |
| No frontend test runner (Jest/Vitest) | Low | Technical debt |
| No E2E manual verification of full purchase → license → seat limit flow | Medium | Step 7 pending |
| License revocation not detected by desktop (validate-once model) | Low | Conscious deferral |

---

## What's Next

**Immediate (next 1-2 sessions):**
1. Switch Stripe to live mode (5.5.5a-e): create live product/price, copy live API keys, create live webhook, update Vercel env vars
2. Populate production placeholders: updater pubkey, GitHub repo URL, Cloudflare KV namespace IDs
3. Deploy Cloudflare Workers proxy and test end-to-end trial flow
4. Configure `TRIAL_SIGNING_SECRET` for production HMAC signing

**Before launch (E2E verification — Step 7):**
5. Manual test: fresh install → trial mode → 50-msg limit → upgrade → purchase → license entry → seat activation → 3rd device rejected → offline resilience
6. Provision Vercel Postgres for website entitlement DB
7. Stripe CLI webhook replay testing

**Post-launch:**
8. Monitor license revocation manually (validate-once model)
9. Consider adding Vitest for frontend component tests
10. Org chart view (parking lot)

---

## Cross-Surface Notes

- **Launch hardening reconciliation (2026-02-25/26):** The original 9-step hardening plan accidentally landed Steps 6-7 in a stale iCloud repo. A corrected plan was written and executed: Steps 1-3 cleaned up the website (removed trial_devices, added seat limits to validate-license, deleted entitlement/check), Steps 4-5 updated the desktop (device_id in validation, seat-limit UI, format alignment). Both repos committed.
- **Two repos:** Website is at `/Users/mattod/Desktop/Misc/Archive/HR-Tools/hr-command-center` (Vercel/Next.js). Desktop is at `/Users/mattod/Desktop/HRCommand` (Tauri/Rust). These are independently versioned.
- **Trial architecture:** Trials are message-count only (50 msgs via Cloudflare proxy). The website's time-based trial system (14 days, Postgres trial_devices table) was removed during hardening — it was incompatible with the proxy model.
- **Seat limits:** Enforced server-side. Desktop sends UUID v4 device_id → website's `validate-license` endpoint registers activation → rejects 3rd device with `SEAT_LIMIT_EXCEEDED`. Frontend shows friendly "Contact support" message.
- **Proxy security:** Includes origin allowlist, per-IP throttling, optional HMAC signature verification with replay protection, and proxy-authoritative usage headers.
- **Test count:** 317 Rust tests passing. No frontend test framework — type-checking only.
- **features.json:** 57 features, all 57 passing. Only outstanding items: Stripe live mode (5.5.5) and E2E manual verification.

---

*This file is the single source of truth for external Claude sessions. Update it at the end of any session with meaningful changes.*
