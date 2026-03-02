# Progress Archive — Launch Hardening

> Archived from `docs/PROGRESS.md` on 2026-03-02

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
