# Trial Proxy Hardening Runbook

> **Issue:** [#3 — Harden trial proxy against abuse](https://github.com/matthewod11-stack/PeoplePartner/issues/3)
> **Threat model:** Scripted abuse (curl loops, scraping, automated quota bypass) — not nation-state attack.
> **Status:** Worker code is ready. Activation gated on a client-side secret distribution decision (see § "Architectural gap" below).

---

## Current state (read this before flipping anything)

The Worker (`proxy/src/index.ts`) and desktop client (`src-tauri/src/chat.rs`, `src-tauri/src/trial.rs`) both have HMAC signing implemented:

| Layer | Status |
|---|---|
| Origin allowlist (Worker) | ✅ active |
| Per-IP rate limit (Worker, 300/hr) | ✅ active |
| Per-device quota (Worker, 50 messages) | ✅ active |
| Body / model / max_tokens validation (Worker) | ✅ active |
| HMAC SHA-256 signature verification (Worker) | ⚠️ wired but inert — only enforces if `TRIAL_SIGNING_SECRET` is set |
| 5-minute timestamp freshness window (Worker) | ⚠️ inert until secret set |
| Replay protection via KV (Worker) | ⚠️ inert until secret set |
| Client signs requests (`compute_trial_signature` in `chat.rs`) | ⚠️ wired but inert — only signs if `proxy_signing_secret` is in settings table |
| Secret provisioning path on customer installs | ❌ **MISSING** — see § "Architectural gap" |

---

## Architectural gap (must resolve before activating signing)

The desktop client reads `proxy_signing_secret` from the SQLite `settings` table (`trial.rs:204`). The settings table starts empty on every install, and **no UI or build-time path populates it**. There is also no env-var fallback in the Rust code, despite the original `proxy/README.md` mentioning `HRCOMMAND_PROXY_SIGNING_SECRET` — that env var is not read anywhere.

**Consequence:** If we run `wrangler secret put TRIAL_SIGNING_SECRET` on the Worker right now, every trial-mode install (every v0.2.4 customer who hasn't activated a license) will start failing with HTTP 401 `missing_signature` on the next trial message. Trial mode breaks for everyone.

### Three options for closing the gap

| Option | Approach | Pros | Cons |
|---|---|---|---|
| **A. Build-time embed** | Add `option_env!("PEOPLEPARTNER_PROXY_SIGNING_SECRET")` in `trial.rs`, fall back to settings. Pass via GHA secret at release build. | Simple. Works for all installs. Rotates with each release. | Recoverable from binary by reverse engineering. Sufficient for stated threat model. |
| **B. Settings UI + ship-with-config** | Add a Settings UI field, embed default value in `tauri.conf.json` at build. | Same as A but also lets advanced users override. | More UI surface. Same secret-recoverability profile as A. |
| **C. Defer signing, lean on IP+quota** | Don't activate. Document that scripted abuse is constrained to 50 messages × N IPs. | Zero rollout risk. | Leaves a real abuse vector — single attacker rotating IPs can drain unlimited quota across fresh device IDs. |

**Recommendation:** **A**. Lowest-friction, matches the stated threat model, ships in a normal release with no UX surface area.

---

## Rollout sequence (assuming Option A)

Each step is independently verifiable. Do not skip the soak windows.

### Step 1 — Generate the secret

```bash
openssl rand -hex 32   # produces a 64-char hex string. Save this VALUE — you'll use it twice.
```

### Step 2 — Embed in client (next release, e.g. v0.2.5)

Implement option A:

1. In `src-tauri/src/trial.rs`, add a build-time fallback in `get_proxy_signing_secret()`:
   ```rust
   pub async fn get_proxy_signing_secret(pool: &DbPool) -> Result<Option<String>, SettingsError> {
       // Build-time secret takes precedence over settings (which is dev/manual-test path).
       if let Some(baked) = option_env!("PEOPLEPARTNER_PROXY_SIGNING_SECRET") {
           if !baked.is_empty() {
               return Ok(Some(baked.to_string()));
           }
       }
       settings::get(pool, KEY_PROXY_SIGNING_SECRET).await
   }
   ```
2. Add `PEOPLEPARTNER_PROXY_SIGNING_SECRET` as a GitHub Actions repository secret with the value from Step 1.
3. In the release workflow (`.github/workflows/release.yml`), pass the env var to the Tauri build step:
   ```yaml
   - name: Build app
     env:
       PEOPLEPARTNER_PROXY_SIGNING_SECRET: ${{ secrets.PEOPLEPARTNER_PROXY_SIGNING_SECRET }}
     run: ...
   ```
4. Tag and ship v0.2.5. Auto-update will roll customers forward.

### Step 3 — Soak (24–72 hours)

Watch for adoption. The auto-updater should move ≥ 80% of v0.2.4 trial users to v0.2.5 within 48 hours under normal conditions. Anyone still on v0.2.4 in trial mode after 72h is either:
- Offline (won't be affected by Step 4)
- Has disabled auto-update (rare; will be temporarily blocked by Step 4 until they update)

Use Cloudflare Worker analytics to confirm requests now include `x-trial-signature` headers before proceeding.

### Step 4 — Activate Worker enforcement

```bash
cd app/proxy
wrangler secret put TRIAL_SIGNING_SECRET
# paste the value from Step 1 when prompted
```

Verify the secret is set:

```bash
wrangler secret list
```

You should see `TRIAL_SIGNING_SECRET` in the output (value not shown).

### Step 5 — Verify enforcement

Run the verification script:

```bash
./scripts/verify-hardening.sh
```

It tests:
- Unsigned request → expects HTTP 401 `missing_signature`
- Stale-timestamp signed request → expects HTTP 401 `stale_signature`
- Wrong-secret signed request → expects HTTP 401 `invalid_signature`
- Replayed signed request → expects HTTP 409 `replay_detected`
- Valid signed request → expects HTTP 200

A signed v0.2.5 desktop install should now successfully send a trial message (visible in `chat.rs` log + Worker analytics).

### Step 6 — Confirm IP throttle thresholds

Current default: `MAX_IP_REQUESTS_PER_HOUR = 300`.

Sanity check: a single legitimate user generates roughly 50 messages per trial (one-shot). Even a power user testing across multiple devices won't approach 300/hr from one IP. The current threshold is a safety valve against scripted floods — it should not affect legitimate use.

Reasonable adjustments:
- **Lower** to 100/hr if you want stricter caps after Step 4 lands. Run for a week and review `rate_limited` counts in Worker logs.
- **Raise** to 600/hr only if you see legitimate users tripping it (very unlikely with the 50-message lifetime cap).

Update via `wrangler.toml` and redeploy:

```toml
[vars]
MAX_IP_REQUESTS_PER_HOUR = "300"
```

```bash
wrangler deploy
```

---

## Rollback

If Step 4 causes a spike in `missing_signature` 401s:

```bash
wrangler secret delete TRIAL_SIGNING_SECRET
```

Effect: Worker stops enforcing signatures (back to pre-Step-4 state). Trial messages flow again. No customer data lost, no auto-update needed. Rollback window is < 60 seconds.

---

## Operational notes

- **The signing secret is not a security boundary.** It's a friction layer against scripted abuse. Treat it like a tier-1 secret in CI but not like a customer key.
- **Rotate every major release.** New secret, new GHA build, new Worker `wrangler secret put`. Stale builds get cut off naturally as auto-update advances.
- **Worker analytics:** Watch for elevated `invalid_signature` rates after activation — those are either reverse-engineering attempts (which is fine, the secret is recoverable, but the fact that someone bothers means you're seeing real abuse) or stale-build customers.
- **Trial proxy is Mac-only by definition.** No Windows / Linux clients exist to coordinate with.

---

## Closure criteria for issue #3

- [ ] Decision recorded for Option A / B / C (this runbook recommends A)
- [ ] Step 2 implemented and shipped in a tagged release
- [ ] Step 4 executed; `wrangler secret list` confirms `TRIAL_SIGNING_SECRET` set
- [ ] Step 5 verification script passes all assertions
- [ ] Soak window monitored (3–7 days post-activation, no abnormal 401 spikes)
- [ ] Issue closed with a comment linking the verification run

---

*Created 2026-05-01. Update after each rollout step with the date executed and any deviations.*
