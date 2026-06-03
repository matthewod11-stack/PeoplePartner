//! Identity identifier types for the Sourcerer module (FHR-73 S1.1).
//!
//! The identifier subset (`IdentifierType`, `ConfidenceLevel`,
//! `ObservedIdentifier`) shipped in Task 2.  Task 4 adds the resolution
//! surface: `PersonIdentity`, `ResolvedCandidate`, `SourceData`, the merge
//! types (`MergeRule`/`MergeReason`/`MergeDecision`), `ClusterSummary`, and the
//! `ResolveResult`/`ResolveStats`/`IdentityError` returned by `resolve()`.
//!
//! These are faithful ports of the `@sourcerer/core` `identity-resolver.ts`
//! shapes, trimmed to the S1.1 boundary (spec §5.2 + §10): no `evidence`,
//! `enrichments`, `signals`, `score`, `tier`, or `pii` surfaces (those belong to
//! later phases / FHR-90), and the always-empty TS `pendingMerges` field is
//! dropped.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The kind of identifier observed for a person.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentifierType {
    Linkedin,
    Email,
    Github,
    Twitter,
    PersonalUrl,
    NameCompany,
}

/// Confidence level for an observed identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfidenceLevel {
    High,
    Medium,
    Low,
}

/// An identifier observed in a search result.
///
/// `value` holds the normalized form; `raw_value` holds the raw string as
/// discovered.  Email identifiers are merge keys only — no PII storage surface
/// (PII extraction is FHR-90, out of scope here).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObservedIdentifier {
    #[serde(rename = "type")]
    pub kind: IdentifierType,
    /// Normalized value (e.g. the LinkedIn slug, lowercased email, etc.)
    pub value: String,
    /// Raw string as found in the page/URL before normalization.
    pub raw_value: String,
    /// Which adapter produced this identifier (e.g. `"exa"`).
    pub source_adapter: String,
    pub confidence: ConfidenceLevel,
}

// ---------------------------------------------------------------------------
// Resolution surface (Task 4)
// ---------------------------------------------------------------------------

/// Per-adapter source provenance attached to a resolved candidate.
///
/// One `ResolvedCandidate` carries a map of these keyed by adapter name
/// (`"exa"`, `"github"`, …); the URL list is the set of discovery URLs that
/// adapter contributed.  This is the trimmed Rust analogue of the TS
/// `SourceData` — `retrieved_at` is dropped (no per-call timestamp is threaded
/// through discovery yet) in favour of the minimal merge-key surface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceData {
    /// Adapter that produced this source (e.g. `"exa"`).
    pub adapter: String,
    /// Discovery URLs this adapter contributed for the person.
    pub urls: Vec<String>,
}

/// A stable identity for a person resolved across one or more sources.
///
/// Faithful port of TS `PersonIdentity`.  `merge_confidence` is the numeric
/// score (1.0 single / 0.95 high+medium / 0.7 low); `low_confidence_merge` is
/// set when the candidate was auto-merged via the Pass-4 similar-name+company
/// rule.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonIdentity {
    pub canonical_id: String,
    pub observed_identifiers: Vec<ObservedIdentifier>,
    /// Adapters this identity was merged from; `None` for a single-source
    /// (un-merged) candidate.  Order follows merge application (A then folded-in
    /// B clusters), matching the TS `cluster.rawCandidates.map(...)` order.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merged_from: Option<Vec<String>>,
    /// Numeric merge confidence: 1.0 (single), 0.95 (high/medium), 0.7 (low).
    pub merge_confidence: f64,
    /// `true` when auto-merged via the low-confidence Pass-4 rule.
    pub low_confidence_merge: bool,
}

