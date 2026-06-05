//! Narrative generator (TS `narrative-generator.ts`): score breakdown text +
//! LLM prose with evidence citations, over the shared `chat_temp` seam @ 0.3.

use super::schemas::Severity;
use super::score::Score;

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

#[cfg(test)]
mod tests {
    use super::super::schemas::{HallucinationPenalty, RedFlag, Severity};
    use super::super::score::{ScoreComponent, ScoringWeights};
    use super::*;

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
}
