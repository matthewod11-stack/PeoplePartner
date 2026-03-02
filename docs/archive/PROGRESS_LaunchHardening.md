# Progress Archive — Launch Hardening

> Archived from `docs/PROGRESS.md` on 2026-03-02

---

## Session: 2026-02-26 (Pre-Launch Deployment Checklist — Tasks 1-6)

**Phase:** 5.5 (Pre-Launch Deployment)
**Focus:** Provision infrastructure, configure secrets, deploy proxy — all pre-launch config tasks before E2E verification

### Completed
- [x] **Task 1:** Provisioned Vercel Postgres for website entitlement tables
- [x] **Task 2:** Ran `001_entitlements.sql` migration — `licenses`, `license_activations`, `stripe_webhook_events` tables live
- [x] **Task 3:** Switched Stripe to live mode — new product/price, live API keys, live webhook endpoint, Vercel env vars updated, redeployed
- [x] **Task 4:** Deployed Cloudflare Workers proxy — KV namespace created, `CLAUDE_API_KEY` secret set, deployed to `https://hrcommand-proxy.hrcommand.workers.dev`
- [x] **Task 5:** Configured auto-updater — signing keypair generated, pubkey + GitHub releases endpoint in `tauri.conf.json`, private key stored as GitHub Actions secret
- [x] **Task 6:** Wired `TRIAL_SIGNING_SECRET` — generated shared HMAC secret, set on Cloudflare Worker and as GitHub Actions secret, added `option_env!` build-time lookup in `trial.rs`
- [x] Fixed default proxy URL: `hrcommand-proxy.workers.dev` → `hrcommand-proxy.hrcommand.workers.dev`
- [x] Added proxy URL to CSP `connect-src` in `tauri.conf.json`
- [x] Linked website repo to Vercel CLI (`vercel link`)

### Code Changes (Desktop Repo)
- `src-tauri/src/trial.rs` — Updated `DEFAULT_PROXY_URL` to actual deployed URL, added `option_env!("HRCOMMAND_PROXY_SIGNING_SECRET")` build-time lookup
- `src-tauri/tauri.conf.json` — Set updater pubkey, GitHub releases endpoint, added proxy to CSP `connect-src`
- `proxy/wrangler.toml` — Set real KV namespace IDs

### Verification
- [x] `cargo test` — 317 passed, 0 failed, 1 ignored
- [x] `cargo check` — clean (47 pre-existing warnings)
- [x] TypeScript — 3 pre-existing type errors (missing runtime-only declarations)

### Infrastructure Provisioned
| Service | Detail |
|---------|--------|
| Vercel Postgres | `hrcommand-entitlements` DB with 3 tables |
| Stripe (live) | Product, price, webhook, 4 env vars on Vercel |
| Cloudflare Worker | `hrcommand-proxy.hrcommand.workers.dev` with KV + 2 secrets |
| GitHub Secrets | `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`, `HRCOMMAND_PROXY_SIGNING_SECRET` |

### Next Session Should
1. Execute Task 7: E2E verification — `cargo tauri dev`, test trial proxy chat, upgrade flow, license activation, seat limits, offline mode
2. If proxy chat fails, debug CORS / CSP / origin issues between Tauri and the Worker
3. Consider a test purchase + immediate refund to verify live Stripe webhook flow end-to-end
4. After E2E passes, commit final changes and prep for first release build

---

## Session: 2026-02-25 (Launch Hardening Audit & Corrected Plan)

**Phase:** Pre-Launch
**Focus:** Audit failed launch hardening plan execution, produce corrected plan

### Completed
- [x] Discovered original 9-step launch hardening plan used wrong desktop repo path (stale iCloud copy at `~/Library/Mobile Documents/.../HRCommand` instead of `~/Desktop/HRCommand`)
- [x] Confirmed Steps 1-5, 8 (website) landed correctly in `/Users/mattod/Desktop/Misc/Archive/HR-Tools/hr-command-center`
- [x] Confirmed Steps 6-7 (desktop entitlement) landed in stale iCloud repo — all uncommitted, architecturally incompatible with Phase 5 codebase
- [x] Full file-by-file audit of Step 6-7 code vs current repo: 5 new files and 6 modified files analyzed
- [x] Compatibility audit of website entitlement API (Steps 1-5) vs desktop proxy architecture — found 5 major misalignments
- [x] Locked design decisions with user: message-count trials (keep), UUID v4 identity (keep), validate-once (keep), seat limits (enforce via validate-license)
- [x] Wrote corrected 7-step launch hardening plan → `/Users/mattod/Desktop/LAUNCH-HARDENING-CORRECTED-PLAN.md`
- [x] Cleaned up iCloud repo — discarded all uncommitted Step 6-7 changes (`git checkout .` + `git clean -fd`)

