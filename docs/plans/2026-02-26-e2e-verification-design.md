# E2E Verification: Trial → Purchase → License Flow

**Date:** 2026-02-26
**Phase:** 5.5 (Pre-Launch Deployment)
**Scope:** Code audit + bug fixes (no live app testing)
**Status:** Complete

---

## Bugs Found & Fixed

### Bug 1: Proxy URL missing `/v1/messages` path (Critical) — FIXED

- `trial.rs:20` defaults to `https://hrcommand-proxy.hrcommand.workers.dev` (no path)
- `chat.rs:536` posted directly to that URL: `.post(proxy_url)`
- Worker at `index.ts:130` rejects anything except `pathname === "/v1/messages"` with 404
- **Result:** Every trial chat request would get 404

**Fix:** `chat.rs:535` now constructs endpoint: `format!("{}/v1/messages", proxy_url.trim_end_matches('/'))`. This handles base URLs from any source (env var, settings, default) without requiring them to include the path.

---

## Audit Results

| # | Integration Point | Status | Notes |
|---|---|---|---|
| 1 | Trial chat → Proxy | PASS | Headers, body, response parsing match. URL fixed. |
| 2 | Proxy → Anthropic API | PASS | Model override + max_tokens cap match Rust constants |
| 3 | License → Validation API | PASS | URL, body schema, response parsing, fail-open all correct |
| 4 | Upgrade flow → Website | PASS | URLs correct in constants.ts |
| 5 | HMAC signing | PASS | Payload format, key encoding, hash algo, hex output all match |
| 6 | CSP config | PASS | All 3 external domains in connect-src |
| 7 | Build + tests | PASS | 317 tests, 0 failures |
| 8 | ROADMAP cleanup | PASS | 5.5.5 checked off |
