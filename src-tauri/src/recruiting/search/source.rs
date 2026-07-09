//! `CandidateSource` — the network seam for candidate discovery.
//!
//! Only two methods: `search` (tiered query) and `find_similar` (seed URL).
//! Production impl hits Exa; `FakeCandidateSource` (test-only) returns
//! scripted `ExaHit` vecs and records call order.

use async_trait::async_trait;
use thiserror::Error;

use crate::recruiting::adapters::exa::{ExaError, ExaHit, ExaSearchOpts};
use crate::recruiting::intake::schemas::SearchQuery;

// ---------------------------------------------------------------------------
// SearchError
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum SearchError {
    #[error("exa source error: {0}")]
    Source(#[from] ExaError),
}

// ---------------------------------------------------------------------------
// Trait
// ---------------------------------------------------------------------------

/// The discovery network seam.
///
/// Each method returns `(hits, actual_cost_usd)` where `actual_cost_usd` is
/// `Some` when Exa reported `costDollars.total` for the call and `None`
/// otherwise. The executor forwards that `Option<f64>` to `CostGate::record`
/// so Task 5's `CostTracker` can prefer the real number over its estimate
/// (spec §12) without any further signature change.
#[async_trait]
pub trait CandidateSource: Send + Sync {
    async fn search(
        &self,
        query: &SearchQuery,
        num_results: u32,
    ) -> Result<(Vec<ExaHit>, Option<f64>), SearchError>;

    async fn find_similar(&self, seed_url: &str)
        -> Result<(Vec<ExaHit>, Option<f64>), SearchError>;
}

// ---------------------------------------------------------------------------
// Production implementation
// ---------------------------------------------------------------------------

/// Wires `CandidateSource` to the live Exa API.
pub struct ExaCandidateSource {
    pub exa_api_key: String,
    /// FHR-91: present in production so discovery egress is audited; `None`
    /// only in unit tests, which have no DB pool.
    audit: Option<crate::recruiting::adapters::exa::ExaAudit>,
}

impl ExaCandidateSource {
    pub fn new(exa_api_key: String) -> Self {
        Self {
            exa_api_key,
            audit: None,
        }
    }

    /// Attach the DB pool so this source's Exa egress writes audit rows.
    pub fn with_audit(mut self, audit: crate::recruiting::adapters::exa::ExaAudit) -> Self {
        self.audit = Some(audit);
        self
    }
}

#[async_trait]
impl CandidateSource for ExaCandidateSource {
    async fn search(
        &self,
        query: &SearchQuery,
        num_results: u32,
    ) -> Result<(Vec<ExaHit>, Option<f64>), SearchError> {
        let opts = ExaSearchOpts {
            num_results,
            include_domains: query.include_domains.as_deref(),
            exclude_domains: query.exclude_domains.as_deref(),
            category: Some("people"),
        };
        let resp = crate::recruiting::adapters::exa::search_with(
            &query.text,
            &opts,
            &self.exa_api_key,
            self.audit.as_ref(),
        )
        .await?;
        let actual_cost = resp.cost_dollars.as_ref().map(|c| c.total);
        Ok((resp.results, actual_cost))
    }

    async fn find_similar(
        &self,
        seed_url: &str,
    ) -> Result<(Vec<ExaHit>, Option<f64>), SearchError> {
        let resp = crate::recruiting::adapters::exa::find_similar(
            seed_url,
            &self.exa_api_key,
            self.audit.as_ref(),
        )
        .await?;
        let actual_cost = resp.cost_dollars.as_ref().map(|c| c.total);
        Ok((resp.results, actual_cost))
    }
}

// ---------------------------------------------------------------------------
// Test fake
// ---------------------------------------------------------------------------

/// Scripted `CandidateSource` for unit tests.
///
/// - `search_results` — keyed by `query.text`; returns the mapped vec or `[]`.
/// - `similar_results` — keyed by `seed_url`; returns the mapped vec or `[]`.
/// - `calls` — records `"search:<text>"` / `"similar:<url>"` in order.
/// - `fail_query` — if `Some("text")`, the matching `search` call returns a
///   non-fatal `ExaError::Api` (to test per-query error isolation).
/// - `fail_with_invalid_key` — if true, every call returns `ExaError::InvalidKey`.
#[cfg(test)]
pub struct FakeCandidateSource {
    pub search_results: std::collections::HashMap<String, Vec<ExaHit>>,
    pub similar_results: std::collections::HashMap<String, Vec<ExaHit>>,
    pub calls: std::sync::Mutex<Vec<String>>,
    pub fail_query: Option<String>,
    pub fail_with_invalid_key: bool,
}

#[cfg(test)]
impl FakeCandidateSource {
    pub fn empty() -> Self {
        Self {
            search_results: std::collections::HashMap::new(),
            similar_results: std::collections::HashMap::new(),
            calls: std::sync::Mutex::new(vec![]),
            fail_query: None,
            fail_with_invalid_key: false,
        }
    }
}

#[cfg(test)]
#[async_trait]
impl CandidateSource for FakeCandidateSource {
    async fn search(
        &self,
        query: &SearchQuery,
        _num_results: u32,
    ) -> Result<(Vec<ExaHit>, Option<f64>), SearchError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("search:{}", query.text));

        if self.fail_with_invalid_key {
            return Err(SearchError::Source(ExaError::InvalidKey));
        }
        if let Some(ref fail_text) = self.fail_query {
            if fail_text == &query.text {
                return Err(SearchError::Source(ExaError::Api {
                    status: 500,
                    body: "fake error".into(),
                }));
            }
        }
        // No scripted cost in tests; `None` exercises the estimate-fallback path.
        Ok((
            self.search_results
                .get(&query.text)
                .cloned()
                .unwrap_or_default(),
            None,
        ))
    }

    async fn find_similar(
        &self,
        seed_url: &str,
    ) -> Result<(Vec<ExaHit>, Option<f64>), SearchError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("similar:{}", seed_url));

        if self.fail_with_invalid_key {
            return Err(SearchError::Source(ExaError::InvalidKey));
        }
        Ok((
            self.similar_results
                .get(seed_url)
                .cloned()
                .unwrap_or_default(),
            None,
        ))
    }
}
