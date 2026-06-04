//! Scoring engine foundation (FHR-77 — Recruit S2.1).
//!
//! This submodule is the foundation the MS2 scoring pipeline (S2.2 signal
//! extraction, S2.3 score calculator + narrative, S2.4 differential eval)
//! layers onto. FHR-77 lands two foundational pieces:
//!
//!   - [`templates`] — the prompt-template loader: the three Sourcerer
//!     `scoring-*.md` templates vendored **verbatim** (`include_str!`) plus a
//!     faithful port of the TS `template-loader` (front-matter parse +
//!     `{{var}}` interpolation). The verbatim constraint is load-bearing: the
//!     S2.4 differential eval treats the TS golden set as a port-fidelity
//!     oracle, which only holds if the prompts are byte-identical.
//!   - The recruiting LLM seam scoring rides on is the shared
//!     [`crate::recruiting::intake::deps::IntakeProvider`] over
//!     `crate::chat::send_message` (BYOK key lookup, PII redaction, provider
//!     routing). S2.2/S2.4 use its `structured_output`; S2.3 narrative uses
//!     its `chat` (free-text) path.

// Foundation ahead of wiring: the loader + `ScoringPrompt` registry are
// consumed by S2.2 (signal extraction) and S2.3 (narrative). Mirrors
// `recruiting/intake/mod.rs`, which carries the same allow for the same reason.
#![allow(dead_code)]

pub mod evidence;
pub mod sanitize;
pub mod schemas;
pub mod templates;
