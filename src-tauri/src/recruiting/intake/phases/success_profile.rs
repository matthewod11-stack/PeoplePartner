//! Phase 3 — Success profile. Port of success-profile.ts.

use async_trait::async_trait;
use crate::recruiting::intake::context::{IntakeContext, IntakeContextUpdates};
use crate::recruiting::intake::deps::{IntakeDeps, IntakeError};
use crate::recruiting::intake::node::{IntakeNode, NodeId, ParsedResponse, Phase};
use crate::recruiting::intake::schemas::{CompositeProfile, Message, MessageRole, ProfileAnalysis, ProfileInput};

const COMPOSITE_SYSTEM: &str = r#"You are a talent analyst building a success profile from existing team members. Analyze the provided profiles and identify common patterns. Return a JSON object with:
- careerTrajectories: the career trajectories from the profiles (array of arrays of {company, role, duration, signals})
- skillSignatures: array of the most common/important skills across all profiles
- seniorityCalibration: a description of the typical seniority range (e.g., "4-7 years, owns subsystems")
- cultureSignals: array of common culture indicators across profiles"#;

const ANTI_PATTERN_SYSTEM: &str = r#"Parse the user's red flags/anti-patterns into a JSON array of strings. Each string should be a concise anti-pattern description (e.g., "frequent job-hopper", "no public code", "only enterprise experience")."#;

const SKIP_PHRASES: &[&str] = &["skip", "none", "n/a", "no one", "not sure"];

fn is_skip(resp: &str) -> bool {
    let n = resp.trim().to_lowercase();
    SKIP_PHRASES.iter().any(|p| n.contains(p))
}

const CONFIRM_PHRASES: &[&str] = &["yes", "looks good", "correct", "confirmed", "proceed", "lgtm", "looks right"];

// `contains`-based: a reply like "yes but also add X" reads as a pure confirm.
// Accepted trade-off (same as search_config.rs); revisit with a leading-token
// heuristic if user testing shows it matters.
fn is_confirm(resp: &str) -> bool {
    let n = resp.trim().to_lowercase();
    CONFIRM_PHRASES.iter().any(|p| n.contains(p)) || n == "y"
}

fn user_msg(c: &str) -> Message { Message { role: MessageRole::User, content: c.into() } }
fn system_msg(c: &str) -> Message { Message { role: MessageRole::System, content: c.into() } }

/// Parse a multi-line references blob into ProfileInputs. Port of
/// success-profile.ts:21-62 using string methods (no regex dep).
pub fn parse_profile_inputs(text: &str) -> Vec<ProfileInput> {
    let mut out = Vec::new();
    for raw in text.split('\n') {
        let line = raw.trim();
        if line.is_empty() { continue; }
        let lower = line.to_lowercase();
        if lower.contains("github.com/") { out.push(ProfileInput::GithubUrl { url: line.into() }); continue; }
        if lower.contains("linkedin.com/in/") { out.push(ProfileInput::LinkedinUrl { url: line.into() }); continue; }
        if lower.starts_with("http://") || lower.starts_with("https://") {
            out.push(ProfileInput::PersonalUrl { url: line.into() }); continue;
        }
        if let Some((name, company)) = split_name_company(line) {
            out.push(ProfileInput::NameCompany { name, company }); continue;
        }
        if line.chars().count() > 20 { out.push(ProfileInput::PastedText { text: line.into() }); }
    }
    out
}

/// Split "Name @ Company" or "Name at Company" (first separator wins).
fn split_name_company(line: &str) -> Option<(String, String)> {
    for sep in [" @ ", " at ", " At ", " AT "] {
        if let Some(idx) = line.find(sep) {
            let name = line[..idx].trim().to_string();
            let company = line[idx + sep.len()..].trim().to_string();
            if !name.is_empty() && !company.is_empty() { return Some((name, company)); }
        }
    }
    None
}

/// Collect each profile's URLs, dedupe preserving first-seen order.
/// Port of `extractSimilaritySeeds` in content-research.ts.
pub fn extract_similarity_seeds(profiles: &[ProfileAnalysis]) -> Vec<String> {
    let mut seeds: Vec<String> = Vec::new();
    for p in profiles {
        for u in &p.urls {
            if !seeds.contains(u) { seeds.push(u.clone()); }
        }
    }
    seeds
}

