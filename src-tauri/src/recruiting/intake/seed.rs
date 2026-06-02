//! JD/transcript ingestion seed (FHR-93, roadmap S4.0). Turns one piece of
//! redacted text (a JD or a hiring-call transcript) into a partial
//! `IntakeContext` via a single structured LLM extraction, so the FHR-85
//! conversation opens already knowing the role/company/success-profile and
//! just confirms + fills gaps. All extracted fields are optional.

use serde::Deserialize;
use super::context::IntakeContext;
use super::deps::{IntakeDeps, IntakeError};
use super::schemas::{CompanyIntel, CompositeProfile, Message, MessageRole, RoleParameters};

/// Company fields an LLM can pull from prose. Excludes `url`/`analyzedAt`
/// (no crawl happened) — `build_seeded_context` stamps those.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanySeed {
    #[serde(default)] pub name: String,
    #[serde(default)] pub tech_stack: Vec<String>,
    #[serde(default)] pub team_size: Option<String>,
    #[serde(default)] pub funding_stage: Option<String>,
    #[serde(default)] pub product_category: Option<String>,
    #[serde(default)] pub culture_signals: Vec<String>,
    #[serde(default)] pub pitch: Option<String>,
    #[serde(default)] pub competitors: Option<Vec<String>>,
}

/// One structured extraction over the input text. Every field optional — a bare
/// JD fills `role`; a rich transcript fills more.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeedExtraction {
    #[serde(default)] pub role: Option<RoleParameters>,
    #[serde(default)] pub company: Option<CompanySeed>,
    #[serde(default)] pub success_profile: Option<CompositeProfile>,
    #[serde(default)] pub anti_patterns: Vec<String>,
}

/// Map an extraction onto a fresh `IntakeContext`. `role_description` is always
/// set to the source text so the empty-extraction case still degrades into a
/// normal intake. `now` stamps a seeded company's `analyzed_at`.
pub fn build_seeded_context(seed: SeedExtraction, role_description: String, now: &str) -> IntakeContext {
    let company_intel = seed.company.map(|c| CompanyIntel {
        name: c.name,
        url: String::new(),                 // transcript-sourced, no crawl
        tech_stack: c.tech_stack,
        team_size: c.team_size,
        funding_stage: c.funding_stage,
        product_category: c.product_category,
        culture_signals: c.culture_signals,
        pitch: c.pitch,
        competitors: c.competitors,
        analyzed_at: now.to_string(),
    });
    IntakeContext {
        role_description: Some(role_description),
        role_parameters: seed.role,
        company_intel,
        composite_profile: seed.success_profile,
        anti_patterns: seed.anti_patterns,
        ..Default::default()
    }
}

const SEED_EXTRACTION_SYSTEM: &str = r#"You are a recruiter's assistant. The user provides a job description (JD) OR a transcript of a hiring conversation. Extract a structured intake seed as a JSON object. Include ONLY the keys the text actually supports — omit any you cannot ground in the text. Schema:
- role: { title, level (junior|mid|senior|staff|principal|lead|director), scope (one sentence), location?, remotePolicy? (remote|hybrid|in_person|negotiable), compensationRange? {currency (required), min?, max?}, mustHaveSkills: string[], niceToHaveSkills: string[], teamSize?, reportingTo? }
- company: { name, techStack: string[], teamSize?, fundingStage?, productCategory?, cultureSignals: string[], pitch?, competitors?: string[] }
- successProfile: { skillSignatures: string[], seniorityCalibration (e.g. "6-9 yrs, owns subsystems"), cultureSignals: string[], careerTrajectories: [] }
- antiPatterns: string[]  (red flags / disqualifiers mentioned)

A bare JD will usually yield only `role` (and maybe `antiPatterns`). A hiring-call transcript may yield all four. Never invent a company name, competitors, or anti-patterns that aren't in the text."#;

