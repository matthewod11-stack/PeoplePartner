//! Evidence grounding validation (TS `grounding-validator.ts`). Strips phantom
//! evidence IDs, drops red flags citing fakes, reduces confidence proportionally,
//! and applies the H-9 score penalty. Pure — no LLM.
//!
//! FHR-105: the citation-validation core (split cited IDs into valid/phantom
//! against the canonical set) lives in the shared `crate::grounding` module;
//! this file layers recruiting's scoring semantics — confidence adjustment and
//! the H-9 penalty — on top. Those stay here by design: briefs have no scores.

use std::collections::HashSet;

use crate::grounding::{split_citations, CitationCarrying};

use super::schemas::{ExtractedSignals, SignalDimension};

/// Per-hallucination floor: each fabricated citation costs ≥15% of the
/// dimension score, regardless of how many real citations dilute it (H-9).
pub const HALLUCINATION_PENALTY_FLOOR: f64 = 0.15;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViolationAction {
    Removed,
    RedFlagDropped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroundingViolation {
    pub dimension: String,
    pub invalid_id: String,
    pub action: ViolationAction,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GroundingResult {
    pub validated: ExtractedSignals,
    pub violations: Vec<GroundingViolation>,
}

/// H-9: `min(1, max(FLOOR * hallucinated, hallucinated / totalCited))`. 0 when clean.
pub fn compute_hallucination_penalty(hallucinated_count: u32, total_cited_count: u32) -> f64 {
    if hallucinated_count == 0 || total_cited_count == 0 {
        return 0.0;
    }
    let proportional = hallucinated_count as f64 / total_cited_count as f64;
    let floor = HALLUCINATION_PENALTY_FLOOR * hallucinated_count as f64;
    proportional.max(floor).min(1.0)
}

fn validate_dimension(
    name: &str,
    dim: &SignalDimension,
    canonical_ids: &HashSet<String>,
    violations: &mut Vec<GroundingViolation>,
) -> SignalDimension {
    let original_count = dim.evidence_ids.len();
    let split = split_citations(&dim.evidence_ids, canonical_ids);
    for id in &split.phantom_ids {
        violations.push(GroundingViolation {
            dimension: name.to_string(),
            invalid_id: id.clone(),
            action: ViolationAction::Removed,
        });
    }
    let valid_ids = split.valid_ids;

    let ratio = if original_count > 0 {
        valid_ids.len() as f64 / original_count as f64
    } else {
        1.0
    };
    let adjusted_confidence = dim.confidence * ratio;

    let hallucinated_count = (original_count - valid_ids.len()) as u32;
    let penalty = compute_hallucination_penalty(hallucinated_count, original_count as u32);
    let adjusted_score = dim.score * (1.0 - penalty);

    SignalDimension {
        score: adjusted_score,
        evidence_ids: valid_ids,
        confidence: adjusted_confidence,
        hallucination_penalty: if hallucinated_count > 0 {
            Some(super::schemas::HallucinationPenalty {
                hallucinated_count,
                total_cited_count: original_count as u32,
                penalty_applied: penalty,
                raw_score_before_penalty: dim.score,
            })
        } else {
            None
        },
    }
}

/// Strip phantom evidence IDs from all dimensions + red flags. Pure: returns a
/// new `ExtractedSignals`; the input is borrowed and unchanged.
pub fn validate_grounding(
    signals: &ExtractedSignals,
    canonical_ids: &HashSet<String>,
) -> GroundingResult {
    let mut violations = Vec::new();

    // Match TS object-literal evaluation order: dimensions first (push 'removed'),
    // then red flags (push 'red_flag_dropped'). S2.4 differential eval depends on
    // this ordering matching the TS golden set.
    let technical_depth = validate_dimension(
        "technicalDepth",
        &signals.technical_depth,
        canonical_ids,
        &mut violations,
    );
    let domain_relevance = validate_dimension(
        "domainRelevance",
        &signals.domain_relevance,
        canonical_ids,
        &mut violations,
    );
    let trajectory_match = validate_dimension(
        "trajectoryMatch",
        &signals.trajectory_match,
        canonical_ids,
        &mut violations,
    );
    let culture_fit = validate_dimension(
        "cultureFit",
        &signals.culture_fit,
        canonical_ids,
        &mut violations,
    );
    let reachability = validate_dimension(
        "reachability",
        &signals.reachability,
        canonical_ids,
        &mut violations,
    );

    let red_flags = signals
        .red_flags
        .iter()
        .filter(|flag| {
            if canonical_ids.contains(&flag.evidence_id) {
                true
            } else {
                violations.push(GroundingViolation {
                    dimension: "redFlags".to_string(),
                    invalid_id: flag.evidence_id.clone(),
                    action: ViolationAction::RedFlagDropped,
                });
                false
            }
        })
        .cloned()
        .collect();

    let validated = ExtractedSignals {
        technical_depth,
        domain_relevance,
        trajectory_match,
        culture_fit,
        reachability,
        red_flags,
        prompt_versions: signals.prompt_versions.clone(),
    };

    GroundingResult {
        validated,
        violations,
    }
}

/// FHR-105: `ExtractedSignals` asserts citations in TS object-literal source
/// order — the five dimensions, then red flags — matching the violation order
/// `validate_grounding` records (S2.4 differential-eval parity).
impl CitationCarrying for ExtractedSignals {
    fn cited_ids(&self) -> Vec<String> {
        let dims = [
            &self.technical_depth,
            &self.domain_relevance,
            &self.trajectory_match,
            &self.culture_fit,
            &self.reachability,
        ];
        dims.iter()
            .flat_map(|d| d.evidence_ids.iter().cloned())
            .chain(self.red_flags.iter().map(|f| f.evidence_id.clone()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recruiting::scoring::schemas::{RedFlag, Severity};

    #[test]
    fn penalty_zero_when_no_hallucinations() {
        assert_eq!(compute_hallucination_penalty(0, 5), 0.0);
        assert_eq!(compute_hallucination_penalty(0, 0), 0.0);
    }

    #[test]
    fn penalty_proportional_for_small_slip() {
        // 1 of 5 fake → 20% (proportional beats the 15% floor).
        assert!((compute_hallucination_penalty(1, 5) - 0.20).abs() < 1e-9);
    }

    #[test]
    fn penalty_floor_dominates_padding_attack() {
        // 1 of 50 fake → 15% floor, not 2% proportional.
        assert!((compute_hallucination_penalty(1, 50) - 0.15).abs() < 1e-9);
    }

    #[test]
    fn penalty_capped_at_one() {
        // 10 fake × 0.15 floor = 1.5 → capped at 1.0.
        assert_eq!(compute_hallucination_penalty(10, 10), 1.0);
    }

    fn dim(score: f64, conf: f64, ids: &[&str]) -> SignalDimension {
        SignalDimension {
            score,
            confidence: conf,
            evidence_ids: ids.iter().map(|s| s.to_string()).collect(),
            hallucination_penalty: None,
        }
    }
    fn canon(ids: &[&str]) -> HashSet<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }
    fn signals(td: SignalDimension, red: Vec<RedFlag>) -> ExtractedSignals {
        ExtractedSignals {
            technical_depth: td,
            domain_relevance: dim(70.0, 0.8, &["ev-1"]),
            trajectory_match: dim(60.0, 0.7, &["ev-1"]),
            culture_fit: dim(50.0, 0.6, &["ev-1"]),
            reachability: dim(40.0, 0.5, &["ev-1"]),
            red_flags: red,
            prompt_versions: None,
        }
    }

    #[test]
    fn strips_phantom_ids_and_records_violation() {
        let s = signals(dim(80.0, 1.0, &["ev-1", "ev-fake"]), vec![]);
        let r = validate_grounding(&s, &canon(&["ev-1"]));
        assert_eq!(
            r.validated.technical_depth.evidence_ids,
            vec!["ev-1".to_string()]
        );
        assert_eq!(
            r.violations
                .iter()
                .filter(|v| v.invalid_id == "ev-fake")
                .count(),
            1
        );
    }

    #[test]
    fn keeps_valid_red_flags() {
        let s = signals(
            dim(80.0, 1.0, &["ev-1"]),
            vec![RedFlag {
                signal: "gap".into(),
                evidence_id: "ev-1".into(),
                severity: Severity::Low,
            }],
        );
        let r = validate_grounding(&s, &canon(&["ev-1"]));
        assert_eq!(r.validated.red_flags.len(), 1);
    }

    #[test]
    fn drops_red_flags_with_fake_ids() {
        let s = signals(
            dim(80.0, 1.0, &["ev-1"]),
            vec![RedFlag {
                signal: "x".into(),
                evidence_id: "ev-fake".into(),
                severity: Severity::High,
            }],
        );
        let r = validate_grounding(&s, &canon(&["ev-1"]));
        assert!(r.validated.red_flags.is_empty());
        assert!(r
            .violations
            .iter()
            .any(|v| v.action == ViolationAction::RedFlagDropped));
    }

    #[test]
    fn empty_canonical_set_does_not_crash() {
        let s = signals(dim(80.0, 1.0, &["ev-1"]), vec![]);
        let r = validate_grounding(&s, &canon(&[]));
        assert!(r.validated.technical_depth.evidence_ids.is_empty());
    }

    #[test]
    fn does_not_mutate_input() {
        let s = signals(dim(80.0, 1.0, &["ev-1", "ev-fake"]), vec![]);
        let before = s.clone();
        let _ = validate_grounding(&s, &canon(&["ev-1"]));
        assert_eq!(s, before, "validate_grounding must be pure");
    }

    #[test]
    fn no_hallucination_metadata_on_clean_dimension() {
        let s = signals(dim(80.0, 1.0, &["ev-1"]), vec![]);
        let r = validate_grounding(&s, &canon(&["ev-1"]));
        assert!(r.validated.technical_depth.hallucination_penalty.is_none());
    }

    #[test]
    fn preserves_duplicate_valid_citations() {
        let s = signals(dim(80.0, 1.0, &["ev-1", "ev-1"]), vec![]);
        let r = validate_grounding(&s, &canon(&["ev-1"]));
        assert_eq!(
            r.validated.technical_depth.evidence_ids,
            vec!["ev-1".to_string(), "ev-1".to_string()]
        );
    }

    #[test]
    fn applies_confidence_and_score_adjustment() {
        // 1 of 2 fake → confidence ×0.5; score ×(1 - max(0.5, 0.15)) = ×0.5.
        let s = signals(dim(80.0, 1.0, &["ev-1", "ev-fake"]), vec![]);
        let r = validate_grounding(&s, &canon(&["ev-1"]));
        let d = &r.validated.technical_depth;
        assert!((d.confidence - 0.5).abs() < 1e-9);
        assert!((d.score - 40.0).abs() < 1e-9);
        let hp = d
            .hallucination_penalty
            .as_ref()
            .expect("penalty metadata attached");
        assert_eq!(hp.hallucinated_count, 1);
        assert_eq!(hp.total_cited_count, 2);
        assert!((hp.raw_score_before_penalty - 80.0).abs() < 1e-9);
    }

    #[test]
    fn violation_order_matches_ts_dimensions_before_red_flags() {
        // TS pushes dimension 'removed' violations before red-flag drops (object-literal
        // source order). Pin it so S2.4's differential eval stays byte-parity with TS.
        let s = signals(
            dim(80.0, 1.0, &["ev-1", "ev-fake-dim"]),
            vec![RedFlag {
                signal: "x".into(),
                evidence_id: "ev-fake-flag".into(),
                severity: Severity::High,
            }],
        );
        let r = validate_grounding(&s, &canon(&["ev-1"]));
        let dim_pos = r
            .violations
            .iter()
            .position(|v| v.invalid_id == "ev-fake-dim")
            .expect("dim violation present");
        let flag_pos = r
            .violations
            .iter()
            .position(|v| v.invalid_id == "ev-fake-flag")
            .expect("red-flag violation present");
        assert!(
            dim_pos < flag_pos,
            "dimension removal must precede red-flag drop (TS parity)"
        );
    }

    #[test]
    fn citation_carrying_impl_surfaces_phantoms_across_dims_and_flags() {
        // FHR-105: the shared trait sees every asserted citation — dimension
        // evidence in source order, then red-flag evidence.
        let s = signals(
            dim(80.0, 1.0, &["ev-1", "ev-fake-dim"]),
            vec![RedFlag {
                signal: "x".into(),
                evidence_id: "ev-fake-flag".into(),
                severity: Severity::High,
            }],
        );
        let phantoms = crate::grounding::phantom_citations(&s, &canon(&["ev-1"]));
        assert_eq!(
            phantoms,
            vec!["ev-fake-dim".to_string(), "ev-fake-flag".to_string()]
        );
        assert!(!crate::grounding::is_fully_grounded(&s, &canon(&["ev-1"])));
    }

    #[test]
    fn accumulates_violations_across_dimensions_in_source_order() {
        // Fakes in technicalDepth (1st dim) AND reachability (5th dim) + a fake red flag.
        // The flat violations list must be: dim removals in source order, then red-flag drops.
        // This is the exact shape S2.4 (FHR-80) diffs against the TS golden set.
        let s = ExtractedSignals {
            technical_depth: dim(80.0, 1.0, &["ev-1", "ev-fake-td"]),
            domain_relevance: dim(70.0, 0.8, &["ev-1"]),
            trajectory_match: dim(60.0, 0.7, &["ev-1"]),
            culture_fit: dim(50.0, 0.6, &["ev-1"]),
            reachability: dim(40.0, 0.5, &["ev-1", "ev-fake-reach"]),
            red_flags: vec![RedFlag {
                signal: "x".into(),
                evidence_id: "ev-fake-flag".into(),
                severity: Severity::High,
            }],
            prompt_versions: None,
        };
        let r = validate_grounding(&s, &canon(&["ev-1"]));
        let dims: Vec<&str> = r.violations.iter().map(|v| v.dimension.as_str()).collect();
        assert_eq!(dims, vec!["technicalDepth", "reachability", "redFlags"]);
        assert_eq!(
            r.violations
                .iter()
                .filter(|v| v.action == ViolationAction::Removed)
                .count(),
            2
        );
        assert_eq!(
            r.violations
                .iter()
                .filter(|v| v.action == ViolationAction::RedFlagDropped)
                .count(),
            1
        );
    }
}
