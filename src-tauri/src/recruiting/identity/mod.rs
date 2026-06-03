// The `resolve()` entrypoint and resolution types are consumed by Task 7's
// `recruiting_run_search` command, which isn't wired yet; suppress dead-code +
// unused-re-export warnings module-wide until then (matching the established
// `search/mod.rs` pattern).
#![allow(dead_code, unused_imports)]

//! Identity foundation + 4-pass resolver for the Sourcerer module (FHR-73 S1.1).
//!
//! Submodule structure:
//! - `types`     — `IdentifierType`, `ConfidenceLevel`, `ObservedIdentifier`,
//!   plus the Task 4 resolution surface (`PersonIdentity`, `ResolvedCandidate`,
//!   `SourceData`, merge types, `ResolveResult`/`ResolveStats`/`IdentityError`)
//! - `normalize` — per-type normalization functions + dispatcher
//! - `matching`  — `names_match`, `levenshtein`, `names_similar`, `split_name_company`
//! - `extract`   — `extract_identifiers`, `extract_name` (used by Task 3 discovery)
//! - `arbiter`   — `IdentityArbiter` LLM tiebreak seam + `NoopArbiter` default
//! - `resolver`  — the 4-pass `resolve()` entrypoint (absorbs FHR-74)

pub mod arbiter;
pub mod extract;
pub mod matching;
pub mod normalize;
pub mod resolver;
pub mod types;

// The resolver entrypoint. `resolve(raw, &NoopArbiter)` is the pure-heuristic
// path Task 7's command wires up; a real `IdentityArbiter` can veto Pass-4
// auto-merges. Re-exported here so callers say `identity::resolve(...)`.
pub use resolver::resolve;

#[cfg(test)]
mod resolve_integration_tests {
    //! Proves the Task 3 `DiscoveryPipeline` output (`Vec<RawCandidate>`) feeds
    //! `identity::resolve` and produces the expected candidate count — the
    //! discover → resolve seam, using a fake Exa source (zero network).

    use super::resolve;
    use crate::recruiting::adapters::exa::ExaHit;
    use crate::recruiting::identity::arbiter::NoopArbiter;
    use crate::recruiting::intake::schemas::{
        SearchConfig, SearchQuery, SearchQueryTier, TierThresholds,
    };
    use crate::recruiting::search::source::FakeCandidateSource;
    use crate::recruiting::search::{DiscoveryPipeline, NoopCostGate};

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

    fn cfg() -> SearchConfig {
        SearchConfig {
            role_name: "test".into(),
            tiers: vec![SearchQueryTier {
                priority: 1,
                queries: vec![SearchQuery {
                    text: "q".into(),
                    target_companies: None,
                    include_domains: None,
                    exclude_domains: None,
                    max_results: None,
                }],
            }],
            scoring_weights: Default::default(),
            tier_thresholds: TierThresholds {
                tier1_min_score: 70,
                tier2_min_score: 50,
            },
            enrichment_priority: vec![],
            anti_filters: vec![],
            similarity_seeds: vec![],
            max_candidates: 10,
            max_cost_usd: 10.0,
            created_at: "2026-06-03T00:00:00Z".into(),
            version: 1,
        }
    }

    #[tokio::test]
    async fn discovery_pipeline_output_feeds_resolve() {
        // Two distinct URLs for the SAME LinkedIn profile slug — survive URL
        // dedup (different paths) but collapse to one person in identity
        // resolution (Pass 1 LinkedIn exact match on the dash-preserving slug).
        let mut fake = FakeCandidateSource::empty();
        fake.search_results.insert(
            "q".into(),
            vec![
                hit("https://www.linkedin.com/in/sarah-chen"),
                hit("https://linkedin.com/in/sarah-chen?trk=public"),
                hit("https://github.com/different-person"),
            ],
        );

        let (raw, _stats) = DiscoveryPipeline::run(fake, &cfg(), &mut NoopCostGate)
            .await
            .unwrap();

        // 3 distinct URLs survive URL dedup (query string disambiguates #1/#2).
        assert_eq!(raw.len(), 3);

        let res = resolve(raw, &NoopArbiter).await.unwrap();

        // The two LinkedIn URLs resolve to one person; the github URL is a
        // second person → 2 resolved candidates.
        assert_eq!(res.candidates.len(), 2);
        assert_eq!(res.stats.input_count, 3);
        assert_eq!(res.stats.output_count, 2);
        assert!(res.stats.high_confidence_merges >= 1);
    }
}
