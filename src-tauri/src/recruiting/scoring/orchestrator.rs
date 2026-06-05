//! Per-candidate + batch scoring orchestration (TS `ScoredCandidate` assembly):
//! build_evidence → extract_signals (grounded) → score → tier → narrative.

use std::sync::Arc;

use serde::Serialize;

use super::evidence::build_evidence;
use super::narrative::{generate_narrative, NarrativeOptions};
use super::schemas::ExtractedSignals;
use super::score::{assign_tier, score_candidate, Score, ScoringConfig, Tier};
use super::signal_extract::{extract_signals, ExtractSignalsOptions, ScoringError};
use crate::recruiting::identity::types::ResolvedCandidate;
use crate::recruiting::intake::deps::IntakeProvider;
use crate::recruiting::intake::schemas::TalentProfile;

/// A candidate with its grounded signals, score, tier, and narrative — the
/// shortlist row (TS `ScoredCandidate`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoredCandidate {
    pub candidate: ResolvedCandidate,
    pub signals: ExtractedSignals,
    pub score: Score,
    pub tier: Tier,
    pub narrative: String,
}

/// Score one resolved candidate end-to-end. `retrieved_at` is the injectable
/// clock value (build_evidence derives evidence IDs from it).
pub async fn score_resolved_candidate(
    candidate: ResolvedCandidate,
    talent_profile: &TalentProfile,
    provider: Arc<dyn IntakeProvider>,
    config: &ScoringConfig,
    opts: &NarrativeOptions,
    retrieved_at: &str,
) -> Result<ScoredCandidate, ScoringError> {
    let evidence = build_evidence(&candidate, retrieved_at);
    // NOTE: `evidence` feeds extraction + narrative below but is NOT retained on
    // ScoredCandidate. The later output-renderer port (markdown/CSV) will need
    // per-candidate evidence — add `evidence: Vec<EvidenceItem>` to ScoredCandidate
    // then. The omission is a scope boundary, not TS parity.
    let extraction = extract_signals(
        &evidence,
        talent_profile,
        provider.clone(),
        &ExtractSignalsOptions::default(),
    )
    .await?;
    let signals = extraction.signals;
    let score = score_candidate(&signals, config);
    let tier = assign_tier(score.total, &config.tier_thresholds);
    let narrative = generate_narrative(
        &candidate,
        talent_profile,
        &signals,
        &score,
        &evidence,
        provider,
        opts,
    )
    .await?
    .narrative;
    Ok(ScoredCandidate {
        candidate,
        signals,
        score,
        tier,
        narrative,
    })
}

