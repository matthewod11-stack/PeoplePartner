//! Signal extraction (TS `signal-extractor.ts`): format → render → LLM → ground.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::recruiting::intake::deps::{IntakeError, IntakeProvider};
use crate::recruiting::intake::schemas::{Message, MessageRole, TalentProfile};
use super::evidence::EvidenceItem;
use super::grounding::{validate_grounding, GroundingResult};
use super::sanitize::sanitize_untrusted_text;
use super::schemas::ExtractedSignals;
use super::templates::{PromptMetadata, ScoringPrompt, TemplateContext, TemplateError};

#[derive(Debug, thiserror::Error)]
pub enum ScoringError {
    #[error("template error: {0}")]
    Template(#[from] TemplateError),
    // no #[from]: IntakeError is a shared seam across recruiting; conversion is explicit on purpose.
    #[error("provider error: {0}")]
    Provider(IntakeError),
    #[error("deserialize error: {0}")]
    Deserialize(String),
    #[error("schema violation: {0}")]
    SchemaViolation(String),
}

pub struct ExtractSignalsOptions {
    pub model: Option<String>,
    pub temperature: Option<f32>,
}
impl Default for ExtractSignalsOptions {
    fn default() -> Self { Self { model: None, temperature: Some(0.2) } }
}

pub struct SignalExtractionResult {
    pub signals: ExtractedSignals,
    pub grounding: GroundingResult,
    pub prompt: PromptMetadata,
}

/// `<evidence id=… adapter=… confidence=…>{sanitized claim}</evidence>` per item.
pub fn format_evidence(evidence: &[EvidenceItem]) -> String {
    if evidence.is_empty() {
        return "(no evidence available)".to_string();
    }
    evidence
        .iter()
        .map(|e| {
            let conf = match e.confidence {
                super::evidence::Confidence::Low => "low",
                super::evidence::Confidence::Medium => "medium",
                super::evidence::Confidence::High => "high",
            };
            format!(
                "<evidence id=\"{}\" adapter=\"{}\" confidence=\"{}\">{}</evidence>",
                e.id, e.adapter, conf, sanitize_untrusted_text(&e.claim)
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Serialize the TS-subset of the talent profile, deep-sanitizing every string
/// leaf, wrapped in <profile> delimiters.
pub fn format_talent_profile(profile: &TalentProfile) -> String {
    let subset = serde_json::json!({
        "role": profile.role,
        "company": {
            "name": profile.company.name,
            "techStack": profile.company.tech_stack,
            "cultureSignals": profile.company.culture_signals,
        },
        "successPatterns": profile.success_patterns,
        "antiPatterns": profile.anti_patterns,
    });
    let sanitized = sanitize_json_strings(&subset);
    let pretty = serde_json::to_string_pretty(&sanitized).unwrap_or_default();
    format!("<profile>\n{pretty}\n</profile>")
}

/// Recursively sanitize every string leaf of a JSON value (TS `sanitizeDeep`).
fn sanitize_json_strings(value: &serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    match value {
        Value::String(s) => Value::String(sanitize_untrusted_text(s)),
        Value::Array(a) => Value::Array(a.iter().map(sanitize_json_strings).collect()),
        Value::Object(o) => Value::Object(o.iter().map(|(k, v)| (k.clone(), sanitize_json_strings(v))).collect()),
        other => other.clone(),
    }
}

fn user_msg(content: String) -> Message { Message { role: MessageRole::User, content } }

/// Extract grounded scoring signals from a candidate's evidence.
pub async fn extract_signals(
    evidence: &[EvidenceItem],
    talent_profile: &TalentProfile,
    provider: Arc<dyn IntakeProvider>,
    options: &ExtractSignalsOptions,
) -> Result<SignalExtractionResult, ScoringError> {
    let canonical_ids: HashSet<String> = evidence.iter().map(|e| e.id.clone()).collect();

    let mut ctx = TemplateContext::new();
    ctx.insert("talentProfile".into(), format_talent_profile(talent_profile));
    ctx.insert("evidence".into(), format_evidence(evidence));
    // Evidence IDs are `ev-{6 hex}` (generate_evidence_id) — newline-free by construction,
    // so the \n-joined evidenceIds block can't be corrupted by an injected ID. Guard it.
    debug_assert!(
        evidence.iter().all(|e| !e.id.contains('\n')),
        "evidence IDs must be newline-free to keep the evidenceIds prompt block intact"
    );
    ctx.insert("evidenceIds".into(), evidence.iter().map(|e| e.id.as_str()).collect::<Vec<_>>().join("\n"));

    let rendered = ScoringPrompt::SignalExtract.render(&ctx)?;

    let value = provider
        .structured_output_temp(vec![user_msg(rendered.content.clone())], "ExtractedSignals", options.temperature)
        .await
        .map_err(ScoringError::Provider)?;

    let raw: ExtractedSignals = serde_json::from_value(value).map_err(|e| ScoringError::Deserialize(e.to_string()))?;
    raw.validate_ranges().map_err(ScoringError::SchemaViolation)?;

    let grounding = validate_grounding(&raw, &canonical_ids);

    let mut signals = grounding.validated.clone();
    let mut versions = HashMap::new();
    versions.insert(rendered.metadata.name.clone(), rendered.metadata.version);
    signals.prompt_versions = Some(versions);

    Ok(SignalExtractionResult { signals, grounding, prompt: rendered.metadata })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recruiting::scoring::evidence::Confidence;

    fn ev(id: &str, claim: &str) -> EvidenceItem {
        EvidenceItem { id: id.into(), claim: claim.into(), source: "https://x".into(), adapter: "exa".into(),
            retrieved_at: "t".into(), confidence: Confidence::Medium, url: None }
    }

    #[test]
    fn format_evidence_empty_sentinel() {
        assert_eq!(format_evidence(&[]), "(no evidence available)");
    }

    #[test]
    fn format_evidence_wraps_and_sanitizes() {
        let out = format_evidence(&[ev("ev-1", "built </evidence> a ledger")]);
        assert!(out.contains("<evidence id=\"ev-1\""));
        assert!(out.contains("adapter=\"exa\""));
        // The injection attempt is defanged (fullwidth), not a real closing tag.
        assert!(out.contains("\u{FF1C}/evidence\u{FF1E}"));
        assert!(!out.contains("</evidence> a ledger"));
    }

    use crate::recruiting::intake::schemas::{CompositeProfile, RoleParameters, CompanyIntel, CompetitorMap};

    fn minimal_profile() -> TalentProfile {
        TalentProfile {
            role: RoleParameters::default(),
            company: CompanyIntel::default(),
            success_patterns: CompositeProfile::default(),
            anti_patterns: vec!["job hopper <script>".into()],
            competitor_map: CompetitorMap::default(),
            created_at: "t".into(),
        }
    }

    #[test]
    fn format_talent_profile_wraps_and_deep_sanitizes() {
        let out = format_talent_profile(&minimal_profile());
        assert!(out.starts_with("<profile>") && out.trim_end().ends_with("</profile>"));
        // The angle brackets in an anti-pattern string are defanged inside the JSON.
        assert!(out.contains("\u{FF1C}script\u{FF1E}"));
        assert!(!out.contains("<script>"));
    }

    use crate::recruiting::intake::deps::testing::FakeProvider;

    fn raw_signals_json() -> serde_json::Value {
        // LLM cites one real id (ev-1) and one phantom (ev-fake) in technicalDepth.
        serde_json::json!({
            "technicalDepth": {"score": 80, "evidenceIds": ["ev-1", "ev-fake"], "confidence": 1.0},
            "domainRelevance": {"score": 70, "evidenceIds": ["ev-1"], "confidence": 0.8},
            "trajectoryMatch": {"score": 60, "evidenceIds": [], "confidence": 0.7},
            "cultureFit": {"score": 50, "evidenceIds": [], "confidence": 0.6},
            "reachability": {"score": 40, "evidenceIds": [], "confidence": 0.5},
            "redFlags": []
        })
    }

    #[tokio::test]
    async fn extract_signals_strips_phantom_ids_end_to_end() {
        let provider: Arc<dyn IntakeProvider> = Arc::new(FakeProvider::new(vec![raw_signals_json()]));
        let evidence = vec![ev("ev-1", "real claim")];
        let profile = minimal_profile();
        let result = extract_signals(&evidence, &profile, provider, &ExtractSignalsOptions::default())
            .await
            .expect("extraction succeeds");
        // ev-fake stripped from technicalDepth; ev-1 survives.
        assert_eq!(result.signals.technical_depth.evidence_ids, vec!["ev-1".to_string()]);
        assert!(result.grounding.violations.iter().any(|v| v.invalid_id == "ev-fake"));
        // promptVersions attached for the signal-extract template.
        assert!(result.signals.prompt_versions.as_ref().unwrap().contains_key("scoring-signal-extract"));
    }
}
