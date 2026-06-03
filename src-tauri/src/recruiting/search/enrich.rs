//! Optional content enrichment for resolved candidates (FHR-73 S1.1, Task 6).
//!
//! `enrich_candidates` runs on a `&mut [ResolvedCandidate]` *after* identity
//! resolution. It batches candidate URLs (≤ `batch_size` per Exa call), checks
//! the budget before each batch, fetches page text, and fans results back via
//! `ResolvedCandidate::apply_page`.
//!
//! # Ruled decisions honoured
//! - **#2** — Soft partial success: budget exhaustion sets `stopped_early` and
//!   returns the partial `EnrichmentOutcome`; no error is returned.
//! - **#3** — Always-on-if-keyed default: `None` `EnrichmentPriority` for Exa
//!   → treat as `run_condition = "always"` (enrich when the key is present).
//!
//! # URL selection
//! A candidate's primary URL is the first URL from its first source in
//! `BTreeMap` iteration order (alphabetical by adapter name). Candidates with
//! no URLs in any source are counted in `skipped_no_url` and left untouched.

use async_trait::async_trait;
use serde::Serialize;
use thiserror::Error;

use crate::recruiting::{
    adapters::exa::{self, ExaError},
    cost::{CostDecision, CostModel, CostTracker, ExaOp},
    identity::types::ResolvedCandidate,
    intake::schemas::{EnrichmentPriority, SearchConfig},
};

// ---------------------------------------------------------------------------
// FetchedPage
// ---------------------------------------------------------------------------

/// One page's fetched content — maps to a single `ExaHit` from `/contents`.
#[derive(Debug, Clone)]
pub struct FetchedPage {
    pub url: String,
    pub text: Option<String>,
    pub title: Option<String>,
}