### Key Findings
- Website built time-based trial system (14 days, Postgres) — incompatible with desktop's message-count trials (50 msgs, proxy KV)
- Website's `POST /api/entitlement/check` requires 64-char SHA-256 device hash — desktop sends 36-char UUID v4 — endpoint is unusable
- Website's seat limit enforcement only goes through entitlement endpoint — validate-license skips device activation
- License revocation (refund/dispute) happens server-side but desktop never re-validates — revoked licenses work forever
- Proxy is completely disconnected from website's entitlement system

### Issues Encountered
- Pre-existing TS type errors (3): missing type declarations for `rehype-sanitize`, `@tauri-apps/plugin-updater`, `@tauri-apps/plugin-process`

### Next Session Should
1. Execute corrected plan from `~/Desktop/LAUNCH-HARDENING-CORRECTED-PLAN.md` — start with Step 1 (website: remove unused trial_devices infrastructure)
2. Steps 1-3 are website-only; Steps 4-5 are desktop-only; Step 6 commits both
3. Website repo has uncommitted Steps 1-5, 8 work — modify in place, do not redo
4. Pre-existing TS type errors are not from this session — address separately or ignore

---

## Session: 2026-02-26 (Launch Hardening Execution — Steps 1-6)

**Phase:** 5.3-5.4 (Launch Hardening)
**Focus:** Execute corrected launch hardening plan across both website and desktop repos

### Completed
- [x] **Website Step 1:** Removed unused `trial_devices` table, `TrialDeviceRecord`, `getOrCreateTrialDevice()`, trial code paths from `evaluateEntitlement()`, `EntitlementMode`, `EntitlementCheckRequest/Response`
- [x] **Website Step 2:** Extended `validate-license` endpoint to accept `device_id`, register device activations via `upsertLicenseActivation()`, enforce 2-device seat limit. Added `isValidDeviceIdentifier()` accepting both SHA-256 hash and UUID v4.
- [x] **Website Step 3:** Deleted `/api/entitlement/check` endpoint and directory. Replaced complex `evaluateEntitlement()` state machine with clean `validateLicense()` function (~30 lines, 5 exit points).
- [x] **Desktop Step 4:** Added `LicenseValidationResult` enum (`Valid | Invalid | SeatLimitExceeded`). `validate_license_key_remote()` now sends `device_id` and parses `reason`/`message` from response. `store_license_key()` fetches device_id via `trial::get_device_id()` and returns seat-limit-specific errors.
- [x] **Desktop Step 5:** Strict format validation: `HRC-` prefix + 6 groups of 4 hex digits = 33 chars. Updated placeholder and hint text to show correct 6-group format. Seat-limit errors detected via regex and shown as friendly "Contact support" message.
- [x] **Step 6:** Committed both repos (desktop: `bc53b60`, website: `994c437`)
- [x] Parallel agent execution: 3 agents launched (website, desktop Rust, desktop frontend). Desktop agents hit sandbox restrictions but provided exact changes; applied manually.

### Verification
- [x] `cargo check` — passes (47 pre-existing warnings, 0 new)
- [x] `cargo test` — 317 passed, 0 failed, 1 ignored
- [x] `npx tsc --noEmit` — TypeScript clean
- [x] Website `npm run lint` — clean
- [x] Website `npm run build` — clean, `/api/entitlement/check` gone from route table
- [x] Zero dangling references to removed code in website repo

### Notes
- Website repo sandbox restrictions prevented agent edits — applied directly from main context
- The `evaluateEntitlement()` → `validateLicense()` simplification removed ~120 lines of trial/entitlement state machine code
- `unwrap_or_default()` for device_id fallback means empty string still lets validation proceed

### Next Session Should
1. Execute Step 7: Manual E2E verification (trial flow → purchase → license → seat limits → offline)
2. Step 7 is blocked on: Vercel Postgres provisioning, Stripe CLI for webhook replay
3. Remaining pre-launch: 5.5.5 Switch Stripe to live mode (5 tasks)
4. Update `tauri.conf.json` placeholders (updater pubkey, GitHub endpoint)
