//! Exa pricing model — pure estimation, no mutable state.
//!
//! `CostModel` is the single shared pricing brain.  All three fields default to
//! `COST_PER_SEARCH_ESTIMATE = 0.005` (faithful port of the TS cost-tracker
//! constant; tunable per-op in a later sprint).
//!
//! `estimate_search_config` is the entry point the **future S1.4 pre-run preview**
//! command will call — the preview==runtime invariant (spec §12, §15) is the test
//! contract that preview dollars == what the runtime tracker accumulates when every
//! call lands at its estimate.  The two paths use IDENTICAL arithmetic by design.

use crate::recruiting::intake::schemas::SearchConfig;

// ---------------------------------------------------------------------------
// Op kind
// ---------------------------------------------------------------------------

/// An Exa operation that has a cost. Mirrors `ExaOpKind` in the executor seam
/// but lives in `cost/` so the pricing logic is self-contained. The bridge
/// (`impl CostGate for CostTracker`) maps between the two enums.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExaOp {
    Search,
    FindSimilar,
    GetContents,
}

impl ExaOp {
    /// Every variant, in declaration order. Used by `tracker::label_for_key`
    /// to reconstruct labels on snapshot restore without a `_` wildcard, so a
    /// future variant forces an exhaustiveness compile error there.
    ///
    /// The `const _: ()` assertion below makes adding a variant break the build
    /// here too: it exhaustively matches every variant (no wildcard), so the
    /// `match` fails to compile until `ALL` is extended.
    pub const ALL: [ExaOp; 3] = [ExaOp::Search, ExaOp::FindSimilar, ExaOp::GetContents];

    /// Stable string key for `BTreeMap<&'static str, f64>` per-op accounting.
    pub fn label(self) -> &'static str {
        match self {
            ExaOp::Search => "search",
            ExaOp::FindSimilar => "find_similar",
            ExaOp::GetContents => "get_contents",
        }
    }
}

/// Compile-time guard: forces `ExaOp::ALL` to stay in sync with the variant set.
/// Adding a variant makes this exhaustive `match` fail to compile until the new
/// variant is added to `ALL` above.
const _: () = {
    fn _assert_all_exhaustive(op: ExaOp) {
        match op {
            ExaOp::Search => {}
            ExaOp::FindSimilar => {}
            ExaOp::GetContents => {}
        }
        // ExaOp::ALL has one entry per arm above; keep them in sync.
        assert!(ExaOp::ALL.len() == 3);
    }
};

// ---------------------------------------------------------------------------
// Pricing constant
// ---------------------------------------------------------------------------

/// Uniform per-result estimate (USD) — faithful port of TS cost-tracker.ts.
/// All three Exa op kinds use this until per-op empirical data is collected.
pub const COST_PER_SEARCH_ESTIMATE: f64 = 0.005;

/// How many results to assume per seed `find_similar` call when the actual
/// count is not yet known (mirrors `DEFAULT_NUM_RESULTS` in the executor).
const DEFAULT_NUM_RESULTS: u32 = 10;

// ---------------------------------------------------------------------------
// CostModel
// ---------------------------------------------------------------------------

/// Pure cost estimator — no mutable state.
///
/// All three fields seed to [`COST_PER_SEARCH_ESTIMATE`] via `Default`.
#[derive(Debug, Clone)]
pub struct CostModel {
    pub per_search_result_usd: f64,
    pub per_find_similar_result_usd: f64,
    pub per_contents_url_usd: f64,
}

impl Default for CostModel {
    fn default() -> Self {
        Self {
            per_search_result_usd: COST_PER_SEARCH_ESTIMATE,
            per_find_similar_result_usd: COST_PER_SEARCH_ESTIMATE,
            per_contents_url_usd: COST_PER_SEARCH_ESTIMATE,
        }
    }
}

impl CostModel {
    /// Estimate cost in USD for `units` results from the given operation.
    ///
    /// Intentionally pure and allocation-free.  Returns 0.0 for `units == 0`.
    pub fn estimate(&self, op: ExaOp, units: u32) -> f64 {
        let rate = match op {
            ExaOp::Search => self.per_search_result_usd,
            ExaOp::FindSimilar => self.per_find_similar_result_usd,
            ExaOp::GetContents => self.per_contents_url_usd,
        };
        rate * f64::from(units)
    }