// ---------------------------------------------------------------------------
// EnrichError
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum EnrichError {
    /// The Exa `/contents` call failed.
    #[error("exa contents error: {0}")]
    Exa(#[from] ExaError),
    /// All URLs in the batch were empty (should not normally occur).
    #[error("empty url batch")]
    Empty,
}

impl EnrichError {
    /// `true` when retrying the same batch might succeed (HTTP 429).
    pub fn retryable(&self) -> bool {
        matches!(self, EnrichError::Exa(ExaError::RateLimit { .. }))
    }
}

// ---------------------------------------------------------------------------
// ContentEnricher trait + impls
// ---------------------------------------------------------------------------

/// Seam for content fetching — production uses Exa; tests use `FakeEnricher`.
///
/// Returns `(pages, actual_cost_usd)`: the second element is Exa's reported
/// `costDollars.total` when present, threaded into `CostTracker::record` so
/// actuals reconcile the budget exactly like the discovery seam (Task 3,
/// spec §12). `None` falls back to the model estimate.
#[async_trait]
pub trait ContentEnricher: Send + Sync {
    /// Fetch page content for the given URLs in one batch call.
    async fn fetch_contents(
        &self,
        urls: &[String],
    ) -> Result<(Vec<FetchedPage>, Option<f64>), EnrichError>;
}

// ---------------------------------------------------------------------------
// ExaContentEnricher
// ---------------------------------------------------------------------------

/// Production enricher that delegates to `exa::get_contents`.
pub struct ExaContentEnricher {
    pub exa_api_key: String,
}

#[async_trait]
impl ContentEnricher for ExaContentEnricher {
    async fn fetch_contents(
        &self,
        urls: &[String],
    ) -> Result<(Vec<FetchedPage>, Option<f64>), EnrichError> {
        if urls.is_empty() {
            return Err(EnrichError::Empty);
        }
        let resp = exa::get_contents(urls, &self.exa_api_key).await?;
        // Thread Exa's reported per-call cost so `record` reconciles the budget
        // with the actual charge (spec §12), mirroring the discovery seam.
        let actual_cost = resp.cost_dollars.as_ref().map(|c| c.total);
        let pages = resp
            .results
            .into_iter()
            .map(|hit| FetchedPage {
                url: hit.url,
                text: hit.text,
                title: hit.title,
            })
            .collect();
        Ok((pages, actual_cost))
    }
}

// ---------------------------------------------------------------------------
// FakeEnricher (test only)
// ---------------------------------------------------------------------------

/// Test double for `ContentEnricher`. Configured via builder methods.
#[cfg(test)]
pub struct FakeEnricher {
    /// Batch indices that should fail (0-based). `None` = never fail.
    fail_batches: std::collections::HashSet<usize>,
    /// Track how many `fetch_contents` calls were made.
    call_count: std::sync::atomic::AtomicUsize,
    /// Optional override: return this error kind on failed batches.
    /// `true` = RateLimit (retryable), `false` = Api (not retryable).
    rate_limit_on_failure: bool,
    /// Per-call `actual_cost` the fake reports as the second tuple element —
    /// lets a test assert the seam threads Exa's reported cost into `record`.
    actual_cost: Option<f64>,
}

#[cfg(test)]
impl FakeEnricher {
    /// Always succeeds — echoes back a `FetchedPage` per URL with canned text.
    pub fn echo() -> Self {
        Self {
            fail_batches: std::collections::HashSet::new(),
            call_count: std::sync::atomic::AtomicUsize::new(0),
            rate_limit_on_failure: false,
            actual_cost: None,
        }
    }

    /// Like `echo`, but reports a fixed per-call `actual_cost` so a test can
    /// assert the cost seam threads actuals into `CostTracker::record`.
    pub fn echo_with_cost(actual_cost: f64) -> Self {
        Self {
            fail_batches: std::collections::HashSet::new(),
            call_count: std::sync::atomic::AtomicUsize::new(0),
            rate_limit_on_failure: false,
            actual_cost: Some(actual_cost),
        }
    }

    /// Fail on the given 0-based batch index; all other batches succeed.
    pub fn fail_batch(idx: usize) -> Self {
        let mut set = std::collections::HashSet::new();
        set.insert(idx);
        Self {
            fail_batches: set,
            call_count: std::sync::atomic::AtomicUsize::new(0),
            rate_limit_on_failure: false,
            actual_cost: None,
        }
    }

    /// Same as `fail_batch` but the error is a `RateLimit` (retryable).
    pub fn fail_batch_rate_limit(idx: usize) -> Self {
        let mut set = std::collections::HashSet::new();
        set.insert(idx);
        Self {
            fail_batches: set,
            call_count: std::sync::atomic::AtomicUsize::new(0),
            rate_limit_on_failure: true,
            actual_cost: None,
        }
    }

    /// Number of `fetch_contents` calls received so far.
    pub fn call_count(&self) -> usize {
        self.call_count
            .load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[cfg(test)]
#[async_trait]
impl ContentEnricher for FakeEnricher {
    async fn fetch_contents(
        &self,
        urls: &[String],
    ) -> Result<(Vec<FetchedPage>, Option<f64>), EnrichError> {
        let idx = self
            .call_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        if self.fail_batches.contains(&idx) {
            if self.rate_limit_on_failure {
                return Err(EnrichError::Exa(ExaError::RateLimit {
                    message: "rate limited".into(),
                }));
            } else {
                return Err(EnrichError::Exa(ExaError::Api {
                    status: 500,
                    body: "fake error".into(),
                }));
            }
        }

        let pages = urls
            .iter()
            .map(|url| FetchedPage {
                url: url.clone(),
                text: Some(format!("page text for {url}")),
                title: Some(format!("Title: {url}")),
            })
            .collect();
        // `actual_cost` defaults to None (record falls back to the model
        // estimate); `echo_with_cost` sets it to exercise the seam.
        Ok((pages, self.actual_cost))
    }
}

// ---------------------------------------------------------------------------
// EnrichConfig
// ---------------------------------------------------------------------------

/// Configuration for a single `enrich_candidates` call.
#[derive(Debug, Clone)]
pub struct EnrichConfig {
    /// Maximum URLs per Exa `/contents` call. Default: 5.
    pub batch_size: usize,
}

impl Default for EnrichConfig {
    fn default() -> Self {
        Self { batch_size: 5 }
    }
}

// ---------------------------------------------------------------------------
// EnrichmentOutcome
// ---------------------------------------------------------------------------

/// One candidate failure from a batch error.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateFailure {
    /// The URL that was being fetched (or `"<no url>"` for edge cases).
    pub url: String,
    /// `true` if the error is a 429 that could be retried.
    pub retryable: bool,
}

/// Summary returned by `enrich_candidates`.
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnrichmentOutcome {
    /// Number of candidates whose `page_text` was set.
    pub enriched: usize,
    /// Candidates with no URL in any source — left untouched.
    pub skipped_no_url: usize,
    /// Batches that errored; each entry covers the URLs in that batch.
    pub failures: Vec<CandidateFailure>,
    /// `true` when the budget ran out before all candidates were enriched.
    pub stopped_early: bool,
}

// ---------------------------------------------------------------------------
// run_condition helpers
// ---------------------------------------------------------------------------

/// Find the Exa enrichment priority entry from a `SearchConfig`.
pub fn exa_enrichment_priority(cfg: &SearchConfig) -> Option<&EnrichmentPriority> {
    cfg.enrichment_priority
        .iter()
        .find(|ep| ep.adapter == "exa")
}

/// Decide whether to run Exa enrichment at all.
///
/// - `"always"` → true
/// - `"required"` → true
/// - `"if_cheap_insufficient"` → true unless every candidate already has
///   `page_text` (interpreted as "cheap data already sufficient")
/// - `None` → **true** (ruled decision #3: always-on-if-keyed default)
pub fn should_run_exa_enrichment(
    prio: Option<&EnrichmentPriority>,
    candidates: &[ResolvedCandidate],
) -> bool {
    match prio {
        None => true,
        Some(ep) => match ep.run_condition.as_str() {
            "always" | "required" => true,
            "if_cheap_insufficient" => {
                // Run unless every candidate already has page_text.
                !candidates.iter().all(|c| c.page_text.is_some())
            }
            // An unknown run_condition string is almost certainly a schema/typo
            // artifact. We default it ON to match the always-on-if-keyed posture
            // (ruled decision #3) rather than silently skipping enrichment.
            // TODO: emit a `warn!` here once this is wired to real callers so an
            // operator typo surfaces instead of passing silently.
            _ => true,
        },
    }
}

// ---------------------------------------------------------------------------
// Primary URL extraction
// ---------------------------------------------------------------------------

/// Pick the primary URL for a candidate: first URL from the first source
/// in `BTreeMap` order (alphabetical by adapter name).
///
/// Returns `None` when the candidate has no URLs in any source.
fn primary_url(candidate: &ResolvedCandidate) -> Option<&str> {
    candidate
        .sources
        .values()
        .flat_map(|sd| sd.urls.iter())
        .next()
        .map(String::as_str)
}

// ---------------------------------------------------------------------------
// enrich_candidates
// ---------------------------------------------------------------------------

/// Enrich resolved candidates in place with page text from Exa's `/contents`.
///
/// Builds a URL worklist (one URL per candidate), chunks into batches of
/// `cfg.batch_size` (default 5), and for each batch:
///
/// 1. `cost.check_before(GetContents, batch_len)` — on `Stop`, set
///    `stopped_early` and halt (soft partial, ruled decision #2).
/// 2. `enricher.fetch_contents(batch)` — on error, record a `CandidateFailure`
///    and continue to the next batch (skip-and-continue).
/// 3. Fan pages back by URL match → `candidate.apply_page(text)`.
/// 4. `cost.record(GetContents, batch_len, actual_cost)`.
pub async fn enrich_candidates(
    candidates: &mut [ResolvedCandidate],
    enricher: &dyn ContentEnricher,
    cost: &mut CostTracker,
    cfg: &EnrichConfig,
) -> EnrichmentOutcome {
    let mut outcome = EnrichmentOutcome::default();

    // Build URL worklist: (candidate_index, url)
    // Candidates with no usable URL are counted immediately.
    //
    // Relies on upstream `search::candidate::dedup_by_url` having already
    // collapsed identical URLs before identity resolution, so distinct
    // candidates don't carry the same primary URL — i.e. a URL is not
    // fetched (or charged) twice in practice.
    let mut worklist: Vec<(usize, String)> = Vec::new();
    for (idx, candidate) in candidates.iter().enumerate() {
        match primary_url(candidate) {
            Some(url) => worklist.push((idx, url.to_owned())),
            None => outcome.skipped_no_url += 1,
        }
    }

    // Process in batches.
    for batch in worklist.chunks(cfg.batch_size) {
        let batch_urls: Vec<String> = batch.iter().map(|(_, url)| url.clone()).collect();
        let units = batch_urls.len() as u32;

        // Pre-call budget check.
        match CostTracker::check_before(cost, ExaOp::GetContents, units) {
            CostDecision::Stop { .. } => {
                outcome.stopped_early = true;
                return outcome;
            }
            CostDecision::Proceed { .. } => {}
        }

        // Fetch content. `actual_cost` is Exa's reported `costDollars.total`
        // (None for the fake / plans that don't report it), threaded into
        // `record` below so actuals reconcile the budget (spec §12).
        let (pages, actual_cost) = match enricher.fetch_contents(&batch_urls).await {
            Ok(result) => result,
            Err(err) => {
                // Skip-and-continue: record failure for all URLs in the batch.
                let retryable = err.retryable();
                for (_, url) in batch {
                    outcome.failures.push(CandidateFailure {
                        url: url.clone(),
                        retryable,
                    });
                }
                // POLICY (charge-on-failure): we record the *estimated* cost
                // (actual = None) even when the batch errored. This is
                // conservative — it avoids underreporting spend if Exa bills
                // failed/429 calls. Revisit if Exa clarifies it does not bill
                // failures, in which case skip this `record` for error paths.
                CostTracker::record(cost, ExaOp::GetContents, units, None);
                continue;
            }
        };

        // Build a URL→text map for O(n) matching.
        let page_map: std::collections::HashMap<&str, Option<&str>> = pages
            .iter()
            .map(|p| (p.url.as_str(), p.text.as_deref()))
            .collect();

        for (idx, url) in batch {
            if let Some(Some(text)) = page_map.get(url.as_str()) {
                candidates[*idx].apply_page((*text).to_owned());
                outcome.enriched += 1;
            }
        }

        CostTracker::record(cost, ExaOp::GetContents, units, actual_cost);
    }

    outcome
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::recruiting::{
        cost::{CostModel, CostTracker},
        identity::types::{
            ConfidenceLevel, IdentifierType, ObservedIdentifier, PersonIdentity, ResolvedCandidate,
            SourceData,
        },
    };

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn make_candidate(id: &str, url: Option<&str>) -> ResolvedCandidate {
        let mut sources = BTreeMap::new();
        if let Some(u) = url {
            sources.insert(
                "exa".to_owned(),
                SourceData {
                    adapter: "exa".to_owned(),
                    urls: vec![u.to_owned()],
                },
            );
        }
        ResolvedCandidate {
            id: id.to_owned(),
            identity: PersonIdentity {
                canonical_id: id.to_owned(),
                observed_identifiers: vec![ObservedIdentifier {
                    kind: IdentifierType::Linkedin,
                    value: format!("linkedin-{id}"),
                    raw_value: format!("linkedin-{id}"),
                    source_adapter: "exa".to_owned(),
                    confidence: ConfidenceLevel::High,
                }],
                merged_from: None,
                merge_confidence: 1.0,
                low_confidence_merge: false,
            },
            name: format!("Candidate {id}"),
            sources,
            page_text: None,
        }
    }

    fn resolved_candidates(n: usize) -> Vec<ResolvedCandidate> {
        (0..n)
            .map(|i| make_candidate(&i.to_string(), Some(&format!("https://example.com/{i}"))))
            .collect()
    }

    fn resolved_candidates_no_url(n: usize) -> Vec<ResolvedCandidate> {
        (0..n)
            .map(|i| make_candidate(&i.to_string(), None))
            .collect()
    }

    fn tracker(budget: f64) -> CostTracker {
        CostTracker::new(CostModel::default(), budget)
    }

    fn prio(run_condition: &str) -> EnrichmentPriority {
        EnrichmentPriority {
            adapter: "exa".to_owned(),
            required: false,
            run_condition: run_condition.to_owned(),
        }
    }

    // -----------------------------------------------------------------------
    // Spec §15 enrich test list
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn seven_urls_batch_five_makes_two_calls() {
        let mut resolved = resolved_candidates(7);
        let enricher = FakeEnricher::echo();
        let mut cost = tracker(1000.0);
        let out = enrich_candidates(&mut resolved, &enricher, &mut cost, &EnrichConfig::default()).await;
        assert_eq!(enricher.call_count(), 2, "ceil(7/5) = 2 calls");
        assert!(
            resolved.iter().all(|c| c.page_text.is_some()),
            "all candidates must have page_text after enrichment"
        );
        assert_eq!(out.enriched, 7);
        assert_eq!(out.skipped_no_url, 0);
        assert!(out.failures.is_empty());
        assert!(!out.stopped_early);
    }

    #[tokio::test]
    async fn batch_failure_skips_and_continues() {
        // 10 candidates, batch 0 (first 5) fails → 5 failures, batches 1..= succeed.
        let mut resolved = resolved_candidates(10);
        let enricher = FakeEnricher::fail_batch(0);
        let mut cost = tracker(1000.0);
        let out = enrich_candidates(&mut resolved, &enricher, &mut cost, &EnrichConfig::default()).await;
        assert_eq!(out.failures.len(), 5, "all 5 URLs in the failing batch are recorded as failures");
        // The second batch (indices 5..10) must have succeeded.
        assert!(
            resolved.iter().skip(5).all(|c| c.page_text.is_some()),
            "candidates from the second batch must be enriched"
        );
        assert_eq!(out.enriched, 5);
        assert!(!out.stopped_early);
    }

    #[tokio::test]
    async fn run_condition_gating_none_is_always_on() {
        // None → ruled decision #3 → always true.
        let any_candidates = resolved_candidates(3);
        assert!(
            should_run_exa_enrichment(None, &any_candidates),
            "None prio must default to always-on"
        );
    }

    #[tokio::test]
    async fn run_condition_gating_always() {
        let candidates = resolved_candidates(3);
        assert!(should_run_exa_enrichment(Some(&prio("always")), &candidates));
    }

    #[tokio::test]
    async fn run_condition_gating_if_cheap_insufficient_not_all_enriched() {
        let candidates = resolved_candidates(3); // none have page_text
        assert!(
            should_run_exa_enrichment(Some(&prio("if_cheap_insufficient")), &candidates),
            "should run when not all candidates are enriched"
        );
    }

    #[tokio::test]
    async fn run_condition_gating_if_cheap_insufficient_all_enriched() {
        let mut candidates = resolved_candidates(3);
        for c in &mut candidates {
            c.apply_page("already have text".to_owned());
        }
        assert!(
            !should_run_exa_enrichment(Some(&prio("if_cheap_insufficient")), &candidates),
            "should NOT run when all candidates already have page_text"
        );
    }

    #[tokio::test]
    async fn budget_exhaustion_yields_partial_enrichment() {
        // Budget = exactly one batch of 5 URLs. Second batch is blocked.
        // Derive from the model rate (not a hardcoded 0.025) so a future
        // pricing change can't silently test the wrong boundary — mirrors
        // `tracker.rs::exact_boundary_is_allowed` tying to the constant.
        let mut resolved = resolved_candidates(10);
        let budget = CostModel::default().per_contents_url_usd * 5.0;
        let mut cost = CostTracker::new(CostModel::default(), budget);
        let out = enrich_candidates(
            &mut resolved,
            &FakeEnricher::echo(),
            &mut cost,
            &EnrichConfig::default(),
        )
        .await;
        assert!(out.stopped_early, "must stop early on budget exhaustion");
        assert!(
            resolved.iter().take(5).all(|c| c.page_text.is_some()),
            "first 5 candidates (batch 0) must be enriched"
        );
        assert!(
            resolved.iter().skip(5).all(|c| c.page_text.is_none()),
            "last 5 candidates (batch 1) must NOT be enriched (budget stop)"
        );
    }

    #[tokio::test]
    async fn rate_limit_failure_is_retryable() {
        let mut resolved = resolved_candidates(5);
        let enricher = FakeEnricher::fail_batch_rate_limit(0);
        let mut cost = tracker(1000.0);
        let out = enrich_candidates(&mut resolved, &enricher, &mut cost, &EnrichConfig::default()).await;
        assert!(!out.failures.is_empty(), "failures must be non-empty on rate limit");
        assert!(
            out.failures.iter().all(|f| f.retryable),
            "all failures from 429 must be retryable"
        );
    }

    #[tokio::test]
    async fn non_retryable_failure_is_not_retryable() {
        let mut resolved = resolved_candidates(5);
        let enricher = FakeEnricher::fail_batch(0); // returns Api{500}
        let mut cost = tracker(1000.0);
        let out = enrich_candidates(&mut resolved, &enricher, &mut cost, &EnrichConfig::default()).await;
        assert!(!out.failures.is_empty());
        assert!(
            out.failures.iter().all(|f| !f.retryable),
            "non-429 failures must NOT be retryable"
        );
    }

    #[tokio::test]
    async fn url_less_candidate_is_skipped() {
        // Mix: 2 with URLs, 3 without.
        let mut resolved = vec![
            make_candidate("a", Some("https://example.com/a")),
            make_candidate("b", None),
            make_candidate("c", Some("https://example.com/c")),
            make_candidate("d", None),
            make_candidate("e", None),
        ];
        let enricher = FakeEnricher::echo();
        let mut cost = tracker(1000.0);
        let out = enrich_candidates(&mut resolved, &enricher, &mut cost, &EnrichConfig::default()).await;
        assert_eq!(out.skipped_no_url, 3, "3 url-less candidates must be counted");
        assert_eq!(out.enriched, 2, "2 candidates with URLs must be enriched");
        // url-less candidates must remain untouched (no page_text).
        assert!(resolved[1].page_text.is_none());
        assert!(resolved[3].page_text.is_none());
        assert!(resolved[4].page_text.is_none());
        // url-ful candidates must be enriched.
        assert!(resolved[0].page_text.is_some());
        assert!(resolved[2].page_text.is_some());
    }

    #[tokio::test]
    async fn all_url_less_candidates_produce_zero_calls() {
        let mut resolved = resolved_candidates_no_url(5);
        let enricher = FakeEnricher::echo();
        let mut cost = tracker(1000.0);
        let out = enrich_candidates(&mut resolved, &enricher, &mut cost, &EnrichConfig::default()).await;
        assert_eq!(enricher.call_count(), 0, "no fetch calls when all candidates lack URLs");
        assert_eq!(out.skipped_no_url, 5);
        assert_eq!(out.enriched, 0);
    }

    #[tokio::test]
    async fn actual_cost_is_threaded_into_tracker_record() {
        // The enricher reports a per-call actual of 0.0123 (≠ the 5*0.005=0.025
        // model estimate). The tracker must record the *actual*, proving the
        // cost seam threads it through `record` (spec §12), not the estimate.
        let mut resolved = resolved_candidates(5); // exactly one batch of 5
        let enricher = FakeEnricher::echo_with_cost(0.0123);
        let mut cost = tracker(1000.0);
        let _ = enrich_candidates(&mut resolved, &enricher, &mut cost, &EnrichConfig::default()).await;
        assert_eq!(enricher.call_count(), 1, "exactly one batch");
        assert!(
            (cost.total_usd() - 0.0123).abs() < 1e-9,
            "tracker must record the reported actual cost, not the model estimate; got {:.9}",
            cost.total_usd()
        );
    }

    #[tokio::test]
    async fn exa_enrichment_priority_finds_exa_entry() {
        use crate::recruiting::intake::schemas::{
            AntiFilter, SearchQuery, SearchQueryTier, ScoringWeights, TierThresholds,
        };

        let ep = EnrichmentPriority {
            adapter: "exa".to_owned(),
            required: true,
            run_condition: "always".to_owned(),
        };
        let cfg = SearchConfig {
            role_name: "test".into(),
            tiers: vec![],
            scoring_weights: ScoringWeights::default(),
            tier_thresholds: TierThresholds {
                tier1_min_score: 70,
                tier2_min_score: 50,
            },
            enrichment_priority: vec![ep.clone()],
            anti_filters: vec![],
            similarity_seeds: vec![],
            max_candidates: 100,
            max_cost_usd: 10.0,
            created_at: "2026-06-03".into(),
            version: 1,
        };

        let found = exa_enrichment_priority(&cfg);
        assert!(found.is_some(), "must find the exa entry");
        assert_eq!(found.unwrap().run_condition, "always");

        // Empty config → None
        let empty_cfg = SearchConfig {
            enrichment_priority: vec![],
            ..cfg
        };
        assert!(exa_enrichment_priority(&empty_cfg).is_none());
    }

    #[tokio::test]
    async fn single_candidate_single_call() {
        let mut resolved = resolved_candidates(1);
        let enricher = FakeEnricher::echo();
        let mut cost = tracker(1000.0);
        let out = enrich_candidates(&mut resolved, &enricher, &mut cost, &EnrichConfig::default()).await;
        assert_eq!(enricher.call_count(), 1);
        assert_eq!(out.enriched, 1);
        assert!(resolved[0].page_text.is_some());
    }

    #[tokio::test]
    async fn exactly_one_batch_makes_one_call() {
        let mut resolved = resolved_candidates(5);
        let enricher = FakeEnricher::echo();
        let mut cost = tracker(1000.0);
        let out = enrich_candidates(&mut resolved, &enricher, &mut cost, &EnrichConfig::default()).await;
        assert_eq!(enricher.call_count(), 1, "exactly 5 URLs = exactly 1 batch");
        assert_eq!(out.enriched, 5);
    }
}