/// Score a batch and return it ranked by `score.total` descending (stable on
/// ties → preserves input order for equal scores).
pub async fn score_candidates(
    candidates: Vec<ResolvedCandidate>,
    talent_profile: &TalentProfile,
    provider: Arc<dyn IntakeProvider>,
    config: &ScoringConfig,
    opts: &NarrativeOptions,
    retrieved_at: &str,
) -> Result<Vec<ScoredCandidate>, ScoringError> {
    let mut scored = Vec::with_capacity(candidates.len());
    for c in candidates {
        scored.push(
            score_resolved_candidate(
                c,
                talent_profile,
                provider.clone(),
                config,
                opts,
                retrieved_at,
            )
            .await?,
        );
    }
    scored.sort_by(|a, b| {
        b.score
            .total
            .partial_cmp(&a.score.total)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(scored)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recruiting::identity::types::{PersonIdentity, SourceData};
    use crate::recruiting::intake::deps::testing::FakeProvider;
    use crate::recruiting::intake::schemas::{
        CompanyIntel, CompetitorMap, CompositeProfile, RoleParameters,
    };
    use std::collections::BTreeMap;

    const CLOCK: &str = "2026-06-01T00:00:00Z";

    fn candidate(id: &str, name: &str) -> ResolvedCandidate {
        let mut sources = BTreeMap::new();
        sources.insert(
            "exa".to_string(),
            SourceData {
                adapter: "exa".into(),
                urls: vec![format!("https://github.com/{id}")],
            },
        );
        ResolvedCandidate {
            id: id.into(),
            identity: PersonIdentity {
                canonical_id: id.into(),
                observed_identifiers: vec![],
                merged_from: None,
                merge_confidence: 1.0,
                low_confidence_merge: false,
            },
            name: name.into(),
            sources,
            page_text: None,
        }
    }
    fn profile() -> TalentProfile {
        TalentProfile {
            role: RoleParameters::default(),
            company: CompanyIntel::default(),
            success_patterns: CompositeProfile::default(),
            anti_patterns: vec![],
            competitor_map: CompetitorMap::default(),
            created_at: "t".into(),
        }
    }
    // Signals with EMPTY evidence_ids so grounding strips nothing → scores stand.
    fn signals_json(tech: u32) -> serde_json::Value {
        serde_json::json!({
            "technicalDepth": {"score": tech, "evidenceIds": [], "confidence": 1.0},
            "domainRelevance": {"score": 0, "evidenceIds": [], "confidence": 1.0},
            "trajectoryMatch": {"score": 0, "evidenceIds": [], "confidence": 1.0},
            "cultureFit": {"score": 0, "evidenceIds": [], "confidence": 1.0},
            "reachability": {"score": 0, "evidenceIds": [], "confidence": 1.0},
            "redFlags": []
        })
    }

    #[tokio::test]
    async fn score_resolved_candidate_end_to_end() {
        // techDepth 100 * weight 0.30 * conf 1.0 = 30.0 total → Tier3 (< 40).
        let provider: Arc<dyn IntakeProvider> = Arc::new(
            FakeProvider::new(vec![signals_json(100)]).with_texts(vec!["A narrative.".into()]),
        );
        let out = score_resolved_candidate(
            candidate("jane", "Jane"),
            &profile(),
            provider,
            &ScoringConfig::default(),
            &NarrativeOptions::default(),
            CLOCK,
        )
        .await
        .expect("scores");
        assert!(
            (out.score.total - 30.0).abs() < 1e-6,
            "got {}",
            out.score.total
        );
        assert_eq!(out.tier, Tier::Tier3);
        assert_eq!(out.narrative, "A narrative.");
    }

    #[tokio::test]
    async fn score_candidates_ranks_descending() {
        // Candidate A scores 30 (techDepth 100), B scores 9 (techDepth 30). B is queued first.
        let provider: Arc<dyn IntakeProvider> = Arc::new(
            FakeProvider::new(vec![signals_json(30), signals_json(100)])
                .with_texts(vec!["B narr".into(), "A narr".into()]),
        );
        let out = score_candidates(
            vec![candidate("b", "Bob"), candidate("a", "Alice")],
            &profile(),
            provider,
            &ScoringConfig::default(),
            &NarrativeOptions::default(),
            CLOCK,
        )
        .await
        .expect("scores");
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].candidate.name, "Alice", "higher score ranks first");
        assert_eq!(out[1].candidate.name, "Bob");
    }

    #[tokio::test]
    async fn score_candidates_is_stable_on_ties() {
        // Equal scores (both techDepth 50 → 15.0). Input order [first, second] preserved.
        let provider: Arc<dyn IntakeProvider> = Arc::new(
            FakeProvider::new(vec![signals_json(50), signals_json(50)])
                .with_texts(vec!["n1".into(), "n2".into()]),
        );
        let out = score_candidates(
            vec![candidate("first", "First"), candidate("second", "Second")],
            &profile(),
            provider,
            &ScoringConfig::default(),
            &NarrativeOptions::default(),
            CLOCK,
        )
        .await
        .expect("scores");
        assert_eq!(out[0].candidate.name, "First");
        assert_eq!(out[1].candidate.name, "Second");
    }

    #[test]
    fn scored_candidate_serializes_camel_case_with_integer_tier() {
        let signals: ExtractedSignals = serde_json::from_value(signals_json(80)).unwrap();
        let score = score_candidate(&signals, &ScoringConfig::default());
        let sc = ScoredCandidate {
            candidate: candidate("jane", "Jane"),
            signals,
            score,
            tier: Tier::Tier2,
            narrative: "n".into(),
        };
        let v = serde_json::to_value(&sc).unwrap();
        assert!(v.get("candidate").is_some());
        assert!(v.get("signals").is_some());
        assert!(v.get("score").is_some());
        assert!(v.get("narrative").is_some());
        assert_eq!(
            v.get("tier").unwrap(),
            &serde_json::json!(2),
            "tier serializes as integer"
        );
    }

    #[tokio::test]
    async fn score_candidates_aborts_batch_on_candidate_error() {
        // Provider scripted with responses for only ONE candidate. The 2nd
        // candidate's extract finds the queue exhausted → error → `?` aborts the
        // whole batch (deliberate hard-abort; no partial success).
        let provider: Arc<dyn IntakeProvider> =
            Arc::new(FakeProvider::new(vec![signals_json(50)]).with_texts(vec!["only one".into()]));
        let result = score_candidates(
            vec![candidate("a", "Alice"), candidate("b", "Bob")],
            &profile(),
            provider,
            &ScoringConfig::default(),
            &NarrativeOptions::default(),
            CLOCK,
        )
        .await;
        assert!(
            result.is_err(),
            "a single candidate failure must abort the batch"
        );
    }
}
