//! `IdentityArbiter` — the optional LLM tiebreak seam for low-confidence merges.
//!
//! Spec §4 / §10. Pass 4 of the resolver (similar-name + similar-company
//! auto-merge) is the *only* pass that consults the arbiter. The contract is
//! strictly subtractive: the arbiter can **veto** a low-confidence auto-merge
//! the heuristics proposed, but it can never *invent* a merge — every candidate
//! pair the arbiter sees was already a heuristic match.
//!
//! - `NoopArbiter` (the production default) always returns `Ok(true)`, giving
//!   exact pure-heuristic parity with the TS `IdentityResolver` (no network) —
//!   this is what the ported golden-set oracle runs against.
//! - `FakeArbiter` (test-only) pops scripted verdicts from a queue so a test
//!   can assert that a `false` verdict blocks the Pass-4 merge.

use async_trait::async_trait;

use super::types::{ClusterSummary, IdentityError, MergeReason};

/// The same-person judgement seam consulted only by Pass 4 (low-confidence).
#[async_trait]
pub trait IdentityArbiter: Send + Sync {
    /// Return `true` if clusters `a` and `b` are the same person, given the
    /// heuristic `reason` that proposed the merge.
    async fn same_person(
        &self,
        a: &ClusterSummary,
        b: &ClusterSummary,
        reason: &MergeReason,
    ) -> Result<bool, IdentityError>;
}

/// Production default: always approves. Exact TS parity, zero network.
pub struct NoopArbiter;

#[async_trait]
impl IdentityArbiter for NoopArbiter {
    async fn same_person(
        &self,
        _a: &ClusterSummary,
        _b: &ClusterSummary,
        _reason: &MergeReason,
    ) -> Result<bool, IdentityError> {
        Ok(true)
    }
}

/// Test-only arbiter that pops scripted verdicts from a queue in order.
///
/// When the queue is exhausted it defaults to `true` (approve) so a partially
/// scripted test still terminates deterministically.
#[cfg(test)]
pub struct FakeArbiter {
    verdicts: std::sync::Mutex<std::collections::VecDeque<bool>>,
}

#[cfg(test)]
impl FakeArbiter {
    /// Build a `FakeArbiter` that returns `verdicts` in order, then `true`.
    pub fn returning(verdicts: Vec<bool>) -> Self {
        Self {
            verdicts: std::sync::Mutex::new(verdicts.into_iter().collect()),
        }
    }
}

#[cfg(test)]
#[async_trait]
impl IdentityArbiter for FakeArbiter {
    async fn same_person(
        &self,
        _a: &ClusterSummary,
        _b: &ClusterSummary,
        _reason: &MergeReason,
    ) -> Result<bool, IdentityError> {
        Ok(self.verdicts.lock().unwrap().pop_front().unwrap_or(true))
    }
}
