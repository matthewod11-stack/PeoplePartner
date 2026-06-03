//! `CostTracker` — runtime spend accumulator + pre-call gate.
//!
//! Implements `CostGate` from `recruiting::search::executor` so it slots in
//! without touching the executor run loop.
//!
//! # Pre-call gate semantics (spec §12)
//!
//! `check_before(op, units)` estimates the *next* call and projects the
//! running total.  If `projected > budget` (using cent-rounded comparison to
//! avoid f64 drift) → `Stop { projected_total_usd, budget_usd }` and
//! `total_usd` is **not changed** (the call never fires, so it must not be
//! charged).  Otherwise → `Proceed { estimated_usd }`.
//!
//! `record(op, units, actual)` is called *after* a permitted call.  It prefers
//! `actual` (Exa's real `costDollars.total`) when `Some`, else falls back to
//! the model's estimate for `units`.
//!
//! A call that lands `total == budget` IS allowed; the next non-zero call Stops.
//!
//! # Port note (divergence from TS)
//!
//! The TS `CostTracker` is post-hoc only (`exceedsBudget` checked after
//! accumulation).  This implementation is hybrid: the pre-call gate prevents
//! overspend without the TS behaviour of throwing "Budget exceeded" after the
//! fact.  Documented for port-fidelity reviewers.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::recruiting::search::executor::{CostGate, ExaOpKind};

use super::pricing::{CostModel, ExaOp};

// ---------------------------------------------------------------------------
// CostDecision
// ---------------------------------------------------------------------------

/// Result of `CostTracker::check_before`.
#[derive(Debug, Clone, PartialEq)]
pub enum CostDecision {
    /// Call is permitted; `estimated_usd` is the estimated cost for this call.
    Proceed { estimated_usd: f64 },
    /// Budget would be exceeded; call must **not** fire.  `total_usd` is
    /// unchanged.
    Stop {
        projected_total_usd: f64,
        budget_usd: f64,
    },
}

// ---------------------------------------------------------------------------
// CostSnapshot / CostReport
// ---------------------------------------------------------------------------

/// Point-in-time cost state — used for `snapshot`/`restore_from` and the
/// final `CostReport`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CostSnapshot {
    pub total_usd: f64,
    pub per_op: BTreeMap<String, f64>,
    pub call_count: u32,
    /// Always "USD" — PII-free by construction (no raw data, just dollars).
    pub currency: String,
}

/// Summary attached to `RunSearchResult` for the frontend.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CostReport {
    pub snapshot: CostSnapshot,
    pub stopped_early: bool,
    pub stop_reason: Option<String>,
}

// ---------------------------------------------------------------------------
// CostTracker
// ---------------------------------------------------------------------------

/// Accumulates spend per `ExaOp` and gates calls against a budget ceiling.
///
/// **Thread safety:** designed for single-threaded use within a single
/// `DiscoveryPipeline::run` call. `CostGate` takes `&mut self` so the borrow
/// checker enforces exclusive access.
pub struct CostTracker {
    model: CostModel,
    max_cost_usd: f64,
    total_usd: f64,
    per_op: BTreeMap<&'static str, f64>,
    call_count: u32,
}

impl CostTracker {
    /// Create a new tracker with the given pricing model and budget.
    pub fn new(model: CostModel, max_cost_usd: f64) -> Self {
        Self {
            model,
            max_cost_usd,
            total_usd: 0.0,
            per_op: BTreeMap::new(),
            call_count: 0,
        }
    }

    /// Current accumulated spend in USD.
    pub fn total_usd(&self) -> f64 {
        self.total_usd
    }

    /// Estimate whether the next call would exceed the budget.
    ///
    /// Does **not** mutate state — a `Stop` return leaves `total_usd` unchanged.
    /// Uses cent-rounded comparison (`(projected * 1e9).round()`) to avoid f64
    /// drift at boundary values.
    pub fn check_before(&self, op: ExaOp, units: u32) -> CostDecision {
        let estimated = self.model.estimate(op, units);
        let projected = self.total_usd + estimated;
        // Round to nanodollar precision to dodge f64 drift.
        if (projected * 1_000_000_000.0).round() > (self.max_cost_usd * 1_000_000_000.0).round() {
            CostDecision::Stop {
                projected_total_usd: projected,
                budget_usd: self.max_cost_usd,
            }
        } else {
            CostDecision::Proceed {
                estimated_usd: estimated,
            }
        }
    }

    /// Record a completed call.
    ///
    /// Prefers `actual` (Exa's reported `costDollars.total`) over the model
    /// estimate.  Records 0 when `units == 0`.
    pub fn record(&mut self, op: ExaOp, units: u32, actual: Option<f64>) {
        let cost = actual.unwrap_or_else(|| self.model.estimate(op, units));
        self.total_usd += cost;
        *self.per_op.entry(op.label()).or_insert(0.0) += cost;
        self.call_count += 1;
    }

