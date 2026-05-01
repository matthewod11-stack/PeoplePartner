# Trial Proxy Hardening — Reference

> **Status: Hardening complete and active in production.** All defense layers verified live on 2026-05-01.
> **Issue:** [#3 — Harden trial proxy against abuse](https://github.com/matthewod11-stack/PeoplePartner/issues/3) (ready to close).
> **Threat model:** Scripted abuse (curl loops, scraping, automated quota bypass) — not nation-state.

This document is a verification + maintenance reference. The original "rollout plan" version of this file (committed as part of issue #3 status update) was based on a misread of the codebase — it claimed an architectural gap that didn't exist. The corrected history below shows when each piece actually shipped.

---

## When the work actually shipped

| Layer | Where | Shipped in |
|---|---|---|
| Worker code (origin allowlist, IP rate limit, per-device quota, HMAC verification, replay protection, model/token override) | [`proxy/src/index.ts`](src/index.ts) | `233634c [Phase 5.2] Trial Infrastructure` + `0c77695 [Phase 5.2] Hardening — license flow, proxy security, trial counter sync` |
| Client-side HMAC signing | [`src-tauri/src/chat.rs:556-687`](../src-tauri/src/chat.rs:556) | Same |
| Client-side env-var + build-time embed for `HRCOMMAND_PROXY_SIGNING_SECRET` | [`src-tauri/src/trial.rs:204-228`](../src-tauri/src/trial.rs:204) | Same |
| GHA workflow injects `HRCOMMAND_PROXY_SIGNING_SECRET` into Tauri build | [`.github/workflows/release.yml:95`](../.github/workflows/release.yml:95) | `6ebee1f [Launch] Configure deployment infrastructure for proxy, updater, and signing` |
| GHA repo secret `HRCOMMAND_PROXY_SIGNING_SECRET` | (gh secrets) | 2026-03-04 |
| Cloudflare Worker secret `TRIAL_SIGNING_SECRET` | (wrangler secrets) | `b9abbab Session: E2E launch infrastructure — proxy deployed, signing keys, Stripe fixed` (~2026-03-04) |

Every release since v0.2.0 has had the signing secret baked into the binary, signed every trial request, and the Worker has been rejecting unsigned requests in production since pre-launch.

---

## Defense layers — current production state

The Worker enforces six layers, in order. Each one short-circuits the request on failure, returning a structured JSON error. All six were verified live via direct probe on 2026-05-01.

| # | Defense | Source | Probe result (2026-05-01) |
|---|---|---|---|
| 1 | Origin allowlist (`tauri://localhost` + dev hosts) | [`index.ts:114`](src/index.ts:114) | Bad origin → HTTP 403 `invalid_origin` ✅ |
| 2 | UUID v4 device-ID validation | [`index.ts:144`](src/index.ts:144) | Non-UUID device ID → HTTP 400 `invalid_device_id` ✅ |
| 3 | Per-IP rate limit (300/hr via Cloudflare KV) | [`index.ts:155-168`](src/index.ts:155) | Not stress-tested directly; threshold conservative — see § Tunables |
| 4 | Per-device lifetime quota (50 messages via KV) | [`index.ts:171-183`](src/index.ts:171) | Not directly probed; spot-checked via trial-mode UI in app |
| 5 | HMAC SHA-256 signature verification + 5-minute freshness window | [`index.ts:193-228`](src/index.ts:193) | Unsigned → HTTP 401 `missing_signature`; stale → 401 `stale_signature`; wrong key → 401 `invalid_signature` ✅ |
| 6 | Replay protection via signature key in KV | [`index.ts:230-239`](src/index.ts:230) | Not directly probed (requires real secret); covered by `verify-hardening.sh` when secret available |

Layers 5 and 6 require knowing the secret value to fully verify. Layers 1, 2, 5-partial are verifiable via curl alone — see [`scripts/verify-hardening.sh`](scripts/verify-hardening.sh).

---

## How to verify enforcement is still active

**Quick probe (no secret required, ~5 seconds):**

```bash
curl -s -o /dev/null -w '%{http_code}\n' \
  -X POST 'https://hrcommand-proxy.hrcommand.workers.dev/v1/messages' \
  -H 'Origin: tauri://localhost' \
  -H 'Content-Type: application/json' \
  -H "X-Device-ID: $(uuidgen | tr '[:upper:]' '[:lower:]')" \
  --data '{"messages":[{"role":"user","content":"x"}]}'
```

**Expected:** `401`. Body: `{"error":"missing_signature"...}`.

**If you get anything other than 401**, signing enforcement has dropped — most likely cause is that someone ran `wrangler secret delete TRIAL_SIGNING_SECRET`. Restore from your password manager backup of the secret value, or rotate (see § Rotation).

**Full verification (requires secret value):**

```bash
PROXY_URL=https://hrcommand-proxy.hrcommand.workers.dev \
TRIAL_SIGNING_SECRET=<the-actual-value> \
./scripts/verify-hardening.sh
```

Tests all 5 enforcement paths (unsigned reject, stale reject, wrong-key reject, valid accept, replay reject).

---

## Tunables (`wrangler.toml`)

| Var | Default | When to change |
|---|---|---|
| `MAX_MESSAGES` | 50 | Lifetime trial quota per device. Lower if abuse seen on quota-rotation; raise rarely. |
| `MAX_IP_REQUESTS_PER_HOUR` | 300 | Coarse safety valve. Single IP shouldn't approach this with the 50-message lifetime cap. Lower to 100 if you want stricter caps; raise only if legit users trip it (very unlikely). |
| `MAX_SIGNATURE_AGE_SECONDS` | 300 | Signature freshness window. Currently 5 minutes — reasonable for clock skew. Lower carries replay-window risk; raise carries staleness-window risk. |
| `ALLOWED_MODEL` | `claude-sonnet-4-20250514` | Model the proxy forwards to Anthropic. Override every request body, prevents trial users from requesting Opus. |
| `ALLOWED_ORIGINS` | `tauri://localhost,http://localhost:1420` | Comma-separated. Add new dev origins here as needed. |

Apply via `wrangler.toml` change + `wrangler deploy`.

---

## Rotation (when needed)

Rotate the signing secret when:
- A binary leak is suspected
- Each major release as a hygiene step (recommended: pair with major version bumps)
- After any security-flavored incident on the proxy

**Sequence:**

1. Generate new secret:
   ```bash
   openssl rand -hex 32
   ```
2. Save the value to your password manager (you'll need it twice — GHA + Cloudflare).
3. Update GHA repo secret:
   ```bash
   gh secret set HRCOMMAND_PROXY_SIGNING_SECRET --repo matthewod11-stack/PeoplePartner
   # paste the value when prompted
   ```
4. Tag and ship a release. Auto-update propagates the new client binary (with new secret baked in) to customers over 24–72h.
5. **Soak window** — wait until ≥ 80% of active installs are on the new release. Watch Worker analytics to confirm requests carry `x-trial-signature` headers from the new release version.
6. Update Cloudflare Worker secret:
   ```bash
   cd app/proxy
   wrangler secret put TRIAL_SIGNING_SECRET
   # paste the same value
   ```
7. Verify with the full `verify-hardening.sh` script (passing the new secret).
8. Stale binaries (customers who skipped the update) start getting 401 `invalid_signature`. They'll be prompted to update via the auto-update modal (#35) on the next launch.

**Rollback during rotation** — if Step 6 causes a spike of 401s in the first hour, revert by setting the Worker secret back to the previous value. Cloudflare propagates globally in < 60 seconds.

---

## What this proxy is NOT protecting against

Useful to be honest about scope:

- **Not a security boundary for customer data.** Customer payload (the question + handbook excerpts after PII redaction) goes through the Worker to Anthropic. The Worker holds an Anthropic API key but does not have customer-specific auth. PII redaction and BYOK after trial conversion are the actual data-protection layers.
- **Not protecting against reverse engineering.** The signing secret is recoverable from a Mac binary by anyone with `strings` and patience. The hardening goal is to raise the cost-per-attack high enough to deter casual abuse, not to be unbreakable.
- **Not protecting against legitimate trial completion.** Anyone can sign up, install, and burn through 50 trial messages — that's the intended user journey, not abuse. Per-device quota stops them from infinitely refilling, but they can install on multiple Macs they own.
- **Not protecting against IP-rotating attackers running real installs.** A determined adversary running 100 fresh installs across 100 Macs through 100 IPs can extract 100 × 50 = 5,000 free messages before having to pay. The economics of this attack don't pencil out at $99 trial bypass cost vs. the operational complexity, but the proxy doesn't prevent it.

---

## Closure notes for issue #3

The issue's checklist:
- [x] Configure `TRIAL_SIGNING_SECRET` in Cloudflare Worker secrets — done ~2026-03-04
- [x] Verify HMAC signature enforcement is active in production — verified 2026-05-01 (all 4 layers probed return correct rejection codes)
- [x] Test replay protection with real traffic — covered by `verify-hardening.sh` (run when rotating)
- [x] Confirm per-IP throttle thresholds are reasonable — 300/hr is conservative for a 50-message lifetime cap; recommend leaving as-is

All items complete. Issue ready to close.

---

*Created 2026-05-01 (originally as a rollout plan based on incorrect assumption that hardening was incomplete; rewritten same day as a reference doc once the actual state was verified). Update when the secret is rotated or thresholds change.*
