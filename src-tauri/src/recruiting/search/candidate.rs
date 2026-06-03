//! `RawCandidate` — the type returned directly from the discovery run loop.
//!
//! Each candidate is a single Exa search hit parsed into a typed struct,
//! with identifiers extracted via `identity::extract_identifiers`. At this
//! stage the candidate has NOT been through identity resolution (that is
//! `identity::resolve`); it may share a real-world person with other
//! `RawCandidate`s that differ only in URL.
//!
//! The two key free functions:
//! - `parse_hit(ExaHit, CandidateProvenance) -> RawCandidate` — the single
//!   `ExaHit → RawCandidate` conversion point (called by the executor for
//!   both seed and tier results).
//! - `dedup_by_url(Vec<RawCandidate>) -> Vec<RawCandidate>` — cheap pre-pass
//!   that collapses same-URL duplicates before the expensive identity merge.

use serde::{Deserialize, Serialize};

use crate::recruiting::adapters::exa::ExaHit;
use crate::recruiting::identity::extract::{extract_identifiers, extract_name};
use crate::recruiting::identity::types::ObservedIdentifier;

// ---------------------------------------------------------------------------
// RawCandidate + provenance types
// ---------------------------------------------------------------------------

/// How a `RawCandidate` was discovered (seed or tier query).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum DiscoveryVia {
    SimilaritySeed,
    TierQuery,
}

/// Tracks which search path produced a `RawCandidate`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CandidateProvenance {
    pub via: DiscoveryVia,
    /// Present when `via == TierQuery`.
    pub query_text: Option<String>,
    /// Present when `via == SimilaritySeed`.
    pub seed_url: Option<String>,
    /// Priority of the tier this came from; `None` for seeds.
    pub tier_priority: Option<u8>,
}

/// A candidate as returned directly from the discovery run loop — after
/// `parse_hit` but before identity resolution or enrichment.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RawCandidate {
    /// Display name inferred from author → title → URL slug.
    pub name: String,
    /// Discovery result URL — dedup key and future enrich key.
    pub url: String,
    /// Exa's internal result ID.
    pub exa_id: String,
    pub title: Option<String>,
    pub author: Option<String>,
    pub published_date: Option<String>,
    /// Exa relevance score — drives dedup tie-breaking.
    pub score: Option<f32>,
    /// `None` at discovery; populated by optional enrichment phase.
    pub text: Option<String>,
    /// Identifiers extracted from URL and page text.
    pub identifiers: Vec<ObservedIdentifier>,
    /// Always `"exa"` for results produced by `ExaCandidateSource`.
    pub source_adapter: String,
    pub provenance: CandidateProvenance,
}

// ---------------------------------------------------------------------------
// parse_hit — single ExaHit → RawCandidate conversion point
// ---------------------------------------------------------------------------

/// Convert one `ExaHit` into a `RawCandidate`, stamping it with `provenance`.
///
/// Calls `extract_identifiers` on the URL (+ optional text) and
/// `extract_name` on author/title/url to fill `name`.  This is the **only**
/// place in the codebase that constructs a `RawCandidate` from an `ExaHit`.
pub fn parse_hit(hit: ExaHit, provenance: CandidateProvenance) -> RawCandidate {
    let identifiers = extract_identifiers(&hit.url, hit.text.as_deref(), "exa");
    let name = extract_name(hit.author.as_deref(), hit.title.as_deref(), &hit.url);
    RawCandidate {
        name,
        url: hit.url.clone(),
        exa_id: hit.id,
        title: hit.title,
        author: hit.author,
        published_date: hit.published_date,
        score: hit.score,
        text: hit.text,
        identifiers,
        source_adapter: "exa".to_string(),
        provenance,
    }
}

// ---------------------------------------------------------------------------
// dedup_by_url — cheap pre-pass before identity resolution
// ---------------------------------------------------------------------------

/// Normalize a URL for grouping purposes.
///
/// Conservative: lowercase, strip scheme (http/https), strip leading `www.`,
/// strip trailing `/`. Query strings are **preserved** — they can
/// disambiguate distinct profiles. Fragment identifiers are also preserved.
///
/// This is intentionally not semantic dedup (two URLs for the same person
/// but with different paths are left as distinct candidates); that is
/// identity resolution's job.
fn normalize_url_key(url: &str) -> String {
    // Strip scheme
    let without_scheme = if let Some(rest) = url.strip_prefix("https://") {
        rest
    } else if let Some(rest) = url.strip_prefix("http://") {
        rest
    } else {
        url
    };

    // Lowercase
    let lowered = without_scheme.to_lowercase();

    // Strip leading www.
    let without_www = if let Some(rest) = lowered.strip_prefix("www.") {
        rest.to_string()
    } else {
        lowered
    };

    // Strip trailing /  (only bare trailing slash, not a path element)
    without_www.trim_end_matches('/').to_string()
}

