//! Narrative generator (TS `narrative-generator.ts`): score breakdown text +
//! LLM prose with evidence citations, over the shared `chat_temp` seam @ 0.3.

use std::sync::Arc;

use super::evidence::EvidenceItem;
use super::schemas::{ExtractedSignals, Severity};
use super::score::Score;
use super::signal_extract::{format_evidence, format_talent_profile, ScoringError};
use super::templates::{PromptMetadata, ScoringPrompt, TemplateContext};
use crate::recruiting::identity::types::ResolvedCandidate;
use crate::recruiting::intake::deps::IntakeProvider;
use crate::recruiting::intake::schemas::{Message, MessageRole, TalentProfile};

/// Render a `Score` into the breakdown text the narrative prompt consumes.
/// Faithful port of TS `formatScoreBreakdown` (weights `{:.2}`, weighted `{:.1}`,
/// optional H-9 line, red-flag summary or "none").
pub fn format_score_breakdown(score: &Score) -> String {
    let mut lines = vec![format!("Total: {}/100", score.total)];
    for comp in &score.breakdown {
        let mut line = format!(
            "- {}: {} × {:.2} = {:.1} (confidence: {})",
            comp.dimension, comp.raw, comp.weight, comp.weighted, comp.confidence
        );
        if let Some(h) = &comp.hallucination_penalty {
            let pct = (h.penalty_applied * 100.0).round() as i64;
            line += &format!(
                " [{}/{} citations hallucinated → -{}% from {}]",
                h.hallucinated_count, h.total_cited_count, pct, h.raw_score_before_penalty
            );
        }
        lines.push(line);
    }
    if score.red_flags.is_empty() {
        lines.push("Red flags: none".to_string());
    } else {
        let summary = score
            .red_flags
            .iter()
            .map(|f| format!("{}: \"{}\"", severity_label(f.severity), f.signal))
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(format!(
            "Red flags: {} ({})",
            score.red_flags.len(),
            summary
        ));
    }
    lines.join("\n")
}

fn severity_label(s: Severity) -> &'static str {
    match s {
        Severity::Low => "low",
        Severity::Medium => "medium",
        Severity::High => "high",
    }
}

#[derive(Debug, Clone, Default)]
pub struct NarrativeOptions {
    pub model: Option<String>,
    pub temperature: Option<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NarrativeResult {
    pub narrative: String,
    pub prompt: PromptMetadata,
    // NOTE: TS exposes `usage: TokenUsage`; omitted — the Rust chat_temp seam returns only String.
}

/// Generate a candidate narrative via the LLM. Grounding is prompt-only
/// (faithful to TS): the vendored `scoring-narrative.md` template carries the
/// cite-only-canonical-IDs + injection-defense constraints. Temperature defaults
/// to 0.3 (TS `?? 0.3`).
pub async fn generate_narrative(
    candidate: &ResolvedCandidate,
    talent_profile: &TalentProfile,
    signals: &ExtractedSignals,
    score: &Score,
    evidence: &[EvidenceItem],
    provider: Arc<dyn IntakeProvider>,
    options: &NarrativeOptions,
) -> Result<NarrativeResult, ScoringError> {
    let mut ctx = TemplateContext::new();
    ctx.insert(
        "talentProfile".into(),
        format_talent_profile(talent_profile),
    );
    ctx.insert("candidateName".into(), candidate.name.clone());
    ctx.insert("evidence".into(), format_evidence(evidence));
    ctx.insert(
        "signals".into(),
        serde_json::to_string_pretty(signals).unwrap_or_default(),
    );
    ctx.insert("scoreBreakdown".into(), format_score_breakdown(score));
    ctx.insert(
        "evidenceIds".into(),
        evidence
            .iter()
            .map(|e| e.id.as_str())
            .collect::<Vec<_>>()
            .join("\n"),
    );

    let rendered = ScoringPrompt::Narrative.render(&ctx)?;
    let messages = vec![Message {
        role: MessageRole::User,
        content: rendered.content,
    }];
    let temperature = options.temperature.unwrap_or(0.3);
    let narrative = provider
        .chat_temp(messages, options.model.as_deref(), Some(temperature))
        .await
        .map_err(ScoringError::Provider)?;

    Ok(NarrativeResult {
        narrative,
        prompt: rendered.metadata,
    })
}

#[cfg(test)]
mod tests {
    use super::super::schemas::{HallucinationPenalty, RedFlag, Severity};
    use super::super::score::{ScoreComponent, ScoringWeights};
    use super::*;

    use super::super::evidence::{Confidence, EvidenceItem};
    use super::super::score::ScoringConfig;
    use crate::recruiting::identity::types::{PersonIdentity, ResolvedCandidate, SourceData};
    use crate::recruiting::intake::deps::testing::FakeProvider;
    use crate::recruiting::intake::deps::IntakeProvider;
    use crate::recruiting::intake::schemas::{
        CompanyIntel, CompetitorMap, CompositeProfile, RoleParameters, TalentProfile,
    };
    use std::collections::BTreeMap;
    use std::sync::Arc;