/// Build the composite profile via the LLM. Port of success-profile.ts:68-108.
async fn build_composite_profile(profiles: &[ProfileAnalysis], deps: &IntakeDeps)
    -> Result<CompositeProfile, IntakeError> {
    if profiles.is_empty() {
        return Ok(CompositeProfile {
            career_trajectories: vec![], skill_signatures: vec![],
            seniority_calibration: "unknown".into(), culture_signals: vec![],
        });
    }
    let user = format!("Team member profiles:\n{}", serde_json::to_string_pretty(profiles).unwrap_or_default());
    let v = deps.provider.structured_output(vec![system_msg(COMPOSITE_SYSTEM), user_msg(&user)], "CompositeProfile").await?;
    serde_json::from_value(v).map_err(|e| IntakeError::Deserialize(e.to_string()))
}

pub struct TeamInput;
#[async_trait]
impl IntakeNode for TeamInput {
    fn id(&self) -> NodeId { NodeId::TeamInput }
    fn phase(&self) -> Phase { Phase::SuccessProfile }
    /// FHR-93: a transcript seeds `composite_profile` directly (it describes the
    /// ideal candidate in prose, not reference-person URLs), so skip URL collection.
    fn skip_if(&self, ctx: &IntakeContext) -> bool { ctx.composite_profile.is_some() }
    async fn prompt(&self, ctx: &IntakeContext) -> String {
        let role_name = ctx.role_parameters.as_ref().map(|r| r.title.as_str()).unwrap_or("this role");
        format!(
            "Now let's build a success profile for {}.\n\nShare references for 1-3 people who are great at this role (current team members, past hires, or role models). For each person, provide any of:\n- GitHub URL\n- LinkedIn URL\n- Personal website\n- Name @ Company\n- A description of their background\n\n(One per line, or paste a paragraph about them)",
            role_name
        )
    }
    async fn parse(&self, response: &str, _ctx: &IntakeContext, deps: &IntakeDeps)
        -> Result<ParsedResponse, IntakeError> {
        if is_skip(response) {
            return Ok(ParsedResponse { structured: serde_json::json!({ "skipped": true }), ..Default::default() });
        }
        let inputs = parse_profile_inputs(response);
        if inputs.is_empty() {
            return Ok(ParsedResponse {
                structured: serde_json::json!({ "noProfiles": true }),
                follow_up_needed: true,
                follow_up_reason: Some("Could not parse any profile references from input".into()),
                ..Default::default()
            });
        }
        let mut analyses = Vec::new();
        for input in &inputs {
            if let Ok(a) = deps.research.analyze_profile(input).await { analyses.push(a); }
        }
        let seeds = extract_similarity_seeds(&analyses);
        Ok(ParsedResponse {
            structured: serde_json::json!({ "profileCount": analyses.len() }),
            context_updates: IntakeContextUpdates {
                team_profiles: Some(analyses),
                similarity_seeds: Some(seeds),
                ..Default::default()
            },
            ..Default::default()
        })
    }
    fn next(&self, parsed: &ParsedResponse, _c: &IntakeContext) -> NodeId {
        let skipped = parsed.structured.get("skipped").and_then(|v| v.as_bool()).unwrap_or(false);
        let no_profiles = parsed.structured.get("noProfiles").and_then(|v| v.as_bool()).unwrap_or(false);
        if skipped || no_profiles { NodeId::AntiPatterns } else { NodeId::TeamAnalysis }
    }
}

