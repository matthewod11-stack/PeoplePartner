//! The 4-pass `IdentityResolver` — deduplicates `RawCandidate`s into canonical
//! `ResolvedCandidate`s. Pure over already-discovered candidates: **no network,
//! no SQLite** (absorbs FHR-74).
//!
//! Faithful port of `@sourcerer/core` `identity-resolver.ts`, with the two
//! consistency deltas pinned by earlier tasks:
//!
//! 1. **LinkedIn merge key is the dash-preserving slug** (`"jane-doe-123"`),
//!    not the TS dash-stripped form — Task 2's `normalize_linkedin_url` is the
//!    source of truth for the key (and the more correct identity key, since
//!    `/in/jane-doe-123` and `/in/janedoe123` are different profiles).
//! 2. **`name_company` identifier values are formatted `"Name @ Company"`** and
//!    parsed via Task 2's `split_name_company` (`@` separator, not TS's `|`).
//!
//! Pass order (exact): (1) high-confidence exact linkedin/email/github index;
//! (2) cross-source email (same normalized email from ≥2 different adapters);
//! (3) name+company (different adapters, both have company, name matches,
//! company exact); (4) similar-name + similar-company (`levenshtein(company)
//! ≤ 3`), arbiter-gated, auto-merge with `low_confidence_merge = true` and
//! `merge_confidence = 0.7`. `apply_merge` folds B into A and tombstones B.

use std::collections::{BTreeMap, HashMap};

use sha2::{Digest, Sha256};

use super::arbiter::IdentityArbiter;
use super::matching::{levenshtein, names_match, names_similar, split_name_company};
use super::normalize::{normalize_email, normalize_identifier_value};
use super::types::{
    ClusterSummary, ConfidenceLevel, IdentifierType, IdentityError, MergeDecision, MergeReason,
    MergeRule, ObservedIdentifier, PersonIdentity, ResolveResult, ResolveStats, ResolvedCandidate,
    SourceData,
};
use crate::recruiting::search::candidate::RawCandidate;

// ---------------------------------------------------------------------------
// Internal cluster type
// ---------------------------------------------------------------------------

/// A working cluster of raw candidates believed to be one person.
///
/// `merged == true` tombstones a cluster that was folded into another; when it
/// is, `merged_into` records the destination cluster index so `apply_merge` can
/// follow the chain to a live root (handles cross-type transitive merges where
/// a later decision names an already-tombstoned cluster as its A side).
struct CandidateCluster {
    raw_candidates: Vec<RawCandidate>,
    all_identifiers: Vec<ObservedIdentifier>,
    merged: bool,
    merged_into: Option<usize>,
}

// ---------------------------------------------------------------------------
// name_company helpers (cluster-level name/company extraction)
// ---------------------------------------------------------------------------

/// Distinct lowercased names in a cluster: every raw candidate's `name`, plus
/// the name half of any `name_company` identifier (`"Name @ Company"`).
fn cluster_names(cluster: &CandidateCluster) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    let push_unique = |s: String, names: &mut Vec<String>| {
        if !s.is_empty() && !names.contains(&s) {
            names.push(s);
        }
    };
    for raw in &cluster.raw_candidates {
        let n = raw.name.to_lowercase().trim().to_string();
        push_unique(n, &mut names);
    }
    for id in &cluster.all_identifiers {
        if id.kind == IdentifierType::NameCompany {
            if let Some((name, _company)) = split_name_company(&id.value) {
                push_unique(name.to_lowercase().trim().to_string(), &mut names);
            }
        }
    }
    names
}

/// Distinct lowercased companies in a cluster: the company half of every
/// `name_company` identifier.
fn cluster_companies(cluster: &CandidateCluster) -> Vec<String> {
    let mut companies: Vec<String> = Vec::new();
    for id in &cluster.all_identifiers {
        if id.kind == IdentifierType::NameCompany {
            if let Some((_name, company)) = split_name_company(&id.value) {
                let c = company.to_lowercase().trim().to_string();
                if !c.is_empty() && !companies.contains(&c) {
                    companies.push(c);
                }
            }
        }
    }
    companies
}

/// Distinct adapters that contributed raw candidates to a cluster.
fn cluster_adapters(cluster: &CandidateCluster) -> Vec<String> {
    let mut adapters: Vec<String> = Vec::new();
    for raw in &cluster.raw_candidates {
        if !adapters.contains(&raw.source_adapter) {
            adapters.push(raw.source_adapter.clone());
        }
    }
    adapters
}

/// Build the read-only summary handed to the arbiter for a Pass-4 decision.
fn cluster_summary(cluster: &CandidateCluster) -> ClusterSummary {
    ClusterSummary {
        names: cluster_names(cluster),
        companies: cluster_companies(cluster),
        adapters: cluster_adapters(cluster),
        identifiers: cluster.all_identifiers.clone(),
    }
}

// ---------------------------------------------------------------------------
// Canonical ID
// ---------------------------------------------------------------------------

