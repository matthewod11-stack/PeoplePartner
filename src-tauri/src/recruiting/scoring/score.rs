//! Deterministic score calculator + tier assignment (TS `score-calculator.ts`).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::schemas::{ExtractedSignals, RedFlag, Severity};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoringWeights {
    pub technical_depth: f64,
    pub domain_relevance: f64,
    pub trajectory_match: f64,
    pub culture_fit: f64,
    pub reachability: f64,
}
impl Default for ScoringWeights {
    fn default() -> Self {
        Self {
            technical_depth: 0.30,
            domain_relevance: 0.25,
            trajectory_match: 0.20,
            culture_fit: 0.15,
            reachability: 0.10,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TierThresholds {
    pub tier1_min_score: f64,
    pub tier2_min_score: f64,
}
impl Default for TierThresholds {
    fn default() -> Self {
        Self {
            tier1_min_score: 70.0,
            tier2_min_score: 40.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedFlagPenalties {
    pub low: f64,
    pub medium: f64,
    pub high: f64,
}
impl Default for RedFlagPenalties {
    fn default() -> Self {
        Self {
            low: 2.0,
            medium: 5.0,
            high: 10.0,
        }
    }
}
impl RedFlagPenalties {
    /// Returns the penalty value configured for the given red-flag severity.
    pub fn for_severity(&self, severity: Severity) -> f64 {
        match severity {
            Severity::Low => self.low,
            Severity::Medium => self.medium,
            Severity::High => self.high,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ScoringConfig {
    pub weights: ScoringWeights,
    pub tier_thresholds: TierThresholds,
    pub red_flag_penalties: RedFlagPenalties,
}

/// Candidate tier — serialized as the integer `1 | 2 | 3` (wire-compatible with
/// the TS golden diff), type-safe in Rust.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(into = "u8", try_from = "u8")]
pub enum Tier {
    Tier1,
    Tier2,
    Tier3,
}

impl From<Tier> for u8 {
    fn from(t: Tier) -> u8 {
        match t {
            Tier::Tier1 => 1,
            Tier::Tier2 => 2,
            Tier::Tier3 => 3,
        }
    }
}
impl TryFrom<u8> for Tier {
    type Error = String;
    fn try_from(v: u8) -> Result<Tier, String> {
        match v {
            1 => Ok(Tier::Tier1),
            2 => Ok(Tier::Tier2),
            3 => Ok(Tier::Tier3),
            other => Err(format!("invalid tier: {other}")),
        }
    }
}

/// Cascading inclusive thresholds (TS `assignTier`).
pub fn assign_tier(total: f64, thresholds: &TierThresholds) -> Tier {
    if total >= thresholds.tier1_min_score {
        Tier::Tier1
    } else if total >= thresholds.tier2_min_score {
        Tier::Tier2
    } else {
        Tier::Tier3
    }
}

/// One dimension's contribution to the total (TS `ScoreComponent`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoreComponent {
    pub dimension: String,
    pub raw: f64,
    pub weight: f64,
    pub weighted: f64,
    pub evidence_ids: Vec<String>,
    pub confidence: f64,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub hallucination_penalty: Option<super::schemas::HallucinationPenalty>,
}

/// Deterministic weighted score (TS `Score`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Score {
    pub total: f64,
    pub breakdown: Vec<ScoreComponent>,
    pub weights: ScoringWeights,
    pub red_flags: Vec<RedFlag>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub prompt_versions: Option<HashMap<String, u32>>,
}

/// Compute a candidate's weighted score from already-grounded signals.
///
/// Faithful to TS `scoreCandidate`: per dimension `weighted = score × weight ×
/// confidence`, summed in the fixed dimension order, minus red-flag penalties,
/// clamped to [0, 100]. The H-9 hallucination penalty is **not** re-applied here
/// — grounding (FHR-78) already folded it into `score`/`confidence`; the metadata
/// rides along in `ScoreComponent` for transparency.
pub fn score_candidate(signals: &ExtractedSignals, config: &ScoringConfig) -> Score {
    // MUST match ExtractedSignals::dimensions() order.
    let weights = [
        config.weights.technical_depth,
        config.weights.domain_relevance,
        config.weights.trajectory_match,
        config.weights.culture_fit,
        config.weights.reachability,
    ];
    let mut breakdown = Vec::with_capacity(5);
    let mut raw_total = 0.0_f64;
    for (i, (name, dim)) in signals.dimensions().into_iter().enumerate() {
        let weight = weights[i];
        let weighted = dim.score * weight * dim.confidence;
        raw_total += weighted;
        breakdown.push(ScoreComponent {
            dimension: name.to_string(),
            raw: dim.score,
            weight,
            weighted,
            evidence_ids: dim.evidence_ids.clone(),
            confidence: dim.confidence,
            hallucination_penalty: dim.hallucination_penalty.clone(),
        });
    }
    let total_penalty: f64 = signals
        .red_flags
        .iter()
        .map(|f| config.red_flag_penalties.for_severity(f.severity))
        .sum();
    let total = (raw_total - total_penalty).clamp(0.0, 100.0);
    Score {
        total,
        breakdown,
        weights: config.weights,
        red_flags: signals.red_flags.clone(),
        prompt_versions: signals.prompt_versions.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_benchmark_constants() {
        let c = ScoringConfig::default();
        assert_eq!(c.weights.technical_depth, 0.30);
        assert_eq!(c.weights.domain_relevance, 0.25);
        assert_eq!(c.weights.trajectory_match, 0.20);
        assert_eq!(c.weights.culture_fit, 0.15);
        assert_eq!(c.weights.reachability, 0.10);
        assert_eq!(c.tier_thresholds.tier1_min_score, 70.0);
        assert_eq!(c.tier_thresholds.tier2_min_score, 40.0);
        assert_eq!(c.red_flag_penalties.for_severity(Severity::Low), 2.0);
        assert_eq!(c.red_flag_penalties.for_severity(Severity::Medium), 5.0);
        assert_eq!(c.red_flag_penalties.for_severity(Severity::High), 10.0);
        assert_eq!(c.weights, ScoringWeights::default());
        assert_eq!(c.tier_thresholds, TierThresholds::default());
        assert_eq!(c.red_flag_penalties, RedFlagPenalties::default());
    }

    #[test]
    fn assign_tier_boundaries_are_inclusive() {
        let t = TierThresholds::default(); // 70 / 40
        assert_eq!(assign_tier(70.0, &t), Tier::Tier1);
        assert_eq!(assign_tier(69.999, &t), Tier::Tier2);
        assert_eq!(assign_tier(40.0, &t), Tier::Tier2);
        assert_eq!(assign_tier(39.999, &t), Tier::Tier3);
        assert_eq!(assign_tier(100.0, &t), Tier::Tier1);
        assert_eq!(assign_tier(0.0, &t), Tier::Tier3);
    }

    #[test]
    fn assign_tier_honors_golden_config_thresholds() {
        let t = TierThresholds {
            tier1_min_score: 72.0,
            tier2_min_score: 48.0,
        };
        assert_eq!(assign_tier(72.0, &t), Tier::Tier1);
        assert_eq!(assign_tier(71.999, &t), Tier::Tier2);
        assert_eq!(assign_tier(48.0, &t), Tier::Tier2);
        assert_eq!(assign_tier(47.999, &t), Tier::Tier3);
    }

    #[test]
    fn tier_serializes_to_integer() {
        assert_eq!(
            serde_json::to_value(Tier::Tier1).unwrap(),
            serde_json::json!(1)
        );
        assert_eq!(
            serde_json::to_value(Tier::Tier3).unwrap(),
            serde_json::json!(3)
        );
        let back: Tier = serde_json::from_value(serde_json::json!(2)).unwrap();
        assert_eq!(back, Tier::Tier2);
    }

    use super::super::schemas::SignalDimension;

    fn dim(score: f64, conf: f64) -> SignalDimension {
        SignalDimension {
            score,
            confidence: conf,
            evidence_ids: vec![],
            hallucination_penalty: None,
        }
    }
    fn signals_57() -> ExtractedSignals {
        ExtractedSignals {
            technical_depth: dim(80.0, 0.9),
            domain_relevance: dim(70.0, 0.8),
            trajectory_match: dim(60.0, 0.7),
            culture_fit: dim(50.0, 0.6),
            reachability: dim(90.0, 0.95),
            red_flags: vec![],
            prompt_versions: None,
        }
    }

    #[test]
    fn score_candidate_matches_ts_golden_vector() {
        // 80*0.3*0.9 + 70*0.25*0.8 + 60*0.2*0.7 + 50*0.15*0.6 + 90*0.1*0.95 = 57.05
        let s = score_candidate(&signals_57(), &ScoringConfig::default());
        assert!((s.total - 57.05).abs() < 1e-6, "got {}", s.total);
        assert_eq!(s.breakdown.len(), 5);
        let order: Vec<&str> = s.breakdown.iter().map(|c| c.dimension.as_str()).collect();
        assert_eq!(
            order,
            vec![
                "technicalDepth",
                "domainRelevance",
                "trajectoryMatch",
                "cultureFit",
                "reachability"
            ]
        );
    }

    #[test]
    fn score_candidate_confidence_gates_contribution() {
        let mut sig = signals_57();
        sig.technical_depth.confidence = 0.0;
        let s = score_candidate(&sig, &ScoringConfig::default());
        assert_eq!(s.breakdown[0].weighted, 0.0);
    }

    #[test]
    fn score_candidate_subtracts_red_flag_penalties_and_clamps_low() {
        let mut sig = signals_57();
        sig.red_flags = vec![
            RedFlag {
                signal: "a".into(),
                evidence_id: "ev-1".into(),
                severity: Severity::High,
            },
            RedFlag {
                signal: "b".into(),
                evidence_id: "ev-1".into(),
                severity: Severity::High,
            },
            RedFlag {
                signal: "c".into(),
                evidence_id: "ev-1".into(),
                severity: Severity::High,
            },
            RedFlag {
                signal: "d".into(),
                evidence_id: "ev-1".into(),
                severity: Severity::High,
            },
            RedFlag {
                signal: "e".into(),
                evidence_id: "ev-1".into(),
                severity: Severity::High,
            },
            RedFlag {
                signal: "f".into(),
                evidence_id: "ev-1".into(),
                severity: Severity::High,
            },
        ]; // 6 × 10 = 60 penalty; 57.05 - 60 < 0
        let s = score_candidate(&sig, &ScoringConfig::default());
        assert_eq!(s.total, 0.0);
        assert_eq!(s.red_flags.len(), 6);
    }

    #[test]
    fn score_candidate_clamps_high() {
        let maxed = ExtractedSignals {
            technical_depth: dim(100.0, 1.0),
            domain_relevance: dim(100.0, 1.0),
            trajectory_match: dim(100.0, 1.0),
            culture_fit: dim(100.0, 1.0),
            reachability: dim(100.0, 1.0),
            red_flags: vec![],
            prompt_versions: None,
        };
        let s = score_candidate(&maxed, &ScoringConfig::default());
        assert!(s.total <= 100.0);
        assert!((s.total - 100.0).abs() < 1e-6);
    }

    #[test]
    fn score_candidate_passes_through_hallucination_metadata() {
        let mut sig = signals_57();
        sig.technical_depth.hallucination_penalty =
            Some(super::super::schemas::HallucinationPenalty {
                hallucinated_count: 1,
                total_cited_count: 4,
                penalty_applied: 0.25,
                raw_score_before_penalty: 90.0,
            });
        let s = score_candidate(&sig, &ScoringConfig::default());
        assert!(s.breakdown[0].hallucination_penalty.is_some());
    }

    #[test]
    fn score_candidate_populates_per_component_fields() {
        // Guards the full ScoreComponent (raw/weight/weighted), not just total —
        // a raw/weight swap would pass every total-only test but fail S2.4's diff.
        let s = score_candidate(&signals_57(), &ScoringConfig::default());
        let td = &s.breakdown[0];
        assert_eq!(td.dimension, "technicalDepth");
        assert_eq!(td.raw, 80.0);
        assert_eq!(td.weight, 0.30);
        assert!((td.weighted - 21.6).abs() < 1e-6, "got {}", td.weighted);
        assert_eq!(td.confidence, 0.9);
        let dr = &s.breakdown[1];
        assert_eq!(dr.raw, 70.0);
        assert_eq!(dr.weight, 0.25);
        assert!((dr.weighted - 14.0).abs() < 1e-6, "got {}", dr.weighted);
    }

    #[test]
    fn score_candidate_sums_mixed_severity_penalties() {
        let mut sig = signals_57();
        sig.red_flags = vec![
            RedFlag {
                signal: "gap".into(),
                evidence_id: "ev-1".into(),
                severity: Severity::Low,
            },
            RedFlag {
                signal: "hop".into(),
                evidence_id: "ev-1".into(),
                severity: Severity::Medium,
            },
        ]; // 2 + 5 = 7 penalty; 57.05 - 7 = 50.05
        let s = score_candidate(&sig, &ScoringConfig::default());
        assert!((s.total - 50.05).abs() < 1e-6, "got {}", s.total);
    }
}
