//! Runtime spend guard + shared pricing for the Exa adapter.
//!
//! `dead_code` is suppressed at module level because this module is fully
//! implemented and tested but not yet wired to a Tauri command.  The warnings
//! will naturally disappear when `recruiting_run_search` is added in the
//! follow-up task (S1.1 command layer).
#![allow(dead_code, unused_imports)]
//!
//! # Structure
//!
//! - `pricing` — pure estimation: `ExaOp`, `CostModel`, constants.
//!   `CostModel::estimate_search_config` is the S1.4 pre-run preview entry
//!   point; its arithmetic is the contract for the preview==runtime invariant.
//!
//! - `tracker` — mutable accumulator: `CostTracker`, `CostDecision`,
//!   `CostReport`, `CostSnapshot`.  Implements `CostGate` from
//!   `search::executor` so it slots into the run loop without changing it.
//!
//! # Usage
//!
//! ```rust,ignore
//! let mut cost = CostTracker::new(CostModel::default(), cfg.max_cost_usd);
//! let res = executor.run(&cfg, &mut cost).await?;
//! let report = cost.report(res.stats.stopped_early, None);
//! ```

pub mod pricing;
pub mod tracker;

// Re-export the public API so callers import from `recruiting::cost::*`.
pub use pricing::{CostModel, ExaOp, COST_PER_SEARCH_ESTIMATE};
pub use tracker::{CostDecision, CostReport, CostSnapshot, CostTracker};
