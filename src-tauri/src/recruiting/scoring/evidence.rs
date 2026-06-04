//! First-class evidence items with deterministic IDs (TS `core/evidence.ts`).

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
