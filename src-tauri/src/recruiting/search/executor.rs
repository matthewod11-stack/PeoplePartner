//! `TieredSearchExecutor` — the core discovery run loop.
//!
//! Runs seeds first (P0), then tiers in ascending priority order (P1..Pn),
//! faithfully porting the TS `ExaAdapter.search` generator.  Cost gating is
//! threaded through `CostGate` so Task 5's `CostTracker` slots in without
//! touching the executor signature.
//!
//! Error handling:
//! - Per-query non-fatal errors (429/5xx) are swallowed and counted in
//!   `stats.query_failures`, mirroring the TS empty-page-and-continue.
//! - `InvalidKey` is fatal → abort the entire run.
//! - `CostGate::check_before` returning `false` → `stopped_early = true`,
//!   halt (soft partial success).

use crate::recruiting::adapters::exa::ExaError;
use crate::recruiting::intake::schemas::SearchConfig;
use crate::recruiting::search::candidate::{parse_hit, CandidateProvenance, DiscoveryVia, RawCandidate};
use crate::recruiting::search::source::{CandidateSource, SearchError};
use serde::{Deserialize, Serialize};

const DEFAULT_NUM_RESULTS: u32 = 10;

// ---------------------------------------------------------------------------
// CostGate seam
// ---------------------------------------------------------------------------

/// Operation kinds the executor bills against the cost gate.
///
/// Designed so Task 5 can `impl CostGate for CostTracker` with full pricing
/// logic without changing the executor signature at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExaOpKind {
    Search,
    FindSimilar,
    // wired in Task 5/6 (enrichment cost)
    #[allow(dead_code)]
    GetContents,
}

/// Cost-gate seam threaded through the run loop.
///
/// `check_before(op, units)` — returns `true` to allow the call, `false` to
/// halt (`stopped_early = true`).  The executor does NOT fire the Exa call
/// when `false` is returned.
///
/// `record(op, units, actual)` — called *after* a permitted call completes,
/// so the tracker can reconcile actual `costDollars` against its estimate.
/// `actual` is `Some(usd)` when Exa returned `costDollars.total`; `None`
/// means the estimate should be used.
///
/// The no-op impl (`NoopCostGate`) always allows and never charges, which is
/// the correct behavior for Task 3 (cost tracking arrives in Task 5).
pub trait CostGate {
    fn check_before(&mut self, op: ExaOpKind, units: u32) -> bool;
    fn record(&mut self, op: ExaOpKind, units: u32, actual: Option<f64>);
}

/// No-cost-accounting implementation. Always permits, never charges.
/// Used in all Task 3 tests and as the default until Task 5 provides
/// `CostTracker`.
pub struct NoopCostGate;
impl CostGate for NoopCostGate {
    fn check_before(&mut self, _op: ExaOpKind, _units: u32) -> bool {
        true
    }
    fn record(&mut self, _op: ExaOpKind, _units: u32, _actual: Option<f64>) {}
}

// ---------------------------------------------------------------------------
// DiscoveryStats + RawSearchResult
// ---------------------------------------------------------------------------

/// Counters returned alongside the raw candidate list.
///
/// `hits_before_dedup`, `hits_after_filter`, `returned`, and
/// `skipped_filter_kinds` are filled in by `DiscoveryPipeline::run` after
/// anti-filters and dedup; the executor fills the rest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryStats {
    /// Number of `find_similar` calls made.
    pub seed_calls: u32,
    /// Number of `search` calls made (successful or not).
    pub query_calls: u32,
    /// Total hits before dedup (filled by pipeline).
    pub hits_before_dedup: u32,
    /// Total hits after anti-filters (filled by pipeline).
    pub hits_after_filter: u32,
    /// Final candidate count returned (filled by pipeline).
    pub returned: u32,
    /// Number of search (or seed) calls that returned an error.
    pub query_failures: u32,
    /// True when the run was cut short by `max_candidates`.
    pub capped: bool,
    /// True when the run was halted early by the cost gate.
    pub stopped_early: bool,
    /// Anti-filter kinds that could not be evaluated at discovery time
    /// (filled by pipeline).
    pub skipped_filter_kinds: Vec<String>,
}

/// Raw output of `TieredSearchExecutor::run` before anti-filters and dedup.
pub struct RawSearchResult {
    pub candidates: Vec<RawCandidate>,
    pub stats: DiscoveryStats,
}

// ---------------------------------------------------------------------------
// TieredSearchExecutor
// ---------------------------------------------------------------------------

/// Runs the tiered discovery loop: seeds first (P0), then tiers ascending.
pub struct TieredSearchExecutor<S: CandidateSource> {
    pub source: S,
}