    fn comp(dim: &str, raw: f64, weight: f64, weighted: f64, conf: f64) -> ScoreComponent {
        ScoreComponent {
            dimension: dim.into(),
            raw,
            weight,
            weighted,
            evidence_ids: vec![],
            confidence: conf,
            hallucination_penalty: None,
        }
    }

    #[test]
    fn breakdown_renders_total_dimensions_and_no_flags() {
        let score = Score {
            total: 57.05,
            breakdown: vec![
                comp("technicalDepth", 80.0, 0.30, 21.6, 0.9),
                comp("reachability", 90.0, 0.10, 8.55, 0.95),
            ],
            weights: ScoringWeights::default(),
            red_flags: vec![],
            prompt_versions: None,
        };
        let out = format_score_breakdown(&score);
        assert_eq!(
            out,
            "Total: 57.05/100\n- technicalDepth: 80 × 0.30 = 21.6 (confidence: 0.9)\n- reachability: 90 × 0.10 = 8.6 (confidence: 0.95)\nRed flags: none"
        );
    }

    #[test]
    fn breakdown_surfaces_hallucination_penalty_line() {
        let mut c = comp("technicalDepth", 90.0, 0.30, 20.25, 1.0);
        c.hallucination_penalty = Some(HallucinationPenalty {
            hallucinated_count: 1,
            total_cited_count: 4,
            penalty_applied: 0.25,
            raw_score_before_penalty: 90.0,
        });
        let score = Score {
            total: 20.25,
            breakdown: vec![c],
            weights: ScoringWeights::default(),
            red_flags: vec![],
            prompt_versions: None,
        };
        let out = format_score_breakdown(&score);
        assert!(
            out.contains("[1/4 citations hallucinated → -25% from 90]"),
            "got:\n{out}"
        );
    }

    #[test]
    fn breakdown_summarizes_red_flags() {
        let score = Score {
            total: 50.0,
            breakdown: vec![],
            weights: ScoringWeights::default(),
            red_flags: vec![RedFlag {
                signal: "job hopper".into(),
                evidence_id: "ev-1".into(),
                severity: Severity::Medium,
            }],
            prompt_versions: None,
        };
        let out = format_score_breakdown(&score);
        assert!(
            out.ends_with("Red flags: 1 (medium: \"job hopper\")"),
            "got:\n{out}"
        );
    }

    fn fixture_candidate() -> ResolvedCandidate {
        let mut sources = BTreeMap::new();
        sources.insert(
            "exa".to_string(),
            SourceData {
                adapter: "exa".into(),
                urls: vec!["https://github.com/jane".into()],
            },
        );
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
            page_text: None,
        }
    }

    fn fixture_profile() -> TalentProfile {
        TalentProfile {
            role: RoleParameters::default(),
            company: CompanyIntel::default(),
            success_patterns: CompositeProfile::default(),
            anti_patterns: vec![],
            competitor_map: CompetitorMap::default(),
            created_at: "t".into(),
        }
    }

    fn fixture_evidence() -> Vec<EvidenceItem> {
        vec![EvidenceItem {
            id: "ev-1".into(),
            claim: "built a ledger".into(),
            source: "https://x".into(),
            adapter: "exa".into(),
            retrieved_at: "t".into(),
            confidence: Confidence::Medium,
            url: None,
        }]
    }

    #[tokio::test]
    async fn generate_narrative_returns_scripted_prose_and_metadata() {
        let provider: Arc<dyn IntakeProvider> = Arc::new(
            FakeProvider::new(vec![]).with_texts(vec!["Strong fit. Built a ledger (ev-1).".into()]),
        );
        // Build fixtures locally (don't reach into another module's #[cfg(test)] mod).
        let signals: ExtractedSignals = serde_json::from_value(serde_json::json!({
            "technicalDepth": {"score": 80, "evidenceIds": [], "confidence": 0.9},
            "domainRelevance": {"score": 0, "evidenceIds": [], "confidence": 1.0},
            "trajectoryMatch": {"score": 0, "evidenceIds": [], "confidence": 1.0},
            "cultureFit": {"score": 0, "evidenceIds": [], "confidence": 1.0},
            "reachability": {"score": 0, "evidenceIds": [], "confidence": 1.0},
            "redFlags": []
        }))
        .unwrap();
        let score =
            crate::recruiting::scoring::score::score_candidate(&signals, &ScoringConfig::default());
        let result = generate_narrative(
            &fixture_candidate(),
            &fixture_profile(),
            &signals,
            &score,
            &fixture_evidence(),
            provider,
            &NarrativeOptions::default(),
        )
        .await
        .expect("narrative succeeds");
        assert_eq!(result.narrative, "Strong fit. Built a ledger (ev-1).");
        assert_eq!(result.prompt.name, "scoring-narrative");
        assert_eq!(result.prompt.version, 2);
    }
}