pub struct TeamAnalysis;
#[async_trait]
impl IntakeNode for TeamAnalysis {
    fn id(&self) -> NodeId { NodeId::TeamAnalysis }
    fn phase(&self) -> Phase { Phase::SuccessProfile }
    async fn prompt(&self, ctx: &IntakeContext) -> String {
        let profiles = &ctx.team_profiles;
        if profiles.is_empty() {
            // FHR-93: seeded-from-transcript success profile — show it for confirmation.
            if let Some(cp) = &ctx.composite_profile {
                let mut lines = vec!["Here's the success profile I built from what you shared:\n".to_string()];
                if !cp.skill_signatures.is_empty() { lines.push(format!("**Key skills:** {}", cp.skill_signatures.join(", "))); }
                if !cp.seniority_calibration.is_empty() { lines.push(format!("**Seniority:** {}", cp.seniority_calibration)); }
                if !cp.culture_signals.is_empty() { lines.push(format!("**Culture signals:** {}", cp.culture_signals.join(", "))); }
                lines.push("\nDoes this capture the ideal candidate? Anything to add or correct?".into());
                return lines.join("\n");
            }
            return "No team profiles to analyze. Let's move on to anti-patterns.".into();
        }

        let mut lines = vec![format!("I analyzed {} team member(s):\n", profiles.len())];

        for profile in profiles {
            lines.push(format!("**{}** ({})", profile.name.as_deref().unwrap_or("Unknown"), profile.input_type));
            if !profile.skill_signatures.is_empty() {
                lines.push(format!("  Skills: {}", profile.skill_signatures.join(", ")));
            }
            if let Some(ref seniority) = profile.seniority_level {
                lines.push(format!("  Seniority: {}", seniority));
            }
            if !profile.career_trajectory.is_empty() {
                let trajectory = profile.career_trajectory.iter()
                    .map(|step| {
                        if let Some(ref role) = step.role {
                            format!("{} ({})", step.company, role)
                        } else {
                            step.company.clone()
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" → ");
                lines.push(format!("  Trajectory: {}", trajectory));
            }
            lines.push(String::new());
        }

        lines.push("Does this capture the team accurately? Anything to add or correct?".into());

        lines.join("\n")
    }
    async fn parse(&self, _response: &str, ctx: &IntakeContext, deps: &IntakeDeps)
        -> Result<ParsedResponse, IntakeError> {
        // FHR-93: a seeded composite with no analyzed team members must not be
        // rebuilt into an empty "unknown" composite. Keep it.
        if ctx.team_profiles.is_empty() && ctx.composite_profile.is_some() {
            return Ok(ParsedResponse::default());
        }
        let composite = build_composite_profile(&ctx.team_profiles, deps).await?;
        Ok(ParsedResponse {
            context_updates: IntakeContextUpdates { composite_profile: Some(composite), ..Default::default() },
            ..Default::default()
        })
    }
    fn next(&self, _p: &ParsedResponse, _c: &IntakeContext) -> NodeId { NodeId::AntiPatterns }
}

pub struct AntiPatterns;
#[async_trait]
impl IntakeNode for AntiPatterns {
    fn id(&self) -> NodeId { NodeId::AntiPatterns }
    fn phase(&self) -> Phase { Phase::SuccessProfile }
    async fn prompt(&self, ctx: &IntakeContext) -> String {
        if !ctx.anti_patterns.is_empty() {
            return format!(
                "From what you shared, I noted these red flags: {}.\n\nAnything to add? (Or say \"looks good\" to proceed.)",
                ctx.anti_patterns.join(", ")
            );
        }
        "What are some red flags or anti-patterns for this role? What would make someone a bad fit?\n\n(e.g., \"no public code\", \"job-hopping\", \"only large-company experience\", \"no distributed systems experience\")".into()
    }
    async fn parse(&self, response: &str, ctx: &IntakeContext, deps: &IntakeDeps)
        -> Result<ParsedResponse, IntakeError> {
        // FHR-93: seeded patterns + a confirm/skip reply → keep them (no LLM, no
        // concat-duplication). A substantive reply still extracts + appends.
        if !ctx.anti_patterns.is_empty() && (is_confirm(response) || is_skip(response)) {
            return Ok(ParsedResponse { structured: serde_json::json!({ "confirmed": true }), ..Default::default() });
        }
        if is_skip(response) {
            return Ok(ParsedResponse { structured: serde_json::json!({ "skipped": true }), ..Default::default() });
        }
        let v = deps.provider.structured_output(vec![system_msg(ANTI_PATTERN_SYSTEM), user_msg(response)], "AntiPatterns").await?;
        let patterns: Vec<String> = serde_json::from_value(v).map_err(|e| IntakeError::Deserialize(e.to_string()))?;
        Ok(ParsedResponse {
            context_updates: IntakeContextUpdates { anti_patterns: Some(patterns), ..Default::default() },
            ..Default::default()
        })
    }
    fn next(&self, _p: &ParsedResponse, _c: &IntakeContext) -> NodeId { NodeId::ConfigGenerate }
}

pub fn create_success_profile_nodes() -> Vec<Box<dyn IntakeNode>> {
    vec![Box::new(TeamInput), Box::new(TeamAnalysis), Box::new(AntiPatterns)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recruiting::intake::context::IntakeContext;
    use crate::recruiting::intake::deps::testing::{FakeProvider, FakeResearch, fake_deps};
    use crate::recruiting::intake::node::{IntakeNode, NodeId};
    use crate::recruiting::intake::schemas::{CompositeProfile, ProfileAnalysis, ProfileInput};

    #[test]
    fn parses_github_linkedin_namecompany_and_text() {
        let inputs = parse_profile_inputs(
            "https://github.com/torvalds\nhttps://linkedin.com/in/x\nJane @ Stripe\nA long freeform description over twenty chars");
        assert!(matches!(inputs[0], ProfileInput::GithubUrl { .. }));
        assert!(matches!(inputs[1], ProfileInput::LinkedinUrl { .. }));
        assert!(matches!(inputs[2], ProfileInput::NameCompany { .. }));
        assert!(matches!(inputs[3], ProfileInput::PastedText { .. }));
    }

    #[test]
    fn extract_seeds_dedupes_preserving_order() {
        let p = |u: &str| ProfileAnalysis { input_type:"github_url".into(), name:None,
            career_trajectory:vec![], skill_signatures:vec![], seniority_level:None,
            culture_signals:vec![], urls:vec![u.into()], analyzed_at:"t".into() };
        let seeds = extract_similarity_seeds(&[p("https://a"), p("https://a"), p("https://b")]);
        assert_eq!(seeds, vec!["https://a".to_string(), "https://b".to_string()]);
    }

    #[tokio::test]
    async fn team_input_skip_goes_to_anti_patterns() {
        let node = TeamInput;
        let deps = fake_deps(FakeProvider::new(vec![]), FakeResearch::default());
        let ctx = IntakeContext::default();
        let parsed = node.parse("skip", &ctx, &deps).await.unwrap();
        assert_eq!(node.next(&parsed, &ctx), NodeId::AntiPatterns);
    }

    #[tokio::test]
    async fn team_input_with_profiles_goes_to_analysis_and_seeds() {
        let node = TeamInput;
        let research = FakeResearch { profiles: vec![ProfileAnalysis {
            input_type:"github_url".into(), name:Some("T".into()), career_trajectory:vec![],
            skill_signatures:vec!["rust".into()], seniority_level:None, culture_signals:vec![],
            urls:vec!["https://github.com/t".into()], analyzed_at:"2026-06-01T00:00:00Z".into() }],
            ..Default::default() };
        let deps = fake_deps(FakeProvider::new(vec![]), research);
        let ctx = IntakeContext::default();
        let parsed = node.parse("https://github.com/t", &ctx, &deps).await.unwrap();
        assert_eq!(node.next(&parsed, &ctx), NodeId::TeamAnalysis);
        assert!(!parsed.context_updates.similarity_seeds.clone().unwrap().is_empty());
    }

    #[tokio::test]
    async fn anti_patterns_skip_advances_to_config() {
        let node = AntiPatterns;
        let deps = fake_deps(FakeProvider::new(vec![]), FakeResearch::default());
        let ctx = IntakeContext::default();
        let parsed = node.parse("skip", &ctx, &deps).await.unwrap();
        assert_eq!(node.next(&parsed, &ctx), NodeId::ConfigGenerate);
    }

    fn composite() -> CompositeProfile {
        CompositeProfile { career_trajectories: vec![], skill_signatures: vec!["rust".into()],
            seniority_calibration: "senior".into(), culture_signals: vec![] }
    }

    #[test]
    fn team_input_skips_when_composite_seeded() {
        let node = TeamInput;
        let seeded = IntakeContext { composite_profile: Some(composite()), ..Default::default() };
        assert!(node.skip_if(&seeded));
        assert!(!node.skip_if(&IntakeContext::default()));
    }

    #[tokio::test]
    async fn team_analysis_keeps_seeded_composite_without_rebuilding() {
        let node = TeamAnalysis;
        // No provider values queued: if parse tried to rebuild via the LLM it would error.
        let deps = fake_deps(FakeProvider::new(vec![]), FakeResearch::default());
        let ctx = IntakeContext { composite_profile: Some(composite()), ..Default::default() }; // team_profiles empty
        let parsed = node.parse("looks accurate", &ctx, &deps).await.unwrap();
        assert!(parsed.context_updates.composite_profile.is_none(), "must NOT overwrite the seeded composite");
        assert_eq!(node.next(&parsed, &ctx), NodeId::AntiPatterns);
    }

    #[tokio::test]
    async fn team_analysis_prompt_shows_seeded_composite() {
        let node = TeamAnalysis;
        let ctx = IntakeContext { composite_profile: Some(composite()), ..Default::default() };
        let prompt = node.prompt(&ctx).await;
        assert!(prompt.contains("rust"), "seeded skills surfaced for confirmation");
    }

    #[tokio::test]
    async fn team_analysis_rebuilds_when_real_profiles_present_even_if_composite_seeded() {
        // Real team_profiles → guard must NOT fire → composite is rebuilt from them (overwrites the seed).
        let node = TeamAnalysis;
        let rebuilt = serde_json::json!({
            "careerTrajectories": [], "skillSignatures": ["go"],
            "seniorityCalibration": "staff", "cultureSignals": []
        });
        let deps = fake_deps(FakeProvider::new(vec![rebuilt]), FakeResearch::default());
        let profile = ProfileAnalysis {
            input_type: "github_url".into(), name: Some("T".into()), career_trajectory: vec![],
            skill_signatures: vec!["go".into()], seniority_level: None, culture_signals: vec![],
            urls: vec!["https://github.com/t".into()], analyzed_at: "t".into() };
        let ctx = IntakeContext { team_profiles: vec![profile], composite_profile: Some(composite()), ..Default::default() };
        let parsed = node.parse("looks good", &ctx, &deps).await.unwrap();
        let cp = parsed.context_updates.composite_profile.expect("rebuilt from real profiles");
        assert_eq!(cp.skill_signatures, vec!["go".to_string()]); // rebuilt ("go"), not the seeded "rust"
    }

    #[tokio::test]
    async fn anti_patterns_seeded_confirm_keeps_without_duplicating() {
        let node = AntiPatterns;
        // Empty queue: a confirm on seeded patterns must not call the LLM.
        let deps = fake_deps(FakeProvider::new(vec![]), FakeResearch::default());
        let ctx = IntakeContext { anti_patterns: vec!["job hopper".into()], ..Default::default() };
        let parsed = node.parse("looks good", &ctx, &deps).await.unwrap();
        assert!(parsed.context_updates.anti_patterns.is_none(), "confirm keeps seeded patterns as-is");
        assert_eq!(node.next(&parsed, &ctx), NodeId::ConfigGenerate);
    }

    #[tokio::test]
    async fn anti_patterns_seeded_addition_appends() {
        let node = AntiPatterns;
        let deps = fake_deps(FakeProvider::new(vec![serde_json::json!(["no public code"])]), FakeResearch::default());
        let ctx = IntakeContext { anti_patterns: vec!["job hopper".into()], ..Default::default() };
        let parsed = node.parse("also avoid people with no public code", &ctx, &deps).await.unwrap();
        // Returns ONLY the new pattern; merge_context_updates appends it onto
        // ctx.anti_patterns (the concat itself is tested in context.rs).
        assert_eq!(parsed.context_updates.anti_patterns, Some(vec!["no public code".to_string()]));
    }

    #[tokio::test]
    async fn anti_patterns_seeded_prompt_lists_them() {
        let node = AntiPatterns;
        let ctx = IntakeContext { anti_patterns: vec!["job hopper".into()], ..Default::default() };
        let prompt = node.prompt(&ctx).await;
        assert!(prompt.contains("job hopper"));
    }
}
