// Re-exports and the `DiscoveryPipeline` entrypoint are consumed by Task 4+
// (identity resolution) and the `recruiting_run_search` command wiring, neither
// of which exists yet; suppress dead-code + unused-re-export warnings
// module-wide until then (matching the established `identity/mod.rs` pattern).
#![allow(dead_code, unused_imports)]

//! Candidate discovery spine for the Sourcerer module (S1.1, FHR-73).
//!
//! Orchestrates `seeds → tiers → anti-filters → URL dedup → truncate` into
//! `DiscoveryPipeline::run`, which returns a `Vec<RawCandidate>` + stats.
//!
//! # Module layout
//! - `source`    — `CandidateSource` trait + prod `ExaCandidateSource` + test fake
//! - `candidate` — `RawCandidate`, `parse_hit`, `dedup_by_url`
//! - `executor`  — `TieredSearchExecutor` run loop + `CostGate` seam
//! - `filters`   — `apply_anti_filters`

pub mod candidate;
pub mod enrich;
pub mod executor;
pub mod filters;
pub mod source;

// Re-exports for consumers that go through `recruiting::search::*`.
pub use candidate::{CandidateProvenance, DiscoveryVia, RawCandidate};
pub use enrich::{
    enrich_candidates, exa_enrichment_priority, should_run_exa_enrichment, CandidateFailure,
    ContentEnricher, EnrichConfig, EnrichError, EnrichmentOutcome, ExaContentEnricher, FetchedPage,
};
pub use executor::{CostGate, DiscoveryStats, ExaOpKind, NoopCostGate, RawSearchResult, TieredSearchExecutor};
pub use filters::apply_anti_filters;
pub use source::{CandidateSource, ExaCandidateSource, SearchError};

use crate::recruiting::intake::schemas::SearchConfig;

// ---------------------------------------------------------------------------
// DiscoveryPipeline — top-level orchestration
// ---------------------------------------------------------------------------

/// Orchestrates the full discovery funnel:
/// 1. `executor.run` → raw hit list
/// 2. `apply_anti_filters` → remove evaluable filter matches
/// 3. `dedup_by_url` → collapse same-URL duplicates
/// 4. Truncate to `max_candidates`
/// 5. Fill in stats fields that need the post-filter counts.
pub struct DiscoveryPipeline;