/// Generate a deterministic, UUID-shaped canonical id from a cluster's
/// identifiers.
///
/// Faithful port of TS `generateCanonicalId`:
/// - filter to stable identifiers (exclude `name_company`); if none remain,
///   fall back to all identifiers,
/// - normalize each (`normalize_identifier_value`),
/// - dedup by `(type, value)`,
/// - sort by type then value,
/// - join as `type=value|…`, sha256, slice into the 8-4-4-4-12 UUID layout.
fn generate_canonical_id(identifiers: &[ObservedIdentifier]) -> String {
    // Stable identifiers exclude name_company (too volatile); fall back to all.
    let stable: Vec<&ObservedIdentifier> = {
        let filtered: Vec<&ObservedIdentifier> = identifiers
            .iter()
            .filter(|id| id.kind != IdentifierType::NameCompany)
            .collect();
        if filtered.is_empty() {
            identifiers.iter().collect()
        } else {
            filtered
        }
    };

    // Normalize + dedup by (type, value), preserving deterministic order via
    // a BTreeMap keyed on the canonical "type:value" string.
    let mut unique: BTreeMap<String, (String, String)> = BTreeMap::new();
    for id in stable {
        let type_str = identifier_type_token(id.kind);
        let value = normalize_identifier_value(id.kind, &id.value);
        let key = format!("{}:{}", type_str, value);
        unique.entry(key).or_insert((type_str.to_string(), value));
    }

    // BTreeMap iterates by key ("type:value") which is exactly the TS sort
    // order (type then value) for these stable strings.
    let canonical = unique
        .values()
        .map(|(t, v)| format!("{}={}", t, v))
        .collect::<Vec<_>>()
        .join("|");

    let hash = Sha256::digest(canonical.as_bytes());
    let hex = hex::encode(hash);

    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32],
    )
}

/// The wire token for an identifier type, matching the TS `id.type` string used
/// in the canonical-id pre-image (snake_case, with the TS field names).
fn identifier_type_token(kind: IdentifierType) -> &'static str {
    match kind {
        IdentifierType::Linkedin => "linkedin_url",
        IdentifierType::Email => "email",
        IdentifierType::Github => "github_username",
        IdentifierType::Twitter => "twitter_handle",
        IdentifierType::PersonalUrl => "personal_url",
        IdentifierType::NameCompany => "name_company",
    }
}

// ---------------------------------------------------------------------------
// Cluster → ResolvedCandidate
// ---------------------------------------------------------------------------

/// Pick the modal display name across a cluster's raw candidates (most common
/// `name`; ties broken by first-seen). Faithful port of TS `chooseBestName`.
fn choose_best_name(cluster: &CandidateCluster) -> String {
    let mut counts: Vec<(String, usize)> = Vec::new();
    for raw in &cluster.raw_candidates {
        let n = raw.name.trim().to_string();
        if let Some(entry) = counts.iter_mut().find(|(name, _)| *name == n) {
            entry.1 += 1;
        } else {
            counts.push((n, 1));
        }
    }
    // Default to the first candidate's name (TS seeds `best` with it), then
    // take the strictly-greater max so first-seen wins ties.
    let mut best = cluster.raw_candidates[0].name.clone();
    let mut best_count = 0usize;
    for (name, count) in &counts {
        if *count > best_count {
            best = name.clone();
            best_count = *count;
        }
    }
    best
}

/// Convert a surviving cluster into a `ResolvedCandidate`.
fn cluster_to_candidate(cluster: &CandidateCluster, low_confidence_merge: bool) -> ResolvedCandidate {
    let canonical_id = generate_canonical_id(&cluster.all_identifiers);

    // Per-adapter sources, keyed by adapter (BTreeMap = deterministic).
    let mut sources: BTreeMap<String, SourceData> = BTreeMap::new();
    for raw in &cluster.raw_candidates {
        let entry = sources
            .entry(raw.source_adapter.clone())
            .or_insert_with(|| SourceData {
                adapter: raw.source_adapter.clone(),
                urls: Vec::new(),
            });
        if !entry.urls.contains(&raw.url) {
            entry.urls.push(raw.url.clone());
        }
    }

    let was_merged = cluster.raw_candidates.len() > 1;
    let merge_confidence = if !was_merged {
        1.0
    } else if low_confidence_merge {
        0.7
    } else {
        0.95
    };

    let merged_from = if was_merged {
        Some(
            cluster
                .raw_candidates
                .iter()
                .map(|r| r.source_adapter.clone())
                .collect(),
        )
    } else {
        None
    };

    let identity = PersonIdentity {
        canonical_id: canonical_id.clone(),
        observed_identifiers: cluster.all_identifiers.clone(),
        merged_from,
        merge_confidence,
        low_confidence_merge,
    };

    ResolvedCandidate {
        id: canonical_id,
        identity,
        name: choose_best_name(cluster),
        sources,
        page_text: None,
    }
}

// ---------------------------------------------------------------------------
// The resolver
// ---------------------------------------------------------------------------

