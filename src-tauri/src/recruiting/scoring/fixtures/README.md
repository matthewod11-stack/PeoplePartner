# Scoring differential-eval fixtures

## `golden-ts-reference.json`

The frozen TypeScript reference for the **FHR-80 (Recruit S2.4) differential eval oracle** — the MS2 port-fidelity gate. It is the *contract* the Rust scorer (`../score.rs`) is diffed against by `../eval_differential.rs`:

- `meta.config` — the `ScoringConfig` used (golden weights 0.30/0.25/0.20/0.15/0.10, thresholds 72/48, penalties 2/5/10).
- `meta.sourcererCommit` — provenance: the Sourcerer commit whose `calculateScore`/`assignTier` produced these values.
- `golden[15]` — the Sourcerer golden set (each: `expectedSignals` input, `expectedTier`, and the TS-computed `ts.{total,breakdown,tier}`).
- `adversarial[11]` — vectors that exercise the formula paths the golden 15 never hit (`redFlags=[]`, `confidence=1` for all 15): red-flag penalties (low/medium/high/mixed-sum), per-dimension confidence gating (`confidence-zero/partial/mixed`), clamp-low, clamp-high, and the inclusive tier boundaries (72.0 / 48.0).

**This is captured TS output, not hand-authored.** The Rust side recomputes independently and diffs at `1e-6` tolerance with exact tier match — a genuine cross-implementation check, not a re-derivation.

## Provenance (current)

Captured from **Sourcerer commit `4e1e0f8`** (`~/Projects/Sourcerer`) via `packages/eval/src/__tests__/dump-rust-reference.test.ts`.

## Re-vendor procedure (when the TS scorer changes)

This fixture is a *frozen contract*: there is **no automated cross-repo drift detection** (the dump script lives in the separate Sourcerer repo). If `packages/scoring/src/score-calculator.ts` changes in Sourcerer — weights, penalties, clamp, tier logic — this fixture **silently goes stale** and the gate will pass against the old expectations. A TS scoring change must be a *deliberate* re-vendor:

```bash
# 1. Regenerate the reference from the current Sourcerer TS
cd ~/Projects/Sourcerer/packages/eval && pnpm vitest run dump-rust-reference

# 2. Copy it here
cp tmp/golden-ts-reference.json \
   ~/Desktop/peoplepartner/app/src-tauri/src/recruiting/scoring/fixtures/golden-ts-reference.json

# 3. Re-run the differential gate (must stay green, or the port needs updating to match)
cd ~/Desktop/peoplepartner/app
cargo test --manifest-path src-tauri/Cargo.toml golden_set_matches_ts_reference adversarial_vectors_match_ts_reference

# 4. Update meta.sourcererCommit provenance above if it changed.
```

If step 3 fails after a deliberate re-vendor, the **Rust port** (`../score.rs`) must be updated to match the new TS reference — do **not** loosen the `1e-6` tolerance.

## Narrative grounding scanner — format assumption

`../narrative.rs::scan_phantom_citations` (used by the `#[ignore]`d `narrative_grounding_cites_only_real_evidence` acceptance test) matches citation tokens with the regex `ev-[A-Za-z0-9]+` — the canonical lowercase-hyphen form the narrative prompt instructs. Off-format hallucinations (`EV-1`, `ev_1`, IDs split across markup) are **not** caught. This is an accepted limitation for a best-effort, manually-run acceptance probe (not a CI gate); harden the regex / assert output format first if that test is ever promoted to a load-bearing role.
