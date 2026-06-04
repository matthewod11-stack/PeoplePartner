//! LLM-extracted scoring signals (TS `core/scoring.ts` + `scoring/schemas.ts`).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity { Low, Medium, High }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedFlag {
    pub signal: String,
    pub evidence_id: String,
    pub severity: Severity,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HallucinationPenalty {
    pub hallucinated_count: u32,
    pub total_cited_count: u32,
    pub penalty_applied: f64,
    pub raw_score_before_penalty: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignalDimension {
    pub score: f64,
    pub evidence_ids: Vec<String>,
    pub confidence: f64,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub hallucination_penalty: Option<HallucinationPenalty>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractedSignals {
    pub technical_depth: SignalDimension,
    pub domain_relevance: SignalDimension,
    pub trajectory_match: SignalDimension,
    pub culture_fit: SignalDimension,
    pub reachability: SignalDimension,
    pub red_flags: Vec<RedFlag>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub prompt_versions: Option<std::collections::HashMap<String, u32>>,
}

impl ExtractedSignals {
    /// Iterate the 5 scoring dimensions by `(name, &dimension)`.
    pub fn dimensions(&self) -> [(&'static str, &SignalDimension); 5] {
        [
            ("technicalDepth", &self.technical_depth),
            ("domainRelevance", &self.domain_relevance),
            ("trajectoryMatch", &self.trajectory_match),
            ("cultureFit", &self.culture_fit),
            ("reachability", &self.reachability),
        ]
    }

    /// Post-deserialize bounds check (the Zod `min/max` equivalent).
    pub fn validate_ranges(&self) -> Result<(), String> {
        for (name, d) in self.dimensions() {
            if !(0.0..=100.0).contains(&d.score) {
                return Err(format!("{name}.score out of range: {}", d.score));
            }
            if !(0.0..=1.0).contains(&d.confidence) {
                return Err(format!("{name}.confidence out of range: {}", d.confidence));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dim(score: f64, conf: f64, ids: &[&str]) -> SignalDimension {
        SignalDimension { score, confidence: conf, evidence_ids: ids.iter().map(|s| s.to_string()).collect(), hallucination_penalty: None }
    }
    pub(crate) fn clean_signals() -> ExtractedSignals {
        ExtractedSignals {
            technical_depth: dim(80.0, 0.9, &["ev-1"]),
            domain_relevance: dim(70.0, 0.8, &["ev-1"]),
            trajectory_match: dim(60.0, 0.7, &[]),
            culture_fit: dim(50.0, 0.6, &[]),
            reachability: dim(40.0, 0.5, &[]),
            red_flags: vec![],
            prompt_versions: None,
        }
    }

    #[test]
    fn deserializes_camelcase_llm_json() {
        let json = r#"{
          "technicalDepth": {"score": 80, "evidenceIds": ["ev-1"], "confidence": 0.9},
          "domainRelevance": {"score": 70, "evidenceIds": [], "confidence": 0.8},
          "trajectoryMatch": {"score": 60, "evidenceIds": [], "confidence": 0.7},
          "cultureFit": {"score": 50, "evidenceIds": [], "confidence": 0.6},
          "reachability": {"score": 40, "evidenceIds": [], "confidence": 0.5},
          "redFlags": [{"signal": "gap", "evidenceId": "ev-1", "severity": "low"}]
        }"#;
        let s: ExtractedSignals = serde_json::from_str(json).unwrap();
        assert_eq!(s.technical_depth.score, 80.0);
        assert_eq!(s.red_flags[0].severity, Severity::Low);
        assert!(s.validate_ranges().is_ok());
    }

    #[test]
    fn validate_ranges_rejects_out_of_bounds_score() {
        let mut s = clean_signals();
        s.technical_depth.score = 150.0;
        assert!(s.validate_ranges().is_err());
    }

    #[test]
    fn validate_ranges_rejects_out_of_bounds_confidence() {
        let mut s = clean_signals();
        s.reachability.confidence = 1.5;
        assert!(s.validate_ranges().is_err());
    }
}