/// Resolve a discovered candidate list into canonical candidates.
///
/// `arbiter` gates Pass 4 only (`NoopArbiter` = pure-heuristic TS parity). With
/// `NoopArbiter`, this never errors; a real arbiter can surface an
/// `IdentityError` from its transport.
pub async fn resolve(
    raw_candidates: Vec<RawCandidate>,
    arbiter: &dyn IdentityArbiter,
) -> Result<ResolveResult, IdentityError> {
    if raw_candidates.is_empty() {
        return Ok(ResolveResult {
            candidates: Vec::new(),
            merge_log: Vec::new(),
            stats: ResolveStats::default(),
        });
    }

    let input_count = raw_candidates.len();

    // Build initial clusters (one per raw candidate).
    let mut clusters: Vec<CandidateCluster> = raw_candidates
        .into_iter()
        .map(|raw| {
            let all_identifiers = raw.identifiers.clone();
            CandidateCluster {
                raw_candidates: vec![raw],
                all_identifiers,
                merged: false,
                merged_into: None,
            }
        })
        .collect();

    let mut merge_log: Vec<MergeDecision> = Vec::new();
    let mut high_count = 0usize;
    let mut medium_count = 0usize;
    let mut low_count = 0usize;

    // Pass 1: high-confidence exact (linkedin/email/github).
    for decision in find_high_confidence_merges(&clusters) {
        if apply_merge(&mut clusters, &decision) {
            merge_log.push(decision);
            high_count += 1;
        }
    }

    // Pass 2: cross-source email linking.
    for decision in find_cross_source_email_merges(&clusters) {
        if apply_merge(&mut clusters, &decision) {
            merge_log.push(decision);
            high_count += 1;
        }
    }

    // Pass 3: medium-confidence (name + company, different adapters).
    for decision in find_medium_confidence_merges(&clusters) {
        if apply_merge(&mut clusters, &decision) {
            merge_log.push(decision);
            medium_count += 1;
        }
    }

    // Pass 4: low-confidence (similar name + similar company), arbiter-gated.
    // Track which surviving cluster index absorbed a low-confidence merge so
    // its candidate gets the 0.7 / low_confidence_merge flag.
    let mut low_conf_clusters: std::collections::HashSet<usize> = std::collections::HashSet::new();
    for decision in find_low_confidence_merges(&clusters) {
        // Skip if either side was already merged by an earlier Pass-4 decision.
        if clusters[decision.cluster_index_a].merged || clusters[decision.cluster_index_b].merged {
            continue;
        }
        // Arbiter gate: a `false` verdict vetoes this auto-merge.
        let summary_a = cluster_summary(&clusters[decision.cluster_index_a]);
        let summary_b = cluster_summary(&clusters[decision.cluster_index_b]);
        let approved = arbiter
            .same_person(&summary_a, &summary_b, &decision.reason)
            .await?;
        if !approved {
            continue;
        }
        if apply_merge(&mut clusters, &decision) {
            low_conf_clusters.insert(decision.cluster_index_a);
            merge_log.push(decision);
            low_count += 1;
        }
    }

    // Build final candidates from surviving (non-tombstoned) clusters.
    let mut candidates: Vec<ResolvedCandidate> = Vec::new();
    for (idx, cluster) in clusters.iter().enumerate() {
        if cluster.merged {
            continue;
        }
        let is_low = low_conf_clusters.contains(&idx);
        candidates.push(cluster_to_candidate(cluster, is_low));
    }

    let output_count = candidates.len();
    Ok(ResolveResult {
        candidates,
        merge_log,
        stats: ResolveStats {
            input_count,
            output_count,
            high_confidence_merges: high_count,
            medium_confidence_merges: medium_count,
            low_confidence_merges: low_count,
        },
    })
}

// ---------------------------------------------------------------------------
// Pass 1: high-confidence index-based merges
// ---------------------------------------------------------------------------

fn find_high_confidence_merges(clusters: &[CandidateCluster]) -> Vec<MergeDecision> {
    let mut decisions: Vec<MergeDecision> = Vec::new();
    let types_to_check: [(IdentifierType, MergeRule); 3] = [
        (IdentifierType::Linkedin, MergeRule::LinkedinUrl),
        (IdentifierType::Email, MergeRule::VerifiedEmail),
        (IdentifierType::Github, MergeRule::GithubUsername),
    ];

    for (kind, rule) in types_to_check {
        // normalized value → ordered list of cluster indices that carry it.
        let mut index: HashMap<String, Vec<usize>> = HashMap::new();
        // Preserve insertion order of keys so decision order is deterministic.
        let mut key_order: Vec<String> = Vec::new();

        for (i, cluster) in clusters.iter().enumerate() {
            if cluster.merged {
                continue;
            }
            for id in &cluster.all_identifiers {
                if id.kind != kind {
                    continue;
                }
                let normalized = normalize_identifier_value(id.kind, &id.value);
                match index.get_mut(&normalized) {
                    Some(v) => {
                        if !v.contains(&i) {
                            v.push(i);
                        }
                    }
                    None => {
                        key_order.push(normalized.clone());
                        index.insert(normalized, vec![i]);
                    }
                }
            }
        }

        for value in &key_order {
            let cluster_indices = &index[value];
            if cluster_indices.len() < 2 {
                continue;
            }
            for k in 1..cluster_indices.len() {
                let idx_a = cluster_indices[0];
                let idx_b = cluster_indices[k];
                decisions.push(MergeDecision {
                    cluster_index_a: idx_a,
                    cluster_index_b: idx_b,
                    reason: MergeReason {
                        rule,
                        confidence: ConfidenceLevel::High,
                        matched_values: [value.clone(), value.clone()],
                        description: format!("Matching {}: {}", identifier_type_token(kind), value),
                    },
                    automatic: true,
                });
            }
        }
    }

    decisions
}

// ---------------------------------------------------------------------------
// Pass 2: cross-source email linking
// ---------------------------------------------------------------------------

struct EmailEntry {
    cluster_idx: usize,
    adapters: Vec<String>,
}

