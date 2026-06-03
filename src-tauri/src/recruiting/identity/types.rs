//! Identity identifier types for the Sourcerer module (FHR-73 S1.1).
//!
//! Identifier subset only — full `PersonIdentity`, `ResolvedCandidate`, and
//! merge types land in Task 4 when `resolve()` is implemented.

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
