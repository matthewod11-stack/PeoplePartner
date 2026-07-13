//! Prep-Brief schema (FHR-106, People Map T5).
//!
//! Two sections, visibly separated in the rendered artifact: Facts (each
//! cited to exactly one grounding item) and Threads (each anchored to a
//! named, cited fact; inference by definition and labeled as such in the
//! UI). A thin record produces fewer or no threads plus an explicit note —
//! never filler (decision 7).
//!
//! Decision 6 is structural: this schema carries no numeric or ordinal
//! assessment of the employee, and the lock test below keeps the assessment
//! vocabulary out of this module's entire source.

use serde::{Deserialize, Serialize};

use crate::grounding::CitationCarrying;

/// One grounded statement, cited to exactly one grounding-context item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BriefFact {
    pub text: String,
    /// Citation ID of the grounding item this fact restates (e.g. `C3`).
    pub citation_id: String,
}

/// One conversation opener, anchored to a named fact. Threads are inference
/// by definition — the UI renders them under an explicit inference label.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BriefThread {
    /// Citation ID of the fact this thread builds on.
    pub anchor_citation_id: String,
    /// The named fact, restated so the anchor is legible without a lookup.
    pub anchor_fact: String,
    /// The question to ask, or topic this person is well placed to speak on.
    pub question: String,
}

/// An ephemeral pre-meeting brief. Rendered on demand, regenerable, never
/// persisted (decision 9) — the audit row is the only durable trace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrepBrief {
    pub employee_id: String,
    pub facts: Vec<BriefFact>,
    /// At most 2–3, gated by the grounding floor; empty on a thin record.
    pub threads: Vec<BriefThread>,
    /// Present when the grounding floor wasn't met: an explicit statement
    /// that the record is too thin to anchor threads (decision 7 wording
    /// lives in the prompt template).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub thin_record_note: Option<String>,
}

/// A brief asserts its facts' citations first (source order), then its
/// threads' anchors — the order they appear in the rendered artifact.
impl CitationCarrying for PrepBrief {
    fn cited_ids(&self) -> Vec<String> {
        self.facts
            .iter()
            .map(|f| f.citation_id.clone())
            .chain(self.threads.iter().map(|t| t.anchor_citation_id.clone()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grounding::{is_fully_grounded, phantom_citations};
    use std::collections::HashSet;

    fn sample_brief() -> PrepBrief {
        PrepBrief {
            employee_id: "emp-1".into(),
            facts: vec![
                BriefFact {
                    text: "Led the API gateway migration with zero downtime.".into(),
                    citation_id: "C1".into(),
                },
                BriefFact {
                    text: "Mentored six junior engineers this cycle.".into(),
                    citation_id: "C2".into(),
                },
            ],
            threads: vec![BriefThread {
                anchor_citation_id: "C1".into(),
                anchor_fact: "Led the API gateway migration.".into(),
                question:
                    "What made the zero-downtime cutover work — worth writing up for the team?"
                        .into(),
            }],
            thin_record_note: None,
        }
    }

    fn canon(ids: &[&str]) -> HashSet<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn serde_round_trips_a_full_brief() {
        let brief = sample_brief();
        let json = serde_json::to_string(&brief).expect("serialize");
        let back: PrepBrief = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(brief, back);
    }

    #[test]
    fn serde_uses_camel_case_field_names() {
        let json = serde_json::to_string(&sample_brief()).expect("serialize");
        assert!(json.contains("\"employeeId\""));
        assert!(json.contains("\"citationId\""));
        assert!(json.contains("\"anchorCitationId\""));
        assert!(json.contains("\"anchorFact\""));
        // thin_record_note is None → omitted entirely.
        assert!(!json.contains("thinRecordNote"));
    }

    #[test]
    fn serde_round_trips_a_thin_record_brief() {
        let brief = PrepBrief {
            employee_id: "emp-2".into(),
            facts: vec![BriefFact {
                text: "Joined as HR Coordinator in December 2025.".into(),
                citation_id: "C1".into(),
            }],
            threads: vec![],
            thin_record_note: Some(
                "This record is too thin to anchor conversation threads.".into(),
            ),
        };
        let json = serde_json::to_string(&brief).expect("serialize");
        assert!(json.contains("\"thinRecordNote\""));
        let back: PrepBrief = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(brief, back);
        assert!(back.threads.is_empty());
    }

    #[test]
    fn brief_asserts_citations_facts_first_then_thread_anchors() {
        let ids = sample_brief().cited_ids();
        assert_eq!(
            ids,
            vec!["C1".to_string(), "C2".to_string(), "C1".to_string()]
        );
    }

    #[test]
    fn shared_grounding_helpers_validate_a_brief() {
        let brief = sample_brief();
        assert!(is_fully_grounded(&brief, &canon(&["C1", "C2"])));
        // Drop C2 from the canonical set → the second fact becomes a phantom.
        let phantoms = phantom_citations(&brief, &canon(&["C1"]));
        assert_eq!(phantoms, vec!["C2".to_string()]);
    }

    /// Decision 6 lock: the assessment vocabulary must not appear anywhere in
    /// this module's source — field names, comments, or strings. Needles are
    /// built at runtime so this test's own source can't satisfy them.
    /// New people_map files MUST be added to the manifest below.
    #[test]
    fn never_assessment_vocabulary_in_people_map_source() {
        let manifest: [(&str, &str); 2] = [
            ("mod.rs", include_str!("mod.rs")),
            ("schema.rs", include_str!("schema.rs")),
        ];
        let needles: Vec<String> = ["sco", "ran", "rat", "ris", "tie", "gra"]
            .iter()
            .zip(["re", "k", "ing", "k", "r", "de"])
            .map(|(a, b)| format!("{}{}", a, b))
            .collect();
        for (file, src) in manifest {
            let lower = src.to_lowercase();
            for needle in &needles {
                assert!(
                    !lower.contains(needle.as_str()),
                    "{file} contains the assessment token {needle:?} — People Map never assesses employees (decision 6)"
                );
            }
        }
    }
}
