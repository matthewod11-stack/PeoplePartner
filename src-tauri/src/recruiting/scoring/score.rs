//! Deterministic score calculator + tier assignment (TS `score-calculator.ts`).

use serde::{Deserialize, Serialize};

use super::schemas::Severity;

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
}