impl DiscoveryPipeline {
    /// Run the discovery funnel and return the final candidate list + stats.
    pub async fn run<S: CandidateSource>(
        source: S,
        cfg: &SearchConfig,
        cost: &mut dyn CostGate,
    ) -> Result<(Vec<RawCandidate>, DiscoveryStats), SearchError> {
        let exec = TieredSearchExecutor::new(source);
        let mut result = exec.run(cfg, cost).await?;

        let hits_before_dedup = result.candidates.len() as u32;

        // Anti-filters
        let (filtered_candidates, skipped_kinds) =
            apply_anti_filters(result.candidates, &cfg.anti_filters);
        let hits_after_filter = filtered_candidates.len() as u32;

        // URL dedup
        let deduped = candidate::dedup_by_url(filtered_candidates);

        // Truncate
        let max = cfg.max_candidates as usize;
        let capped = deduped.len() > max;
        let final_candidates: Vec<RawCandidate> = deduped.into_iter().take(max).collect();
        let returned = final_candidates.len() as u32;

        // Patch stats
        result.stats.hits_before_dedup = hits_before_dedup;
        result.stats.hits_after_filter = hits_after_filter;
        result.stats.returned = returned;
        if capped {
            result.stats.capped = true;
        }
        result.stats.skipped_filter_kinds = skipped_kinds;

        Ok((final_candidates, result.stats))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recruiting::adapters::exa::ExaHit;
    use crate::recruiting::intake::schemas::{AntiFilter, SearchQuery, SearchQueryTier};
    use crate::recruiting::search::source::FakeCandidateSource;

    fn hit(url: &str) -> ExaHit {
        ExaHit {
            id: url.into(),
            url: url.into(),
            title: None,
            score: Some(0.8),
            published_date: None,
            author: None,
            text: None,
            highlights: None,
            summary: None,
        }
    }

    fn empty_cfg(max_candidates: u32) -> SearchConfig {
        SearchConfig {
            role_name: "test".into(),
            tiers: vec![],
            scoring_weights: Default::default(),
            tier_thresholds: crate::recruiting::intake::schemas::TierThresholds {
                tier1_min_score: 70,
                tier2_min_score: 50,
            },
            enrichment_priority: vec![],
            anti_filters: vec![],
            similarity_seeds: vec![],
            max_candidates,
            max_cost_usd: 10.0,
            created_at: "2026-06-03T00:00:00Z".into(),
            version: 1,
        }
    }

    #[tokio::test]
    async fn pipeline_end_to_end_funnel_counts() {
        let mut fake = FakeCandidateSource::empty();
        fake.search_results
            .insert("q".into(), vec![hit("https://a.com/1"), hit("https://A.com/1/"), hit("https://a.com/2")]);

        let mut cfg = empty_cfg(10);
        cfg.tiers = vec![SearchQueryTier {
            priority: 1,
            queries: vec![SearchQuery {
                text: "q".into(),
                target_companies: None,
                include_domains: None,
                exclude_domains: None,
                max_results: None,
            }],
        }];

        let (candidates, stats) = DiscoveryPipeline::run(fake, &cfg, &mut NoopCostGate)
            .await
            .unwrap();

        // 3 hits in, 2 after dedup (https://a.com/1 == https://A.com/1/)
        assert_eq!(stats.hits_before_dedup, 3);
        assert_eq!(candidates.len(), 2);
        assert_eq!(stats.returned, 2);
        assert!(!stats.capped);
    }

    #[tokio::test]
    async fn pipeline_applies_anti_filters_before_dedup() {
        let mut fake = FakeCandidateSource::empty();
        fake.search_results.insert(
            "q".into(),
            vec![
                // This hit will be filtered (acme in host)
                ExaHit {
                    id: "x".into(),
                    url: "https://acme.com/alice".into(),
                    title: None,
                    score: Some(0.9),
                    published_date: None,
                    author: None,
                    text: None,
                    highlights: None,
                    summary: None,
                },
                hit("https://other.com/bob"),
            ],
        );

        let mut cfg = empty_cfg(10);
        cfg.tiers = vec![SearchQueryTier {
            priority: 1,
            queries: vec![SearchQuery {
                text: "q".into(),
                target_companies: None,
                include_domains: None,
                exclude_domains: None,
                max_results: None,
            }],
        }];
        cfg.anti_filters = vec![AntiFilter {
            kind: "exclude_company".into(),
            value: "acme".into(),
            reason: "test".into(),
        }];

        let (candidates, stats) = DiscoveryPipeline::run(fake, &cfg, &mut NoopCostGate)
            .await
            .unwrap();

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].url, "https://other.com/bob");
        assert_eq!(stats.hits_after_filter, 1);
    }

    #[tokio::test]
    async fn pipeline_truncates_to_max_candidates() {
        let mut fake = FakeCandidateSource::empty();
        fake.search_results.insert(
            "q".into(),
            vec![
                hit("https://a.com/1"),
                hit("https://a.com/2"),
                hit("https://a.com/3"),
            ],
        );
        let mut cfg = empty_cfg(2);
        cfg.tiers = vec![SearchQueryTier {
            priority: 1,
            queries: vec![SearchQuery {
                text: "q".into(),
                target_companies: None,
                include_domains: None,
                exclude_domains: None,
                max_results: None,
            }],
        }];

        let (candidates, stats) = DiscoveryPipeline::run(fake, &cfg, &mut NoopCostGate)
            .await
            .unwrap();

        // The fake ignores `num_results`, so the single query returns all 3
        // distinct hits — `hits_before_dedup` is 3. Truncation to
        // `max_candidates = 2` happens in the pipeline (post-dedup), not in
        // the executor, which is what this test pins down.
        assert_eq!(stats.hits_before_dedup, 3);
        assert_eq!(candidates.len(), 2);
        assert_eq!(stats.returned, 2);
    }
}