fn find_cross_source_email_merges(clusters: &[CandidateCluster]) -> Vec<MergeDecision> {
    let mut decisions: Vec<MergeDecision> = Vec::new();
    // normalized email → entries (one per contributing cluster), insertion-ordered.
    let mut email_index: HashMap<String, Vec<EmailEntry>> = HashMap::new();
    let mut key_order: Vec<String> = Vec::new();

    for (i, cluster) in clusters.iter().enumerate() {
        if cluster.merged {
            continue;
        }
        for id in &cluster.all_identifiers {
            if id.kind != IdentifierType::Email {
                continue;
            }
            let normalized = normalize_email(&id.value);
            let entries = email_index.entry(normalized.clone()).or_insert_with(|| {
                key_order.push(normalized.clone());
                Vec::new()
            });
            if let Some(existing) = entries.iter_mut().find(|e| e.cluster_idx == i) {
                if !existing.adapters.contains(&id.source_adapter) {
                    existing.adapters.push(id.source_adapter.clone());
                }
            } else {
                entries.push(EmailEntry {
                    cluster_idx: i,
                    adapters: vec![id.source_adapter.clone()],
                });
            }
        }
    }

    for email in &key_order {
        let entries = &email_index[email];
        if entries.len() < 2 {
            continue;
        }
        // Require the email to come from ≥2 distinct adapters across entries.
        let mut all_adapters: Vec<&String> = Vec::new();
        for e in entries {
            for a in &e.adapters {
                if !all_adapters.contains(&a) {
                    all_adapters.push(a);
                }
            }
        }
        if all_adapters.len() < 2 {
            continue;
        }

        for k in 1..entries.len() {
            let idx_a = entries[0].cluster_idx;
            let idx_b = entries[k].cluster_idx;
            if idx_a == idx_b {
                continue;
            }
            decisions.push(MergeDecision {
                cluster_index_a: idx_a,
                cluster_index_b: idx_b,
                reason: MergeReason {
                    rule: MergeRule::CrossSourceEmail,
                    confidence: ConfidenceLevel::High,
                    matched_values: [email.clone(), email.clone()],
                    description: format!("Same email from different adapters: {}", email),
                },
                automatic: true,
            });
        }
    }

    decisions
}

// ---------------------------------------------------------------------------
// Pass 3: medium-confidence (name + company, different adapters)
// ---------------------------------------------------------------------------

fn find_medium_confidence_merges(clusters: &[CandidateCluster]) -> Vec<MergeDecision> {
    let mut decisions: Vec<MergeDecision> = Vec::new();

    for i in 0..clusters.len() {
        if clusters[i].merged {
            continue;
        }
        for j in (i + 1)..clusters.len() {
            if clusters[j].merged {
                continue;
            }

            // Must come from at least one different adapter.
            let adapters_a = cluster_adapters(&clusters[i]);
            let adapters_b = cluster_adapters(&clusters[j]);
            let has_different_adapter = adapters_b.iter().any(|a| !adapters_a.contains(a));
            if !has_different_adapter {
                continue;
            }

            let names_a = cluster_names(&clusters[i]);
            let names_b = cluster_names(&clusters[j]);
            let companies_a = cluster_companies(&clusters[i]);
            let companies_b = cluster_companies(&clusters[j]);

            if companies_a.is_empty() || companies_b.is_empty() {
                continue;
            }

            let name_match = names_a
                .iter()
                .any(|na| names_b.iter().any(|nb| names_match(na, nb)));
            if !name_match {
                continue;
            }

            let matched_company = companies_a
                .iter()
                .find(|ca| companies_b.iter().any(|cb| cb == *ca))
                .cloned();
            let matched_company = match matched_company {
                Some(c) => c,
                None => continue,
            };

            decisions.push(MergeDecision {
                cluster_index_a: i,
                cluster_index_b: j,
                reason: MergeReason {
                    rule: MergeRule::NameCompany,
                    confidence: ConfidenceLevel::Medium,
                    matched_values: [names_a[0].clone(), names_b[0].clone()],
                    description: format!("Same name + company ({})", matched_company),
                },
                automatic: true,
            });
        }
    }

    decisions
}

// ---------------------------------------------------------------------------
// Pass 4: low-confidence (similar name + similar company)
// ---------------------------------------------------------------------------

fn find_low_confidence_merges(clusters: &[CandidateCluster]) -> Vec<MergeDecision> {
    let mut decisions: Vec<MergeDecision> = Vec::new();

    for i in 0..clusters.len() {
        if clusters[i].merged {
            continue;
        }
        for j in (i + 1)..clusters.len() {
            if clusters[j].merged {
                continue;
            }

            let names_a = cluster_names(&clusters[i]);
            let names_b = cluster_names(&clusters[j]);
            let companies_a = cluster_companies(&clusters[i]);
            let companies_b = cluster_companies(&clusters[j]);

            if companies_a.is_empty() || companies_b.is_empty() {
                continue;
            }

            // Similar (within edit distance 2) but NOT exactly matching.
            let similar_name = names_a.iter().any(|na| {
                names_b
                    .iter()
                    .any(|nb| names_similar(na, nb) && !names_match(na, nb))
            });
            if !similar_name {
                continue;
            }

            // Company within edit distance 3 but not identical.
            let similar_company = companies_a.iter().any(|ca| {
                companies_b
                    .iter()
                    .any(|cb| levenshtein(ca, cb) <= 3 && ca != cb)
            });
            if !similar_company {
                continue;
            }

            decisions.push(MergeDecision {
                cluster_index_a: i,
                cluster_index_b: j,
                reason: MergeReason {
                    rule: MergeRule::SimilarNameCompany,
                    confidence: ConfidenceLevel::Low,
                    matched_values: [names_a[0].clone(), names_b[0].clone()],
                    description: "Similar name + similar company".to_string(),
                },
                automatic: true,
            });
        }
    }

    decisions
}

