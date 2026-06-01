//! Headless intake conversation engine (FHR-85, roadmap S4.1).
//!
//! A graph of `IntakeNode`s the `ConversationEngine` traverses, turning a
//! hiring conversation into a structured `IntakeContext` -> `SearchConfig`.
//! Headless: no UI. The only dep-touching method is `IntakeNode::parse`,
//! behind the `IntakeProvider` / `ContentResearch` traits; tests inject fakes.

//! Public items are reachable via their submodule paths (e.g.
//! `recruiting::intake::engine::ConversationEngine`,
//! `recruiting::intake::runner::create_intake_engine`). Crate-level `pub use`
//! re-exports are intentionally deferred to the S4.2 consumer so this
//! still-internal module doesn't carry an unused public surface.

pub mod context;
pub mod deps;
pub mod engine;
pub mod node;
pub mod phases;
pub mod runner;
pub mod schemas;