impl<S: CandidateSource> TieredSearchExecutor<S> {
    pub fn new(source: S) -> Self {
        Self { source }
    }

    /// Execute discovery: seeds → tiers, cost-gated, with early exit on
    /// `max_candidates` or `CostGate::check_before` returning `false`.
    pub async fn run(
        &self,
        cfg: &SearchConfig,
        cost: &mut dyn CostGate,
    ) -> Result<RawSearchResult, SearchError> {
        let mut candidates: Vec<RawCandidate> = Vec::new();
        let mut stats = DiscoveryStats::default();
        let max_candidates = cfg.max_candidates as usize;

        // ---------------------------------------------------------------
        // P0: Similarity seeds
        // ---------------------------------------------------------------
        for seed_url in &cfg.similarity_seeds {
            // Stop running seeds once the cap is reached (TS exa-adapter.ts:92).
            if candidates.len() >= max_candidates {
                stats.capped = true;
                break;
            }

            // `DEFAULT_NUM_RESULTS` is a pre-call *estimate* of the units this
            // call will bill; the actual hit count flows to `record` after.
            if !cost.check_before(ExaOpKind::FindSimilar, DEFAULT_NUM_RESULTS) {
                stats.stopped_early = true;
                break;
            }

            stats.seed_calls += 1;
            match self.source.find_similar(seed_url).await {
                Ok((hits, actual_cost)) => {
                    let count = hits.len() as u32;
                    cost.record(ExaOpKind::FindSimilar, count, actual_cost);
                    for hit in hits {
                        let prov = CandidateProvenance {
                            via: DiscoveryVia::SimilaritySeed,
                            query_text: None,
                            seed_url: Some(seed_url.clone()),
                            tier_priority: None,
                        };
                        candidates.push(parse_hit(hit, prov));
                    }
                }
                Err(SearchError::Source(ExaError::InvalidKey)) => {
                    return Err(SearchError::Source(ExaError::InvalidKey));
                }
                Err(_) => {
                    stats.query_failures += 1;
                }
            }
        }

        // ---------------------------------------------------------------
        // P1..Pn: Tiers in ascending priority order
        // ---------------------------------------------------------------
        let mut sorted_tiers = cfg.tiers.clone();
        sorted_tiers.sort_by_key(|t| t.priority);

        'outer: for tier in &sorted_tiers {
            for query in &tier.queries {
                let remaining = max_candidates.saturating_sub(candidates.len());
                if remaining == 0 {
                    stats.capped = true;
                    break 'outer;
                }

                let num_results = query
                    .max_results
                    .unwrap_or(DEFAULT_NUM_RESULTS)
                    .min(remaining as u32);

                if !cost.check_before(ExaOpKind::Search, num_results) {
                    stats.stopped_early = true;
                    break 'outer;
                }

                stats.query_calls += 1;
                match self.source.search(query, num_results).await {
                    Ok((hits, actual_cost)) => {
                        let count = hits.len() as u32;
                        cost.record(ExaOpKind::Search, count, actual_cost);
                        for hit in hits {
                            let prov = CandidateProvenance {
                                via: DiscoveryVia::TierQuery,
                                query_text: Some(query.text.clone()),
                                seed_url: None,
                                tier_priority: Some(tier.priority),
                            };
                            candidates.push(parse_hit(hit, prov));
                        }
                    }
                    Err(SearchError::Source(ExaError::InvalidKey)) => {
                        return Err(SearchError::Source(ExaError::InvalidKey));
                    }
                    Err(_) => {
                        stats.query_failures += 1;
                        // Non-fatal: continue to next query (TS empty-page-and-continue)
                    }
                }
            }
        }

        Ok(RawSearchResult { candidates, stats })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recruiting::adapters::exa::ExaHit;
    use crate::recruiting::intake::schemas::{
        SearchQuery, SearchQueryTier, TierThresholds,
    };
    use crate::recruiting::search::source::FakeCandidateSource;

    // --- helpers ------------------------------------------------------------

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

    fn query(text: &str) -> SearchQuery {
        SearchQuery {
            text: text.into(),
            target_companies: None,
            include_domains: None,
            exclude_domains: None,
            max_results: None,
        }
    }