// ---------------------------------------------------------------------------
// Merge application
// ---------------------------------------------------------------------------

/// Fold cluster B into cluster A and tombstone B. Returns `false` (no-op) if B
/// was already merged.
///
/// Decisions are collected against the original cluster snapshot, so a later
/// decision can name an A side that an earlier decision already tombstoned (the
/// cross-type transitive case: `0←1` via LinkedIn then `1←2` via email). We
/// follow `merged_into` to A's live root and fold B there instead of silently
/// dropping B's contents into a dead cluster — strictly more correct than the
/// TS reference, which folds into the tombstoned cluster and loses the data.
fn apply_merge(clusters: &mut [CandidateCluster], decision: &MergeDecision) -> bool {
    let (a, b) = (decision.cluster_index_a, decision.cluster_index_b);
    if clusters[b].merged {
        return false;
    }
    // Resolve A to its live root by following the tombstone chain.
    let mut root_a = a;
    while clusters[root_a].merged {
        root_a = clusters[root_a]
            .merged_into
            .expect("tombstoned cluster must record its merge destination");
    }
    debug_assert!(
        !clusters[root_a].merged,
        "apply_merge: resolved root A ({}) is still tombstoned",
        root_a
    );
    // If A's root IS B, they're already the same cluster — nothing to do.
    if root_a == b {
        return false;
    }
    // Move B's contents into A's root. Take them out of B first to satisfy the
    // borrow checker (can't hold &mut to two indices at once).
    let b_raw = std::mem::take(&mut clusters[b].raw_candidates);
    let b_ids = std::mem::take(&mut clusters[b].all_identifiers);
    clusters[root_a].raw_candidates.extend(b_raw);
    clusters[root_a].all_identifiers.extend(b_ids);
    clusters[b].merged = true;
    clusters[b].merged_into = Some(root_a);
    true
}