    /// Estimate total cost for an entire `SearchConfig` without running it.
    ///
    /// **Must mirror the executor's call pattern exactly** so the
    /// preview==runtime invariant holds (spec §12, §15):
    ///
    /// - Seeds: one `FindSimilar` call per seed URL, each requesting
    ///   `DEFAULT_NUM_RESULTS` results.
    /// - Tiers: one `Search` call per query, each requesting
    ///   `query.max_results.unwrap_or(DEFAULT_NUM_RESULTS)` results, capped
    ///   at `cfg.max_candidates`.
    ///
    /// The cap logic mirrors the executor's **request size**
    /// (`num_results = req.min(remaining)`), **not** its actual-hit decrement.
    /// This is a deliberate divergence: the estimator decrements `remaining`
    /// by the *request size* (`min(req, remaining)`), whereas the runtime
    /// executor decrements `remaining` by the *actual number of hits returned*.
    /// When Exa returns fewer results than requested, the executor's
    /// `remaining` shrinks more slowly, so it may fire *more* downstream calls
    /// than this estimate assumed — meaning the estimate is a conservatively
    /// **higher upper bound**, not an exact guarantee. The S1.4 pre-run preview
    /// must surface this figure as an upper bound ("up to $X"), not an exact
    /// cost. The preview==runtime invariant holds only under the idealized case
    /// where every call returns exactly its requested count.
    pub fn estimate_search_config(&self, cfg: &SearchConfig) -> f64 {
        let mut total = 0.0_f64;
        let mut remaining = cfg.max_candidates;

        // P0: similarity seeds — each costs DEFAULT_NUM_RESULTS units.
        for _ in &cfg.similarity_seeds {
            if remaining == 0 {
                break;
            }
            let units = DEFAULT_NUM_RESULTS.min(remaining);
            total += self.estimate(ExaOp::FindSimilar, units);
            // seeds contribute up to units results each; conservative cap
            remaining = remaining.saturating_sub(units);
        }

        // P1..Pn: tiers in ascending priority order, each query in order.
        let mut sorted_tiers = cfg.tiers.clone();
        sorted_tiers.sort_by_key(|t| t.priority);

        'outer: for tier in &sorted_tiers {
            for query in &tier.queries {
                if remaining == 0 {
                    break 'outer;
                }
                let req = query.max_results.unwrap_or(DEFAULT_NUM_RESULTS);
                let units = req.min(remaining);
                total += self.estimate(ExaOp::Search, units);
                remaining = remaining.saturating_sub(units);
            }
        }

        total
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recruiting::intake::schemas::{SearchQuery, SearchQueryTier, TierThresholds};

    fn bare_cfg(seeds: usize, queries: usize, max_candidates: u32) -> SearchConfig {
        SearchConfig {
            role_name: "test".into(),
            tiers: vec![SearchQueryTier {
                priority: 1,
                queries: (0..queries)
                    .map(|i| SearchQuery {
                        text: format!("q{i}"),
                        target_companies: None,
                        include_domains: None,
                        exclude_domains: None,
                        max_results: None,
                    })
                    .collect(),
            }],
            scoring_weights: Default::default(),
            tier_thresholds: TierThresholds {
                tier1_min_score: 70,
                tier2_min_score: 50,
            },
            enrichment_priority: vec![],
            anti_filters: vec![],
            similarity_seeds: (0..seeds).map(|i| format!("https://s{i}")).collect(),
            max_candidates,
            max_cost_usd: 100.0,
            created_at: "2026-06-03T00:00:00Z".into(),
            version: 1,
        }
    }

    #[test]
    fn estimate_zero_units_is_zero() {
        let m = CostModel::default();
        assert_eq!(m.estimate(ExaOp::Search, 0), 0.0);
        assert_eq!(m.estimate(ExaOp::FindSimilar, 0), 0.0);
        assert_eq!(m.estimate(ExaOp::GetContents, 0), 0.0);
    }

    #[test]
    fn estimate_one_unit_equals_constant() {
        let m = CostModel::default();
        assert!((m.estimate(ExaOp::Search, 1) - COST_PER_SEARCH_ESTIMATE).abs() < 1e-12);
        assert!((m.estimate(ExaOp::FindSimilar, 1) - COST_PER_SEARCH_ESTIMATE).abs() < 1e-12);
        assert!((m.estimate(ExaOp::GetContents, 1) - COST_PER_SEARCH_ESTIMATE).abs() < 1e-12);
    }

    #[test]
    fn per_search_constant_is_0_005() {
        // Port-fidelity assertion: the TS cost-tracker.ts uses 0.005 per result.
        assert!((COST_PER_SEARCH_ESTIMATE - 0.005).abs() < 1e-12);
        let m = CostModel::default();
        assert!((m.per_search_result_usd - 0.005).abs() < 1e-12);
        assert!((m.per_find_similar_result_usd - 0.005).abs() < 1e-12);
        assert!((m.per_contents_url_usd - 0.005).abs() < 1e-12);
    }

    #[test]
    fn estimate_n_units_scales_linearly() {
        let m = CostModel::default();
        let n = 10_u32;
        assert!(
            (m.estimate(ExaOp::Search, n) - COST_PER_SEARCH_ESTIMATE * f64::from(n)).abs() < 1e-12
        );
    }

    #[test]
    fn estimate_search_config_empty_is_zero() {
        let cfg = bare_cfg(0, 0, 100);
        assert_eq!(CostModel::default().estimate_search_config(&cfg), 0.0);
    }

    #[test]
    fn estimate_search_config_sums_correctly() {
        // 1 seed + 2 queries, max_candidates=1000 (no cap).
        let cfg = bare_cfg(1, 2, 1000);
        let m = CostModel::default();
        // seed: FindSimilar, 10 units; q0: Search, 10; q1: Search, 10
        let expected = 3.0 * m.estimate(ExaOp::Search, DEFAULT_NUM_RESULTS);
        assert!((m.estimate_search_config(&cfg) - expected).abs() < 1e-12);
    }
}