/// Run one structured extraction over the (already redacted) text.
pub async fn extract_seed(text: &str, deps: &IntakeDeps) -> Result<SeedExtraction, IntakeError> {
    let messages = vec![
        Message { role: MessageRole::System, content: SEED_EXTRACTION_SYSTEM.to_string() },
        Message { role: MessageRole::User, content: text.to_string() },
    ];
    let v = deps.provider.structured_output(messages, "SeedExtraction").await?;
    serde_json::from_value(v).map_err(|e| IntakeError::Deserialize(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::deps::testing::{FakeProvider, FakeResearch, fake_deps};

    fn role() -> RoleParameters {
        serde_json::from_value(serde_json::json!({
            "title": "Staff Engineer", "level": "staff", "scope": "owns infra",
            "mustHaveSkills": ["rust"], "niceToHaveSkills": []
        })).unwrap()
    }

    #[test]
    fn maps_all_present_fields_into_context() {
        let seed = SeedExtraction {
            role: Some(role()),
            company: Some(CompanySeed { name: "Acme".into(), competitors: Some(vec!["Stripe".into()]), ..Default::default() }),
            success_profile: Some(CompositeProfile {
                career_trajectories: vec![], skill_signatures: vec!["rust".into()],
                seniority_calibration: "senior".into(), culture_signals: vec![] }),
            anti_patterns: vec!["job hopper".into()],
        };
        let ctx = build_seeded_context(seed, "the JD text".into(), "2026-06-02T00:00:00Z");

        assert_eq!(ctx.role_description.as_deref(), Some("the JD text"));
        assert_eq!(ctx.role_parameters.as_ref().unwrap().title, "Staff Engineer");
        let intel = ctx.company_intel.as_ref().expect("company seeded");
        assert_eq!(intel.name, "Acme");
        assert_eq!(intel.url, "");                                  // no crawl
        assert_eq!(intel.analyzed_at, "2026-06-02T00:00:00Z");     // stamped
        assert_eq!(intel.competitors.as_deref(), Some(&["Stripe".to_string()][..]));
        assert_eq!(ctx.composite_profile.as_ref().unwrap().skill_signatures, vec!["rust".to_string()]);
        assert_eq!(ctx.anti_patterns, vec!["job hopper".to_string()]);
    }

    #[test]
    fn partial_company_object_without_name_deserializes() {
        // LLM emitted a company block but omitted name → must not error the whole extraction.
        let seed: SeedExtraction = serde_json::from_value(serde_json::json!({
            "company": { "techStack": ["rust"], "competitors": ["Stripe"] }
        })).expect("partial company must deserialize");
        let c = seed.company.expect("company present");
        assert_eq!(c.name, "");                 // defaulted, not an error
        assert_eq!(c.tech_stack, vec!["rust".to_string()]);
    }

    #[test]
    fn empty_extraction_still_sets_role_description() {
        let ctx = build_seeded_context(SeedExtraction::default(), "raw text".into(), "t");
        assert_eq!(ctx.role_description.as_deref(), Some("raw text"));
        assert!(ctx.role_parameters.is_none());
        assert!(ctx.company_intel.is_none());
        assert!(ctx.composite_profile.is_none());
        assert!(ctx.anti_patterns.is_empty());
    }

    #[tokio::test]
    async fn extract_seed_parses_provider_json() {
        let deps = fake_deps(FakeProvider::new(vec![serde_json::json!({
            "role": {"title":"Eng","level":"senior","scope":"x","mustHaveSkills":[],"niceToHaveSkills":[]},
            "antiPatterns": ["no public code"]
        })]), FakeResearch::default());

        let seed = extract_seed("some JD text", &deps).await.unwrap();
        assert_eq!(seed.role.as_ref().unwrap().title, "Eng");
        assert_eq!(seed.anti_patterns, vec!["no public code".to_string()]);
        assert!(seed.company.is_none());          // omitted field → None
    }

    #[tokio::test]
    async fn extract_seed_empty_object_is_all_none() {
        let deps = fake_deps(FakeProvider::new(vec![serde_json::json!({})]), FakeResearch::default());
        let seed = extract_seed("text with nothing useful", &deps).await.unwrap();
        assert!(seed.role.is_none()
            && seed.company.is_none()
            && seed.success_profile.is_none()
            && seed.anti_patterns.is_empty());
    }
}
