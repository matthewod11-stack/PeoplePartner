//! First-class evidence items with deterministic IDs (TS `core/evidence.ts`).

use crate::recruiting::identity::types::ResolvedCandidate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Confidence { Low, Medium, High }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceItem {
    pub id: String,
    pub claim: String,
    pub source: String,
    pub adapter: String,
    pub retrieved_at: String,
    pub confidence: Confidence,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub url: Option<String>,
}

#[derive(Debug)]
pub struct EvidenceIdInput<'a> {
    pub adapter: &'a str,
    pub source: &'a str,
    pub claim: &'a str,
    pub retrieved_at: &'a str,
}

/// Faithful port of the TS djb2 hash: iterate UTF-16 code units, accumulate in
/// 32-bit signed wrapping arithmetic (`| 0`), read back unsigned (`>>> 0`),
/// hex, left-pad to 6, take the last 6.
pub fn generate_evidence_id(input: &EvidenceIdInput) -> String {
    let raw = format!("{}:{}:{}:{}", input.adapter, input.source, input.claim, input.retrieved_at);
    let mut hash: i32 = 5381;
    for unit in raw.encode_utf16() {
        hash = hash
            .wrapping_shl(5)
            .wrapping_add(hash)
            .wrapping_add(unit as i32);
    }
    let hex = format!("{:06x}", hash as u32);
    let last6 = &hex[hex.len() - 6..];
    format!("ev-{last6}")
}

/// Build evidence from an enriched candidate (post-hoc; reads what survives
/// identity resolution). Discovery-claim per URL + one content-claim from
/// `page_text` when it carries > 100 chars. IDs deduped.
pub fn build_evidence(candidate: &ResolvedCandidate, retrieved_at: &str) -> Vec<EvidenceItem> {
    let mut out: Vec<EvidenceItem> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    let urls: Vec<&String> = candidate.sources.values().flat_map(|s| s.urls.iter()).collect();

    for url in &urls {
        let claim = format!("Discovered candidate profile: {url}");
        push_unique(&mut out, &mut seen, EvidenceItem {
            id: generate_evidence_id(&EvidenceIdInput { adapter: "exa", source: url, claim: &claim, retrieved_at }),
            claim,
            source: (*url).clone(),
            adapter: "exa".into(),
            retrieved_at: retrieved_at.into(),
            confidence: Confidence::Medium,
            url: Some((*url).clone()),
        });
    }

    if let (Some(text), Some(primary)) = (&candidate.page_text, urls.first()) {
        if text.chars().count() > 100 {
            let snippet: String = text.chars().take(200).collect::<String>().replace('\n', " ").trim().to_string();
            let claim = format!("Page content: {snippet}");
            push_unique(&mut out, &mut seen, EvidenceItem {
                id: generate_evidence_id(&EvidenceIdInput { adapter: "exa", source: primary, claim: &claim, retrieved_at }),
                claim,
                source: (*primary).clone(),
                adapter: "exa".into(),
                retrieved_at: retrieved_at.into(),
                confidence: Confidence::Medium,
                url: Some((*primary).clone()),
            });
        }
    }
    out
}

fn push_unique(out: &mut Vec<EvidenceItem>, seen: &mut std::collections::HashSet<String>, item: EvidenceItem) {
    if seen.insert(item.id.clone()) {
        out.push(item);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recruiting::identity::types::{PersonIdentity, SourceData};
    use std::collections::BTreeMap;

    fn input<'a>(claim: &'a str) -> EvidenceIdInput<'a> {
        EvidenceIdInput { adapter: "exa", source: "https://x.com/a", claim, retrieved_at: "2026-01-01T00:00:00Z" }
    }

    #[test]
    fn id_has_ev_prefix_and_six_hex() {
        let id = generate_evidence_id(&input("found via search"));
        assert!(id.starts_with("ev-"), "got {id}");
        let hex = &id[3..];
        assert_eq!(hex.len(), 6);
        assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn id_is_deterministic() {
        assert_eq!(generate_evidence_id(&input("same")), generate_evidence_id(&input("same")));
    }

    #[test]
    fn different_claims_differ() {
        assert_ne!(generate_evidence_id(&input("a")), generate_evidence_id(&input("b")));
    }

    #[test]
    fn matches_ts_reference_vector() {
        // Captured from ~/Projects/Sourcerer generateEvidenceId (TS oracle).
        let id = generate_evidence_id(&input("found via search"));
        assert_eq!(id, "ev-7f7a0b");
    }

    fn resolved(urls: &[&str], page_text: Option<&str>) -> ResolvedCandidate {
        let mut sources = BTreeMap::new();
        sources.insert("exa".to_string(), SourceData {
            adapter: "exa".to_string(),
            urls: urls.iter().map(|u| u.to_string()).collect(),
        });
        ResolvedCandidate {
            id: "person-1".into(),
            identity: PersonIdentity {
                canonical_id: "person-1".into(),
                observed_identifiers: vec![],
                merged_from: None,
                merge_confidence: 1.0,
                low_confidence_merge: false,
            },
            name: "Jane Dev".into(),
            sources,
            page_text: page_text.map(|s| s.to_string()),
        }
    }

    #[test]
    fn build_evidence_emits_discovery_claim_per_url() {
        let c = resolved(&["https://github.com/jane", "https://x.com/jane"], None);
        let ev = build_evidence(&c, "2026-06-04T00:00:00Z");
        assert_eq!(ev.len(), 2);
        assert!(ev.iter().all(|e| e.claim.starts_with("Discovered candidate profile:")));
        assert!(ev.iter().all(|e| e.adapter == "exa" && e.confidence == Confidence::Medium));
    }

    #[test]
    fn build_evidence_adds_content_claim_when_text_long() {
        let long = "Senior Rust engineer. ".repeat(20); // > 100 chars
        let c = resolved(&["https://github.com/jane"], Some(&long));
        let ev = build_evidence(&c, "2026-06-04T00:00:00Z");
        assert_eq!(ev.len(), 2, "1 discovery + 1 content");
        assert!(ev.iter().any(|e| e.claim.starts_with("Page content:")));
    }

    #[test]
    fn build_evidence_skips_content_claim_when_text_short() {
        let c = resolved(&["https://github.com/jane"], Some("too short"));
        let ev = build_evidence(&c, "2026-06-04T00:00:00Z");
        assert_eq!(ev.len(), 1, "discovery only — short text yields no content claim");
    }

    #[test]
    fn build_evidence_dedups_by_id() {
        let c = resolved(&["https://github.com/jane", "https://github.com/jane"], None);
        let ev = build_evidence(&c, "2026-06-04T00:00:00Z");
        assert_eq!(ev.len(), 1, "same url → same id → deduped");
    }
}
