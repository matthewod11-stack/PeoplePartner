// The UI consumer and crate-level re-exports for identity aren't wired yet;
// suppress dead-code warnings module-wide until Task 4's resolver lands.
#![allow(dead_code)]

//! Identity foundation for the Sourcerer module (FHR-73 S1.1 Task 2).
//!
//! Submodule structure:
//!   - `types`     — `IdentifierType`, `ConfidenceLevel`, `ObservedIdentifier`
//!   - `normalize` — per-type normalization functions + dispatcher
//!   - `matching`  — `names_match`, `levenshtein`, `names_similar`, `split_name_company`
//!   - `extract`   — `extract_identifiers`, `extract_name` (used by Task 3 discovery)
//!
//! `resolve()`, `PersonIdentity`, `ResolvedCandidate`, and merge types land in Task 4.

pub mod extract;
pub mod matching;
pub mod normalize;
pub mod types;