/// Collapse duplicate URLs, keeping the hit with the highest `score`.
///
/// - `None` score is treated as the lowest possible (below any `Some`).
/// - Among equal scores, the first-seen URL string is preserved.
/// - Distinct normalized URLs are returned in first-seen order.
pub fn dedup_by_url(candidates: Vec<RawCandidate>) -> Vec<RawCandidate> {
    // Preserve first-seen order via an ordered index list, deduplicate using
    // a map from normalized key → index into the output vec.
    let mut result: Vec<RawCandidate> = Vec::new();
    let mut key_to_idx: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    for cand in candidates {
        let key = normalize_url_key(&cand.url);
        if let Some(&idx) = key_to_idx.get(&key) {
            // We've seen this key before — keep the higher score.
            let existing_score = result[idx].score;
            let new_score = cand.score;
            // `Some(x) > None` for all x (None is lowest).
            if new_score > existing_score {
                // Keep new score and url but preserve all other fields from
                // the first-seen candidate (provenance, identifiers, etc.)
                result[idx].score = new_score;
                // Also update the canonical URL to the first-seen form
                // (key_to_idx already points to the right slot).
            }
            // Otherwise discard.
        } else {
            let idx = result.len();
            key_to_idx.insert(key, idx);
            result.push(cand);
        }
    }

    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recruiting::adapters::exa::ExaHit;

    fn hit(url: &str, score: Option<f32>) -> ExaHit {
        ExaHit {
            id: url.into(),
            url: url.into(),
            title: None,
            score,
            published_date: None,
            author: None,
            text: None,
            highlights: None,
            summary: None,
        }
    }

    fn prov_seed() -> CandidateProvenance {
        CandidateProvenance {
            via: DiscoveryVia::SimilaritySeed,
            query_text: None,
            seed_url: Some("s".into()),
            tier_priority: None,
        }
    }

    #[test]
    fn parse_hit_maps_fields_and_provenance() {
        let prov = CandidateProvenance {
            via: DiscoveryVia::TierQuery,
            query_text: Some("rust".into()),
            seed_url: None,
            tier_priority: Some(1),
        };
        let c = parse_hit(hit("https://linkedin.com/in/jane-doe", Some(0.9)), prov.clone());
        assert_eq!(c.url, "https://linkedin.com/in/jane-doe");
        assert_eq!(c.provenance.tier_priority, Some(1));
        assert!(c.identifiers.iter().any(|i| i.value == "jane-doe"));
    }

    #[test]
    fn parse_hit_sets_source_adapter_exa() {
        let prov = prov_seed();
        let c = parse_hit(hit("https://github.com/jsmith", None), prov);
        assert_eq!(c.source_adapter, "exa");
    }

    #[test]
    fn parse_hit_extracts_name_from_url_slug() {
        let prov = prov_seed();
        let c = parse_hit(hit("https://example.com/john-smith", None), prov);
        assert_eq!(c.name, "john smith");
    }

    #[test]
    fn dedup_keeps_best_score_and_order() {
        let v = vec![
            parse_hit(hit("https://e.com/a", Some(0.5)), prov_seed()),
            parse_hit(hit("https://E.com/a/", Some(0.8)), prov_seed()),
            parse_hit(hit("https://e.com/b", None), prov_seed()),
        ];
        let out = dedup_by_url(v);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].url, "https://e.com/a");
        assert_eq!(out[0].score, Some(0.8));
    }

    #[test]
    fn dedup_preserves_none_score_when_lower_than_existing_some() {
        let v = vec![
            parse_hit(hit("https://e.com/x", Some(0.7)), prov_seed()),
            parse_hit(hit("https://e.com/x", None), prov_seed()),
        ];
        let out = dedup_by_url(v);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].score, Some(0.7));
    }

    #[test]
    fn dedup_both_none_keeps_first() {
        let v = vec![
            parse_hit(hit("https://e.com/y", None), prov_seed()),
            parse_hit(hit("https://e.com/y", None), prov_seed()),
        ];
        let out = dedup_by_url(v);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn dedup_preserves_query_strings() {
        let v = vec![
            parse_hit(hit("https://e.com/p?id=1", Some(0.5)), prov_seed()),
            parse_hit(hit("https://e.com/p?id=2", Some(0.6)), prov_seed()),
        ];
        let out = dedup_by_url(v);
        assert_eq!(out.len(), 2, "distinct query strings are distinct URLs");
    }

    #[test]
    fn dedup_strips_www_and_scheme() {
        let v = vec![
            parse_hit(hit("https://www.linkedin.com/in/alice", Some(0.6)), prov_seed()),
            parse_hit(hit("http://linkedin.com/in/alice", Some(0.9)), prov_seed()),
        ];
        let out = dedup_by_url(v);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].score, Some(0.9));
    }

    #[test]
    fn dedup_empty_input_is_empty() {
        let out = dedup_by_url(vec![]);
        assert!(out.is_empty());
    }
}