// ===========================================================================
// Tests — ported case-for-case from the TS vitest identity-resolver suite.
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recruiting::identity::arbiter::{FakeArbiter, NoopArbiter};
    use crate::recruiting::search::candidate::{CandidateProvenance, DiscoveryVia};

    // --- Factories ---

    fn prov() -> CandidateProvenance {
        CandidateProvenance {
            via: DiscoveryVia::TierQuery,
            query_text: Some("q".into()),
            seed_url: None,
            tier_priority: Some(1),
        }
    }

    fn make_id(kind: IdentifierType, value: &str, source: &str) -> ObservedIdentifier {
        ObservedIdentifier {
            kind,
            value: value.to_string(),
            raw_value: value.to_string(),
            source_adapter: source.to_string(),
            confidence: ConfidenceLevel::High,
        }
    }

    /// Build a `RawCandidate` with the given name/adapter/identifiers.
    fn make_raw(name: &str, adapter: &str, identifiers: Vec<ObservedIdentifier>) -> RawCandidate {
        RawCandidate {
            name: name.to_string(),
            url: format!("https://example.com/{}", name.replace(' ', "-").to_lowercase()),
            exa_id: format!("exa-{}-{}", adapter, name),
            title: None,
            author: None,
            published_date: None,
            score: Some(0.8),
            text: None,
            identifiers,
            source_adapter: adapter.to_string(),
            provenance: prov(),
        }
    }

    /// A raw candidate carrying a single identifier of `kind` (source == adapter).
    fn raw_with_id(kind: IdentifierType, value: &str, adapter: &str) -> RawCandidate {
        make_raw("Jane Doe", adapter, vec![make_id(kind, value, adapter)])
    }

    /// A raw candidate with a `NameCompany` identifier `"<name> @ <company>"`.
    fn raw_named(name: &str, company: &str, adapter: &str) -> RawCandidate {
        let nc = format!("{} @ {}", name, company);
        make_raw(name, adapter, vec![make_id(IdentifierType::NameCompany, &nc, adapter)])
    }

    // --- Normalization (sanity ports; full normalizer suite lives in normalize.rs) ---

    #[test]
    fn linkedin_normalizes_to_dash_preserving_slug() {
        // Rust deviation: dash-PRESERVING slug (the merge key).
        assert_eq!(
            normalize_identifier_value(IdentifierType::Linkedin, "https://www.linkedin.com/in/sarah-chen/"),
            "sarah-chen"
        );
    }

    #[test]
    fn email_strips_gmail_dots_and_plus() {
        assert_eq!(normalize_email("sarah.chen+work@gmail.com"), "sarahchen@gmail.com");
        assert_eq!(normalize_email("sarah@googlemail.com"), "sarah@gmail.com");
        assert_eq!(normalize_email("Sarah@Example.com"), "sarah@example.com");
    }

    #[test]
    fn names_match_and_similar_basics() {
        assert!(names_match("Sarah Chen", "sarah chen"));
        assert!(names_match("Sarah Chen", "Chen Sarah"));
        assert!(!names_match("Sarah Chen", "John Smith"));
        assert!(names_similar("sarah chen", "sara chen"));
        assert!(!names_similar("sarah chen", "john smith"));
    }

    // --- Pass 1: high-confidence merges ---

    #[tokio::test]
    async fn merges_candidates_with_matching_linkedin_url() {
        // Both sides carry the normalized dash-preserving slug.
        let raw = vec![
            raw_with_id(IdentifierType::Linkedin, "sarah-chen", "exa"),
            raw_with_id(IdentifierType::Linkedin, "sarah-chen", "github"),
        ];
        let res = resolve(raw, &NoopArbiter).await.unwrap();
        assert_eq!(res.candidates.len(), 1);
        assert!(res.stats.high_confidence_merges >= 1);
        assert_eq!(res.candidates[0].identity.observed_identifiers.len(), 2);
    }

    #[tokio::test]
    async fn same_person_two_sources_merges_high_confidence() {
        let raw = vec![
            raw_with_id(IdentifierType::Linkedin, "jane-doe", "exa"),
            raw_with_id(IdentifierType::Linkedin, "jane-doe", "github"),
        ];
        let res = resolve(raw, &NoopArbiter).await.unwrap();
        assert_eq!(res.candidates.len(), 1);
        assert_eq!(res.candidates[0].identity.merge_confidence, 0.95);
        assert_eq!(res.candidates[0].sources.len(), 2);
    }

    #[tokio::test]
    async fn merges_candidates_with_matching_email() {
        let raw = vec![
            raw_with_id(IdentifierType::Email, "sarah@gmail.com", "exa"),
            raw_with_id(IdentifierType::Email, "Sarah@Gmail.com", "hunter"),
        ];
        let res = resolve(raw, &NoopArbiter).await.unwrap();
        assert_eq!(res.candidates.len(), 1);
    }

    #[tokio::test]
    async fn merges_candidates_with_matching_github_username() {
        let raw = vec![
            raw_with_id(IdentifierType::Github, "sarahchen", "exa"),
            raw_with_id(IdentifierType::Github, "https://github.com/SarahChen", "github"),
        ];
        let res = resolve(raw, &NoopArbiter).await.unwrap();
        assert_eq!(res.candidates.len(), 1);
    }

    #[tokio::test]
    async fn merges_via_gmail_dot_normalization() {
        let raw = vec![
            raw_with_id(IdentifierType::Email, "sarahchen@gmail.com", "github"),
            raw_with_id(IdentifierType::Email, "sarah.chen@gmail.com", "hunter"),
        ];
        let res = resolve(raw, &NoopArbiter).await.unwrap();
        assert_eq!(res.candidates.len(), 1);
    }

    // --- Pass 2: cross-source email linking ---

    #[tokio::test]
    async fn merges_when_same_email_observed_from_different_adapters() {
        let raw = vec![
            make_raw(
                "Sarah Chen",
                "github",
                vec![
                    make_id(IdentifierType::Github, "sarahchen", "github"),
                    make_id(IdentifierType::Email, "sarah@company.com", "github"),
                ],
            ),
            make_raw(
                "Sarah Chen",
                "hunter",
                vec![make_id(IdentifierType::Email, "sarah@company.com", "hunter")],
            ),
        ];
        let res = resolve(raw, &NoopArbiter).await.unwrap();
        assert_eq!(res.candidates.len(), 1);
        assert!(res.stats.high_confidence_merges >= 1);
    }

    // --- Pass 3: medium-confidence merges ---

    #[tokio::test]
    async fn merges_same_name_same_company_different_sources() {
        let raw = vec![
            raw_named("Sarah Chen", "Chainlink", "exa"),
            raw_named("Sarah Chen", "Chainlink", "github"),
        ];
        let res = resolve(raw, &NoopArbiter).await.unwrap();
        assert_eq!(res.candidates.len(), 1);
        assert!(res.stats.medium_confidence_merges >= 1);
    }

    #[tokio::test]
    async fn does_not_merge_same_name_different_company() {
        let raw = vec![
            raw_named("Sarah Chen", "Chainlink", "exa"),
            raw_named("Sarah Chen", "Google", "github"),
        ];
        let res = resolve(raw, &NoopArbiter).await.unwrap();
        assert_eq!(res.candidates.len(), 2);
    }

    #[tokio::test]
    async fn does_not_merge_same_name_same_company_same_adapter() {
        let raw = vec![
            raw_named("Sarah Chen", "Chainlink", "exa"),
            raw_named("Sarah Chen", "Chainlink", "exa"),
        ];
        let res = resolve(raw, &NoopArbiter).await.unwrap();
        assert_eq!(res.candidates.len(), 2);
    }

    #[tokio::test]
    async fn handles_first_last_reorder_in_medium_confidence() {
        let raw = vec![
            raw_named("Sarah Chen", "Chainlink", "exa"),
            raw_named("Chen Sarah", "Chainlink", "github"),
        ];
        let res = resolve(raw, &NoopArbiter).await.unwrap();
        assert_eq!(res.candidates.len(), 1);
    }

    // --- Pass 4: low-confidence merges (auto-merge with flag) ---

    #[tokio::test]
    async fn auto_merges_similar_name_similar_company_with_flag() {
        // "Sara" vs "Sarah" = lev 1; "Chainklnk" vs "Chainlink" = lev 1.
        let raw = vec![
            raw_named("Sara Chen", "Chainklnk", "exa"),
            raw_named("Sarah Chen", "Chainlink", "github"),
        ];
        let res = resolve(raw, &NoopArbiter).await.unwrap();
        assert_eq!(res.candidates.len(), 1);
        assert!(res.candidates[0].identity.low_confidence_merge);
        assert_eq!(res.candidates[0].identity.merge_confidence, 0.7);
        assert_eq!(res.stats.low_confidence_merges, 1);
    }

    #[tokio::test]
    async fn merge_confidence_0_95_for_high_medium_not_flagged() {
        let raw = vec![
            raw_with_id(IdentifierType::Email, "alice@example.com", "exa"),
            raw_with_id(IdentifierType::Email, "alice@example.com", "github"),
        ];
        let res = resolve(raw, &NoopArbiter).await.unwrap();
        assert_eq!(res.candidates.len(), 1);
        assert!(!res.candidates[0].identity.low_confidence_merge);
        assert_eq!(res.candidates[0].identity.merge_confidence, 0.95);
    }

    #[tokio::test]
    async fn pass4_low_conf_merge_blocked_by_arbiter_veto() {
        let raw = vec![
            raw_named("Jon Smith", "Acme Inc", "exa"),
            raw_named("John Smith", "Acme Ltd", "github"),
        ];
        let veto = FakeArbiter::returning(vec![false]);
        let res = resolve(raw, &veto).await.unwrap();
        assert_eq!(
            res.candidates.len(),
            2,
            "arbiter veto prevents the low-confidence auto-merge"
        );
        assert_eq!(res.stats.low_confidence_merges, 0);
    }

    #[tokio::test]
    async fn pass4_low_conf_merge_approved_by_arbiter() {
        // Same fixture as the veto test, but the arbiter approves → merges to 1.
        let raw = vec![
            raw_named("Jon Smith", "Acme Inc", "exa"),
            raw_named("John Smith", "Acme Ltd", "github"),
        ];
        let approve = FakeArbiter::returning(vec![true]);
        let res = resolve(raw, &approve).await.unwrap();
        assert_eq!(res.candidates.len(), 1);
        assert!(res.candidates[0].identity.low_confidence_merge);
    }

    // --- Acceptance: 3 sources merge to 1 ---

    #[tokio::test]
    async fn three_sources_merge_to_one() {
        let raw = vec![
            make_raw(
                "Sarah Chen",
                "exa",
                vec![
                    make_id(IdentifierType::Linkedin, "sarah-chen", "exa"),
                    make_id(IdentifierType::Email, "sarah@chainlink.com", "exa"),
                ],
            ),
            make_raw(
                "Sarah Chen",
                "github",
                vec![
                    make_id(IdentifierType::Github, "sarahchen", "github"),
                    make_id(IdentifierType::Email, "sarah@chainlink.com", "github"),
                ],
            ),
            make_raw(
                "Sarah Chen",
                "hunter",
                vec![make_id(IdentifierType::Email, "sarah@chainlink.com", "hunter")],
            ),
        ];
        let res = resolve(raw, &NoopArbiter).await.unwrap();
        assert_eq!(res.candidates.len(), 1);
        let c = &res.candidates[0];
        assert!(c.identity.observed_identifiers.len() >= 4);
        assert!(c.sources.contains_key("exa"));
        assert!(c.sources.contains_key("github"));
        assert!(c.sources.contains_key("hunter"));
    }

    // --- Acceptance: genuinely different people ---

    #[tokio::test]
    async fn does_not_merge_two_different_people_with_similar_names() {
        let raw = vec![
            make_raw(
                "Sarah Chen",
                "exa",
                vec![
                    make_id(IdentifierType::Linkedin, "sarah-chen", "exa"),
                    make_id(IdentifierType::Github, "sarahchen", "exa"),
                    make_id(IdentifierType::Email, "sarah@chainlink.com", "exa"),
                ],
            ),
            make_raw(
                "Sara Chen",
                "exa",
                vec![
                    make_id(IdentifierType::Linkedin, "sara-chen-google", "exa"),
                    make_id(IdentifierType::Github, "sarachen", "exa"),
                    make_id(IdentifierType::Email, "sara@google.com", "exa"),
                ],
            ),
        ];
        let res = resolve(raw, &NoopArbiter).await.unwrap();
        assert_eq!(res.candidates.len(), 2);
    }

    // --- Acceptance: idempotent / order-independent canonical id ---

    #[tokio::test]
    async fn produces_same_canonical_id_on_rerun() {
        let build = || {
            vec![
                make_raw(
                    "Sarah Chen",
                    "exa",
                    vec![
                        make_id(IdentifierType::Linkedin, "sarah-chen", "exa"),
                        make_id(IdentifierType::Email, "sarah@chainlink.com", "exa"),
                    ],
                ),
                make_raw(
                    "Sarah Chen",
                    "github",
                    vec![make_id(IdentifierType::Email, "sarah@chainlink.com", "github")],
                ),
            ]
        };
        let res1 = resolve(build(), &NoopArbiter).await.unwrap();
        let res2 = resolve(build(), &NoopArbiter).await.unwrap();
        assert_eq!(res1.candidates.len(), 1);
        assert_eq!(res2.candidates.len(), 1);
        assert_eq!(res1.candidates[0].id, res2.candidates[0].id);
    }

    #[tokio::test]
    async fn produces_same_canonical_id_regardless_of_input_order() {
        let exa = make_raw(
            "Sarah Chen",
            "exa",
            vec![make_id(IdentifierType::Linkedin, "sarah-chen", "exa")],
        );
        let github = make_raw(
            "Sarah Chen",
            "github",
            vec![
                make_id(IdentifierType::Linkedin, "sarah-chen", "github"),
                make_id(IdentifierType::Github, "sarahchen", "github"),
            ],
        );
        let res1 = resolve(vec![exa.clone(), github.clone()], &NoopArbiter)
            .await
            .unwrap();
        let res2 = resolve(vec![github, exa], &NoopArbiter).await.unwrap();
        assert_eq!(res1.candidates.len(), 1);
        assert_eq!(res2.candidates.len(), 1);
        assert_eq!(res1.candidates[0].id, res2.candidates[0].id);
    }

    // --- Edge cases ---

    #[tokio::test]
    async fn handles_empty_input() {
        let res = resolve(vec![], &NoopArbiter).await.unwrap();
        assert_eq!(res.candidates.len(), 0);
        assert_eq!(res.stats.input_count, 0);
        assert_eq!(res.stats.output_count, 0);
        assert_eq!(res.stats.high_confidence_merges, 0);
        assert_eq!(res.stats.medium_confidence_merges, 0);
        assert_eq!(res.stats.low_confidence_merges, 0);
    }

    #[tokio::test]
    async fn handles_single_candidate_no_merges() {
        let raw = vec![raw_with_id(IdentifierType::Email, "sarah@gmail.com", "exa")];
        let res = resolve(raw, &NoopArbiter).await.unwrap();
        assert_eq!(res.candidates.len(), 1);
        assert_eq!(res.merge_log.len(), 0);
        assert!(res.candidates[0].identity.merged_from.is_none());
        assert_eq!(res.candidates[0].identity.merge_confidence, 1.0);
    }

    #[tokio::test]
    async fn handles_transitive_merges() {
        // A↔B via linkedin, B↔C via email → all three merge.
        let raw = vec![
            make_raw(
                "Sarah Chen",
                "exa",
                vec![make_id(IdentifierType::Linkedin, "sarah-chen", "exa")],
            ),
            make_raw(
                "Sarah Chen",
                "github",
                vec![
                    make_id(IdentifierType::Linkedin, "sarah-chen", "github"),
                    make_id(IdentifierType::Email, "sarah@chainlink.com", "github"),
                ],
            ),
            make_raw(
                "Sarah Chen",
                "hunter",
                vec![make_id(IdentifierType::Email, "sarah@chainlink.com", "hunter")],
            ),
        ];
        let res = resolve(raw, &NoopArbiter).await.unwrap();
        assert_eq!(res.candidates.len(), 1);
        // Regression: the cross-type transitive chain (0←1 LinkedIn, then 1←2
        // email where decision-A 1 was already tombstoned) must fold hunter
        // into the live root, not silently drop it. All three sources survive.
        let c = &res.candidates[0];
        assert_eq!(c.sources.len(), 3, "all three adapters survive the transitive merge");
        assert!(c.sources.contains_key("exa"));
        assert!(c.sources.contains_key("github"));
        assert!(c.sources.contains_key("hunter"));
    }

    #[tokio::test]
    async fn handles_candidate_with_only_name_company_identifiers() {
        let raw = vec![raw_named("Sarah Chen", "Chainlink", "exa")];
        let res = resolve(raw, &NoopArbiter).await.unwrap();
        assert_eq!(res.candidates.len(), 1);
        assert!(is_uuid_shaped(&res.candidates[0].id));
    }

    #[tokio::test]
    async fn produces_uuid_formatted_canonical_id() {
        let raw = vec![raw_with_id(IdentifierType::Email, "sarah@test.com", "exa")];
        let res = resolve(raw, &NoopArbiter).await.unwrap();
        assert!(is_uuid_shaped(&res.candidates[0].id));
    }

    // --- Merge result metadata ---

    #[tokio::test]
    async fn returns_accurate_stats() {
        let raw = vec![
            raw_with_id(IdentifierType::Email, "sarah@test.com", "exa"),
            raw_with_id(IdentifierType::Email, "sarah@test.com", "github"),
            raw_with_id(IdentifierType::Email, "john@test.com", "exa"),
        ];
        let res = resolve(raw, &NoopArbiter).await.unwrap();
        assert_eq!(res.stats.input_count, 3);
        assert_eq!(res.stats.output_count, 2);
        assert!(res.stats.high_confidence_merges >= 1);
    }

    #[tokio::test]
    async fn returns_merge_log_with_reasons() {
        let raw = vec![
            raw_with_id(IdentifierType::Linkedin, "sarah-chen", "exa"),
            raw_with_id(IdentifierType::Linkedin, "sarah-chen", "github"),
        ];
        let res = resolve(raw, &NoopArbiter).await.unwrap();
        assert_eq!(res.merge_log.len(), 1);
        assert_eq!(res.merge_log[0].reason.rule, MergeRule::LinkedinUrl);
        assert_eq!(res.merge_log[0].reason.confidence, ConfidenceLevel::High);
        assert!(res.merge_log[0].automatic);
    }

    // --- helpers ---

    fn is_uuid_shaped(id: &str) -> bool {
        let parts: Vec<&str> = id.split('-').collect();
        parts.len() == 5
            && parts[0].len() == 8
            && parts[1].len() == 4
            && parts[2].len() == 4
            && parts[3].len() == 4
            && parts[4].len() == 12
            && id.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
    }
}
