//! People Map — Prep Briefs (FHR-101 arc).
//!
//! One "Prep Brief" action on an employee profile: grounded Facts (cited to
//! local records) + Threads-to-pull (labeled inference). Local data only,
//! through the audited chat seam (#112). Briefs are ephemeral: rendered on
//! demand, never persisted; the only durable trace is the audit row.
//!
//! Hard constraint (People Map decision 6): no numeric or ordinal assessment
//! of a person exists anywhere in this module — no such field, no such
//! output. This is enforced by the source-scan lock test in `schema.rs`,
//! which forbids the assessment vocabulary from appearing anywhere in this
//! module's Rust source. New Rust source files added to this module MUST be
//! added to the lock test's manifest. (Prompt templates under `prompts/` are
//! exempt by design — prohibiting the vocabulary requires naming it.)

pub mod brief;
pub mod context;
pub mod schema;
