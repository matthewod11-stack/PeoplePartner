// Feature flags — compile-time gates for in-development modules.
//
// These are plain constants, deliberately NOT settings-backed: a flag here
// hides an unfinished module from production users while it's being built, and
// is flipped by editing this file + rebuilding. There's nothing to toggle at
// runtime yet — the gated views are empty skeletons. When a module graduates
// to real, user-facing functionality, migrate its flag to a settings-table
// toggle (mirroring the Signals / Fairness "Intelligence Features" pattern in
// SettingsPanel) so users/support can enable it without a rebuild.
//
// Typed `: boolean` (not the inferred literal `false`) on purpose — it stops
// TypeScript from narrowing reads to dead branches, so `if (RECRUITING_ENABLED)`
// and `RECRUITING_ENABLED ? … : …` type-check cleanly with the flag either way.

/**
 * Recruit module (talent sourcing) — FHR-70 → FHR-91.
 * OFF in production until the module is built out. Flip to `true` locally to
 * develop S0.2 (Rust `recruiting` module) and S0.3 (live Exa round-trip)
 * against the UI seam established in S0.1.
 *
 * ⚠️ This flag has a Rust mirror (#109): `RECRUITING_ENABLED` in
 * `src-tauri/src/commands/recruiting.rs` gates the IPC surface itself —
 * with it off, every recruiting command returns `FeatureDisabled` before
 * touching the Keychain, database, or network. Flip BOTH flags together;
 * flipping only this one makes every recruiting call fail with
 * `{ kind: 'FeatureDisabled' }`.
 */
export const RECRUITING_ENABLED: boolean = false;
