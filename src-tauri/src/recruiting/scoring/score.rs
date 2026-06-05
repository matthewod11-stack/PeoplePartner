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
}