    fn cfg_with(
        seeds: Vec<String>,
        tiers: Vec<(u8, &str)>,
        max_candidates: u32,
    ) -> SearchConfig {
        SearchConfig {
            role_name: "test".into(),
            tiers: tiers
                .into_iter()
                .map(|(priority, text)| SearchQueryTier {
                    priority,
                    queries: vec![query(text)],
                })
                .collect(),
            scoring_weights: Default::default(),
            tier_thresholds: TierThresholds {
                tier1_min_score: 70,
                tier2_min_score: 50,
            },
            enrichment_priority: vec![],
            anti_filters: vec![],
            similarity_seeds: seeds,
            max_candidates,
            max_cost_usd: 10.0,
            created_at: "2026-06-03T00:00:00Z".into(),
            version: 1,
        }
    }

    // --- tests --------------------------------------------------------------

    #[tokio::test]
    async fn seeds_run_before_tiers_in_priority_order() {
        let mut fake = FakeCandidateSource::empty();
        fake.similar_results
            .insert("https://seed".into(), vec![hit("https://a")]);
        fake.search_results
            .insert("q-hi".into(), vec![hit("https://b")]);
        fake.search_results
            .insert("q-lo".into(), vec![hit("https://c")]);

        // tiers deliberately given out of order (priority 2 first)
        let cfg = cfg_with(
            vec!["https://seed".into()],
            vec![(2, "q-lo"), (1, "q-hi")],
            100,
        );

        let exec = TieredSearchExecutor::new(fake);
        let res = exec.run(&cfg, &mut NoopCostGate).await.unwrap();

        let calls = exec.source.calls.lock().unwrap().clone();
        assert_eq!(
            calls,
            vec!["similar:https://seed", "search:q-hi", "search:q-lo"]
        );
        assert_eq!(res.candidates.len(), 3);
    }

    #[tokio::test]
    async fn invalid_key_aborts_but_other_query_errors_are_swallowed() {
        let mut fake = FakeCandidateSource::empty();
        fake.fail_query = Some("boom".into());
        fake.search_results
            .insert("ok".into(), vec![hit("https://a")]);

        let cfg = cfg_with(vec![], vec![(1, "boom"), (1, "ok")], 100);
        let res = TieredSearchExecutor::new(fake)
            .run(&cfg, &mut NoopCostGate)
            .await
            .unwrap();

        assert_eq!(res.stats.query_failures, 1);
        assert_eq!(res.candidates.len(), 1);
    }

    #[tokio::test]
    async fn invalid_key_on_search_aborts_run() {
        let mut fake = FakeCandidateSource::empty();
        fake.fail_with_invalid_key = true;
        fake.search_results
            .insert("q".into(), vec![hit("https://a")]);

        let cfg = cfg_with(vec![], vec![(1, "q")], 100);
        let result = TieredSearchExecutor::new(fake)
            .run(&cfg, &mut NoopCostGate)
            .await;

        assert!(
            matches!(result, Err(SearchError::Source(ExaError::InvalidKey))),
            "InvalidKey must abort the run"
        );
    }

    #[tokio::test]
    async fn max_candidates_cap_stops_later_queries() {
        let mut fake = FakeCandidateSource::empty();
        // Each query returns 2 hits
        fake.search_results
            .insert("q1".into(), vec![hit("https://a"), hit("https://b")]);
        // q2 should never be called
        fake.search_results
            .insert("q2".into(), vec![hit("https://c"), hit("https://d")]);

        let cfg = cfg_with(vec![], vec![(1, "q1"), (1, "q2")], 2);
        let exec = TieredSearchExecutor::new(fake);
        let res = exec.run(&cfg, &mut NoopCostGate).await.unwrap();

        let calls = exec.source.calls.lock().unwrap().clone();
        // q2 should not be called because max_candidates was reached after q1
        assert_eq!(calls, vec!["search:q1"], "q2 must not be called");
        assert_eq!(res.candidates.len(), 2);
        assert!(res.stats.capped);
    }

    #[tokio::test]
    async fn max_candidates_cap_stops_later_seeds() {
        let mut fake = FakeCandidateSource::empty();
        // First seed returns 2 hits, hitting the cap of 2.
        fake.similar_results
            .insert("https://s1".into(), vec![hit("https://a"), hit("https://b")]);
        // s2 should never be called.
        fake.similar_results
            .insert("https://s2".into(), vec![hit("https://c")]);

        let cfg = cfg_with(
            vec!["https://s1".into(), "https://s2".into()],
            vec![],
            2,
        );
        let exec = TieredSearchExecutor::new(fake);
        let res = exec.run(&cfg, &mut NoopCostGate).await.unwrap();

        let calls = exec.source.calls.lock().unwrap().clone();
        assert_eq!(
            calls,
            vec!["similar:https://s1"],
            "second seed must not be called once max_candidates is reached"
        );
        assert_eq!(res.candidates.len(), 2);
        assert!(res.stats.capped);
    }