    /// Capture a point-in-time snapshot (for `report()` and future resume).
    pub fn snapshot(&self) -> CostSnapshot {
        CostSnapshot {
            total_usd: self.total_usd,
            per_op: self
                .per_op
                .iter()
                .map(|(&k, &v)| (k.to_owned(), v))
                .collect(),
            call_count: self.call_count,
            currency: "USD".to_owned(),
        }
    }

    /// Build a `CostReport` to attach to `RunSearchResult`.
    pub fn report(&self, stopped_early: bool, stop_reason: Option<String>) -> CostReport {
        CostReport {
            snapshot: self.snapshot(),
            stopped_early,
            stop_reason,
        }
    }

    /// Restore state from a snapshot (wired for future checkpoint/resume —
    /// unused in S1.1 but included per spec §12).
    pub fn restore_from(&mut self, snap: &CostSnapshot) {
        self.total_usd = snap.total_usd;
        self.per_op = snap
            .per_op
            .iter()
            .filter_map(|(k, &v)| label_for_key(k).map(|label| (label, v)))
            .collect();
        self.call_count = snap.call_count;
    }
}

// ---------------------------------------------------------------------------
// CostGate bridge — impl CostGate for CostTracker
// ---------------------------------------------------------------------------

/// Maps `ExaOpKind` (executor seam) → `ExaOp` (cost/pricing).
fn map_op(kind: ExaOpKind) -> ExaOp {
    match kind {
        ExaOpKind::Search => ExaOp::Search,
        ExaOpKind::FindSimilar => ExaOp::FindSimilar,
        ExaOpKind::GetContents => ExaOp::GetContents,
    }
}

/// Reverse of `ExaOp::label` — maps a snapshot's string key back to the
/// canonical `&'static str` label, or `None` if the key isn't a known op.
///
/// Derives its table from `ExaOp::ALL` + `ExaOp::label`, both of which are
/// guarded against drift by the exhaustiveness assertion in `pricing.rs`. So
/// adding a future `ExaOp` variant forces a compile error there rather than
/// silently dropping that variant's snapshot data on restore.
fn label_for_key(key: &str) -> Option<&'static str> {
    ExaOp::ALL
        .into_iter()
        .map(ExaOp::label)
        .find(|&label| label == key)
}

impl CostGate for CostTracker {
    /// Returns `true` when the call may proceed (`Proceed`), `false` to halt
    /// (`Stop`).  The executor sets `stats.stopped_early = true` on `false`
    /// and returns partial results (ruled decision #2 — soft partial success).
    fn check_before(&mut self, op: ExaOpKind, units: u32) -> bool {
        // Call the inherent `CostTracker::check_before` (takes ExaOp),
        // not the trait method (takes ExaOpKind) — use the fully-qualified path
        // to avoid recursive dispatch into the trait impl.
        matches!(
            CostTracker::check_before(self, map_op(op), units),
            CostDecision::Proceed { .. }
        )
    }

