//! Headless intake conversation engine (FHR-85, roadmap S4.1).
//!
//! A graph of `IntakeNode`s the `ConversationEngine` traverses, turning a
//! hiring conversation into a structured `IntakeContext` -> `SearchConfig`.
//! Headless: no UI. The only dep-touching method is `IntakeNode::parse`,
//! behind the `IntakeProvider` / `ContentResearch` traits; tests inject fakes.
//!
//! NOTE: `pub use` re-exports are intentionally omitted until the modules
//! they reference are implemented (added in a later task).

pub mod context;
pub mod deps;
pub mod engine;
pub mod node;
pub mod phases;
pub mod runner;
pub mod schemas;