    #[tokio::test]
    async fn empty_config_returns_empty_no_panic() {
        let fake = FakeCandidateSource::empty();
        let cfg = cfg_with(vec![], vec![], 100);
        let res = TieredSearchExecutor::new(fake)
            .run(&cfg, &mut NoopCostGate)
            .await
            .unwrap();
        assert_eq!(res.candidates.len(), 0);
        assert_eq!(res.stats.seed_calls, 0);
        assert_eq!(res.stats.query_calls, 0);
    }

    #[tokio::test]
    async fn cost_gate_false_sets_stopped_early_and_halts() {
        struct AlwaysStop;
        impl CostGate for AlwaysStop {
            fn check_before(&mut self, _op: ExaOpKind, _units: u32) -> bool {
                false
            }
            fn record(&mut self, _op: ExaOpKind, _units: u32, _actual: Option<f64>) {}
        }

        let mut fake = FakeCandidateSource::empty();
        fake.search_results
            .insert("q".into(), vec![hit("https://a")]);
        let cfg = cfg_with(vec![], vec![(1, "q")], 100);

        let res = TieredSearchExecutor::new(fake)
            .run(&cfg, &mut AlwaysStop)
            .await
            .unwrap();

        assert!(res.stats.stopped_early);
        assert_eq!(res.candidates.len(), 0);
    }

    #[tokio::test]
    async fn seed_and_tier_provenance_tagged_correctly() {
        let mut fake = FakeCandidateSource::empty();
        fake.similar_results
            .insert("https://seed".into(), vec![hit("https://a")]);
        fake.search_results
            .insert("q".into(), vec![hit("https://b")]);

        let cfg = cfg_with(vec!["https://seed".into()], vec![(1, "q")], 100);
        let exec = TieredSearchExecutor::new(fake);
        let res = exec.run(&cfg, &mut NoopCostGate).await.unwrap();

        let seed_cand = &res.candidates[0];
        assert_eq!(seed_cand.provenance.via, DiscoveryVia::SimilaritySeed);
        assert_eq!(
            seed_cand.provenance.seed_url.as_deref(),
            Some("https://seed")
        );
        assert!(seed_cand.provenance.query_text.is_none());

        let tier_cand = &res.candidates[1];
        assert_eq!(tier_cand.provenance.via, DiscoveryVia::TierQuery);
        assert_eq!(tier_cand.provenance.query_text.as_deref(), Some("q"));
        assert_eq!(tier_cand.provenance.tier_priority, Some(1));
    }

    #[tokio::test]
    async fn tiers_sorted_ascending_even_when_given_descending() {
        let mut fake = FakeCandidateSource::empty();
        fake.search_results
            .insert("low-prio".into(), vec![hit("https://a")]);
        fake.search_results
            .insert("high-prio".into(), vec![hit("https://b")]);

        // Tiers given in descending order
        let cfg = cfg_with(vec![], vec![(3, "low-prio"), (1, "high-prio")], 100);
        let exec = TieredSearchExecutor::new(fake);
        exec.run(&cfg, &mut NoopCostGate).await.unwrap();

        let calls = exec.source.calls.lock().unwrap().clone();
        assert_eq!(calls[0], "search:high-prio", "priority 1 must fire first");
        assert_eq!(calls[1], "search:low-prio");
    }

    #[tokio::test]
    async fn seed_invalid_key_aborts() {
        let mut fake = FakeCandidateSource::empty();
        fake.fail_with_invalid_key = true;

        let cfg = cfg_with(vec!["https://seed".into()], vec![], 100);
        let result = TieredSearchExecutor::new(fake)
            .run(&cfg, &mut NoopCostGate)
            .await;

        assert!(matches!(
            result,
            Err(SearchError::Source(ExaError::InvalidKey))
        ));
    }

    #[tokio::test]
    async fn stats_seed_calls_and_query_calls_counted() {
        let mut fake = FakeCandidateSource::empty();
        fake.similar_results
            .insert("https://s1".into(), vec![hit("https://a")]);
        fake.similar_results
            .insert("https://s2".into(), vec![hit("https://b")]);
        fake.search_results
            .insert("q".into(), vec![hit("https://c")]);

        let cfg = cfg_with(
            vec!["https://s1".into(), "https://s2".into()],
            vec![(1, "q")],
            100,
        );
        let res = TieredSearchExecutor::new(fake)
            .run(&cfg, &mut NoopCostGate)
            .await
            .unwrap();

        assert_eq!(res.stats.seed_calls, 2);
        assert_eq!(res.stats.query_calls, 1);
    }
}