    fn record(&mut self, op: ExaOpKind, units: u32, actual: Option<f64>) {
        CostTracker::record(self, map_op(op), units, actual);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recruiting::intake::schemas::{SearchQuery, SearchQueryTier, TierThresholds};
    use super::super::pricing::{CostModel, ExaOp, COST_PER_SEARCH_ESTIMATE};

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn tracker(budget: f64) -> CostTracker {
        CostTracker::new(CostModel::default(), budget)
    }

    fn sample_config() -> crate::recruiting::intake::schemas::SearchConfig {
        // 1 seed + 1 query, max_candidates = 1000 (no cap).
        // Both use DEFAULT_NUM_RESULTS=10, cost = 2 * 10 * 0.005 = 0.100
        crate::recruiting::intake::schemas::SearchConfig {
            role_name: "test".into(),
            tiers: vec![SearchQueryTier {
                priority: 1,
                queries: vec![SearchQuery {
                    text: "rust engineer".into(),
                    target_companies: None,
                    include_domains: None,
                    exclude_domains: None,
                    max_results: None, // DEFAULT_NUM_RESULTS = 10
                }],
            }],
            scoring_weights: Default::default(),
            tier_thresholds: TierThresholds {
                tier1_min_score: 70,
                tier2_min_score: 50,
            },
            enrichment_priority: vec![],
            anti_filters: vec![],
            similarity_seeds: vec!["https://seed.example.com".into()],
            max_candidates: 1000,
            max_cost_usd: 100.0,
            created_at: "2026-06-03T00:00:00Z".into(),
            version: 1,
        }
    }

    /// Count (op, units) pairs the executor would produce for `sample_config`:
    /// - 1 FindSimilar call with DEFAULT_NUM_RESULTS=10
    /// - 1 Search call with DEFAULT_NUM_RESULTS=10
    fn sample_config_calls() -> Vec<(ExaOp, u32)> {
        vec![
            (ExaOp::FindSimilar, 10),
            (ExaOp::Search, 10),
        ]
    }

    // -----------------------------------------------------------------------
    // Core gate tests (spec §15)
    // -----------------------------------------------------------------------

    #[test]
    fn check_before_stops_first_crossing_call_without_charging() {
        // Budget = 0.010 = 2 * 0.005 * 1 unit each.
        let mut t = CostTracker::new(CostModel::default(), 0.010);
        // First call: estimate 0.005, projected 0.005 <= 0.010 → Proceed
        assert!(
            matches!(t.check_before(ExaOp::Search, 1), CostDecision::Proceed { .. }),
            "first call must Proceed"
        );
        t.record(ExaOp::Search, 1, None);
        assert!((t.total_usd() - 0.005).abs() < 1e-9, "after first record");

        // Second call: estimate 0.005, projected 0.010 == budget → Proceed
        assert!(
            matches!(t.check_before(ExaOp::Search, 1), CostDecision::Proceed { .. }),
            "second call must Proceed (exact boundary)"
        );
        t.record(ExaOp::Search, 1, None);
        assert!((t.total_usd() - 0.010).abs() < 1e-9, "after second record");

        // Third call: estimate 0.005, projected 0.015 > budget → Stop
        let d = t.check_before(ExaOp::Search, 1);
        assert!(
            matches!(d, CostDecision::Stop { .. }),
            "third call would breach: {d:?}"
        );
        // CRITICAL: total_usd must NOT change after a Stop.
        assert!(
            (t.total_usd() - 0.010).abs() < 1e-9,
            "stopped call must not be charged; total={:.9}",
            t.total_usd()
        );
    }

    #[test]
    fn record_prefers_actual_over_estimate() {
        let mut t = tracker(100.0);
        // Estimate for 10 units would be 0.050; actual is 0.031.
        t.record(ExaOp::Search, 10, Some(0.031));
        assert!(
            (t.total_usd() - 0.031).abs() < 1e-9,
            "actual cost must be used; got {:.9}",
            t.total_usd()
        );
    }

    #[test]
    fn record_zero_units_records_zero_cost() {
        let mut t = tracker(100.0);
        t.record(ExaOp::Search, 0, None);
        assert_eq!(t.total_usd(), 0.0, "0 units must cost 0");
        assert_eq!(t.call_count, 1, "call_count still increments");
    }

    #[test]
    fn exact_boundary_is_allowed() {
        // Budget exactly equals what two calls will cost.
        let budget = 2.0 * COST_PER_SEARCH_ESTIMATE;
        let mut t = CostTracker::new(CostModel::default(), budget);
        // Call 1: proceeds, charges
        assert!(matches!(t.check_before(ExaOp::Search, 1), CostDecision::Proceed { .. }));
        t.record(ExaOp::Search, 1, None);
        // Call 2: projected == budget exactly → allowed
        assert!(
            matches!(t.check_before(ExaOp::Search, 1), CostDecision::Proceed { .. }),
            "projected == budget must be Proceed"
        );
        t.record(ExaOp::Search, 1, None);
        // Call 3: projected > budget → Stop
        assert!(
            matches!(t.check_before(ExaOp::Search, 1), CostDecision::Stop { .. }),
            "next call after exact fill must Stop"
        );
    }

    #[test]
    fn preview_equals_runtime_invariant() {
        // The estimate_search_config and per-call accumulation must produce
        // identical numbers when all calls use their estimates (actual=None).
        let cfg = sample_config();
        let model = CostModel::default();

        let preview = model.estimate_search_config(&cfg);

        let mut t = CostTracker::new(model, 1000.0);
        for (op, units) in sample_config_calls() {
            t.record(op, units, None);
        }

        assert!(
            (preview - t.total_usd()).abs() < 1e-9,
            "S1.4 preview must equal runtime accumulation: preview={preview:.9} runtime={:.9}",
            t.total_usd()
        );
    }

    #[test]
    fn snapshot_restore_round_trip() {
        let mut t = tracker(100.0);
        t.record(ExaOp::Search, 5, None);
        t.record(ExaOp::FindSimilar, 3, Some(0.012));

        let snap = t.snapshot();
        assert_eq!(snap.currency, "USD");

        let mut t2 = tracker(100.0);
        t2.restore_from(&snap);

        assert!(
            (t2.total_usd() - t.total_usd()).abs() < 1e-12,
            "restore must reproduce total"
        );
        assert_eq!(t2.call_count, t.call_count, "call_count must survive round-trip");
        assert_eq!(
            t2.snapshot().per_op,
            t.snapshot().per_op,
            "per_op must survive round-trip"
        );
    }

    #[test]
    fn serde_camel_case() {
        let snap = CostSnapshot {
            total_usd: 0.015,
            per_op: BTreeMap::from([("search".to_owned(), 0.015)]),
            call_count: 3,
            currency: "USD".to_owned(),
        };
        let v = serde_json::to_value(&snap).unwrap();
        assert!(v.get("totalUsd").is_some(), "totalUsd key must exist in JSON");
        assert!(v.get("perOp").is_some(), "perOp key must exist in JSON");
        assert!(v.get("callCount").is_some(), "callCount key must exist in JSON");
        assert_eq!(v["currency"], "USD");
    }

    #[test]
    fn cost_report_serde_camel_case() {
        let t = tracker(1.0);
        let report = t.report(true, Some("budget exceeded".into()));
        let v = serde_json::to_value(&report).unwrap();
        assert!(v.get("stoppedEarly").is_some());
        assert!(v.get("stopReason").is_some());
        assert!(v.get("snapshot").is_some());
    }

    #[test]
    fn per_op_accumulates_by_label() {
        let mut t = tracker(100.0);
        t.record(ExaOp::Search, 1, None);
        t.record(ExaOp::Search, 1, None);
        t.record(ExaOp::FindSimilar, 1, None);

        let snap = t.snapshot();
        let search_total = snap.per_op.get("search").copied().unwrap_or(0.0);
        let similar_total = snap.per_op.get("find_similar").copied().unwrap_or(0.0);
        assert!(
            (search_total - 2.0 * COST_PER_SEARCH_ESTIMATE).abs() < 1e-12,
            "search per_op must accumulate: {search_total}"
        );
        assert!(
            (similar_total - COST_PER_SEARCH_ESTIMATE).abs() < 1e-12,
            "find_similar per_op must accumulate: {similar_total}"
        );
    }

    // -----------------------------------------------------------------------
    // CostGate bridge test (integration with the executor seam)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn cost_gate_bridge_halts_mid_tier_and_returns_partial() {
        use crate::recruiting::adapters::exa::ExaHit;
        use crate::recruiting::intake::schemas::{SearchQuery, SearchQueryTier, TierThresholds};
        use crate::recruiting::search::executor::TieredSearchExecutor;
        use crate::recruiting::search::source::FakeCandidateSource;

        fn hit(url: &str) -> ExaHit {
            ExaHit {
                id: url.into(),
                url: url.into(),
                title: None,
                score: None,
                published_date: None,
                author: None,
                text: None,
                highlights: None,
                summary: None,
            }
        }

        // Budget = 0.005 → exactly one Search call at 1 unit.
        // Two queries: q1 returns 1 hit (charged 0.005), q2 would exceed budget.
        let budget = COST_PER_SEARCH_ESTIMATE; // 0.005
        let mut tracker = CostTracker::new(CostModel::default(), budget);

        let mut fake = FakeCandidateSource::empty();
        fake.search_results
            .insert("q1".into(), vec![hit("https://a.io")]);
        fake.search_results
            .insert("q2".into(), vec![hit("https://b.io")]);

        let cfg = crate::recruiting::intake::schemas::SearchConfig {
            role_name: "test".into(),
            tiers: vec![SearchQueryTier {
                priority: 1,
                queries: vec![
                    SearchQuery {
                        text: "q1".into(),
                        target_companies: None,
                        include_domains: None,
                        exclude_domains: None,
                        max_results: Some(1), // 1 unit → costs exactly budget
                    },
                    SearchQuery {
                        text: "q2".into(),
                        target_companies: None,
                        include_domains: None,
                        exclude_domains: None,
                        max_results: Some(1), // would cost 0.005 more → over budget
                    },
                ],
            }],
            scoring_weights: Default::default(),
            tier_thresholds: TierThresholds {
                tier1_min_score: 70,
                tier2_min_score: 50,
            },
            enrichment_priority: vec![],
            anti_filters: vec![],
            similarity_seeds: vec![],
            max_candidates: 100,
            max_cost_usd: budget,
            created_at: "2026-06-03T00:00:00Z".into(),
            version: 1,
        };

        let exec = TieredSearchExecutor::new(fake);
        let res = exec.run(&cfg, &mut tracker).await.unwrap();

        // Ruled decision #2 — soft partial success: partial candidates returned.
        assert!(res.stats.stopped_early, "must stop early when budget exceeded");
        assert_eq!(
            res.candidates.len(),
            1,
            "partial results (q1's hit) must be returned"
        );
        // The stopped call must not have been charged.
        assert!(
            (tracker.total_usd() - budget).abs() < 1e-9,
            "total must equal exactly budget; got {:.9}",
            tracker.total_usd()
        );
    }
}