/// A person resolved out of the discovery candidate set (post-identity).
///
/// Spec §5.2: deliberately omits TS `evidence`/`enrichments`/`signals`/`score`/
/// `tier`/`narrative` (later phases) and `pii` (FHR-90).  `page_text` is the
/// single mutable enrichment surface (populated by Task 6's enrichment).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedCandidate {
    /// Canonical id — equal to `identity.canonical_id`; the future
    /// `candidates`-table idempotent upsert key.
    pub id: String,
    pub identity: PersonIdentity,
    /// Display name chosen by `choose_best_name` (modal over source names).
    pub name: String,
    /// Per-adapter sources, keyed by adapter name.  `BTreeMap` for
    /// deterministic iteration/serialization.
    pub sources: BTreeMap<String, SourceData>,
    /// Populated by optional enrichment (Task 6); `None` at resolution time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_text: Option<String>,
}

impl ResolvedCandidate {
    /// Set the enrichment page text in place.
    ///
    /// Trivial setter kept here because §5.2 names `page_text` "the single
    /// mutable enrichment surface" on `ResolvedCandidate`; Task 6's
    /// `enrich_candidates` calls this to fan fetched pages back onto candidates.
    pub fn apply_page(&mut self, page_text: String) {
        self.page_text = Some(page_text);
    }
}

/// Which heuristic produced a merge.  Mirrors TS `MergeRule`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeRule {
    LinkedinUrl,
    VerifiedEmail,
    GithubUsername,
    CrossSourceEmail,
    NameCompany,
    SimilarNameCompany,
}

/// Why two clusters were merged, with the confidence tier and matched values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeReason {
    pub rule: MergeRule,
    pub confidence: ConfidenceLevel,
    /// The two matched values that triggered the merge (`[a, b]`).
    pub matched_values: [String; 2],
    pub description: String,
}

/// A single merge applied during resolution (folds cluster B into cluster A).
///
/// **`cluster_index_a` / `cluster_index_b` are ephemeral working-array indices**
/// into the resolver's internal cluster `Vec`, valid only within the single
/// `resolve()` call that produced this decision. They are NOT stable references
/// to any persistent entity (no `candidates` row, no canonical id, no input
/// position). Consumers of `merge_log` must treat them as provenance/audit
/// breadcrumbs only — never use them to look anything up after `resolve()`
/// returns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeDecision {
    pub cluster_index_a: usize,
    pub cluster_index_b: usize,
    pub reason: MergeReason,
    /// Always `true` in S1.1 — every heuristic merge is automatic (the arbiter
    /// can only veto a Pass-4 candidate before the decision is emitted).
    pub automatic: bool,
}

/// A read-only summary of a cluster handed to the `IdentityArbiter` so it can
/// make a same-person judgement without seeing internal cluster machinery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClusterSummary {
    /// Distinct candidate display names in the cluster.
    pub names: Vec<String>,
    /// Distinct companies parsed from `name_company` identifiers.
    pub companies: Vec<String>,
    /// Distinct adapters that contributed to the cluster.
    pub adapters: Vec<String>,
    /// All identifiers in the cluster.
    pub identifiers: Vec<ObservedIdentifier>,
}

/// Aggregate counts for a resolution run.  Mirrors TS `ResolveResult.stats`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveStats {
    pub input_count: usize,
    pub output_count: usize,
    pub high_confidence_merges: usize,
    pub medium_confidence_merges: usize,
    pub low_confidence_merges: usize,
}

/// The result of resolving a `Vec<RawCandidate>` into canonical candidates.
///
/// The always-empty TS `pendingMerges` field is dropped (spec §10);
/// `low_confidence_merge` on each identity plus `merge_log` carry what a future
/// review UI needs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveResult {
    pub candidates: Vec<ResolvedCandidate>,
    pub merge_log: Vec<MergeDecision>,
    pub stats: ResolveStats,
}

/// Errors from identity resolution.
///
/// `resolve()` with the default `NoopArbiter` never errors; this enum exists so
/// a real LLM arbiter can surface a failure (e.g. transport) through the
/// `IdentityArbiter` seam without changing the `resolve()` signature.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IdentityError {
    /// The arbiter seam failed to render a verdict (e.g. LLM transport error).
    #[error("identity arbiter error: {0}")]
    Arbiter(String),
}
