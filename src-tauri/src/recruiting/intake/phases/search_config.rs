//! Phase 4 — Search config generation. Port of search-config-gen.ts.
//! Smell-fix: config built once in `ConfigGenerate.parse`, stashed in
//! `pending_search_config`; `prompt` is read-only.

use async_trait::async_trait;
use crate::recruiting::intake::context::{IntakeContext, IntakeContextUpdates};
use crate::recruiting::intake::deps::{IntakeDeps, IntakeError};
use crate::recruiting::intake::node::{IntakeNode, NodeId, ParsedResponse, Phase};
use crate::recruiting::intake::schemas::{
    AntiFilter, CompetitorMap, CompositeProfile, EnrichmentPriority, Message, MessageRole,
    ScoringWeights, SearchConfig, SearchQueryTier, TalentProfile, TierThresholds,
};

const QUERIES_SYSTEM: &str = r#"You are a talent sourcing strategist. Generate tiered search queries for finding candidates. Return a JSON array of tier objects, each with:
- priority: 1 (highest precision) to 4 (exploratory)
- queries: array of {text, targetCompanies?, includeDomains?, excludeDomains?, maxResults?}

Guidelines:
- Tier 1: Exact role + company combinations (e.g., "senior backend engineer at Coinbase")
- Tier 2: Skill-focused queries (e.g., "Go distributed systems engineer")
- Tier 3: Domain/trajectory queries (e.g., "DeFi infrastructure engineer")
- Tier 4: Exploratory queries (e.g., "protocol engineer open source contributor")

Target 2-4 queries per tier. Use the provided context to craft highly specific queries."#;

const WEIGHTS_SYSTEM: &str = r#"You are a talent scoring strategist. Propose scoring weights for evaluating candidates. The weights must sum to 1.0. Return a JSON object with these dimensions and their weights:
- technicalDepth: how important is deep technical expertise
- domainRelevance: how important is experience in this specific domain
- trajectoryMatch: how important is a matching career trajectory
- cultureFit: how important is cultural alignment
- reachability: how important is being contactable/recruitable

Consider the role requirements and company context when weighting."#;

const CONFIRM_PHRASES: &[&str] = &["yes", "looks good", "proceed", "lgtm", "good", "confirmed"];

fn matches_any(resp: &str, phrases: &[&str]) -> bool {
    let n = resp.trim().to_lowercase();
    phrases.iter().any(|p| n.contains(p)) || n == "y"
}

fn user_msg(c: String) -> Message { Message { role: MessageRole::User, content: c } }
fn system_msg(c: &str) -> Message { Message { role: MessageRole::System, content: c.into() } }

pub fn dedupe_strings(items: Vec<String>) -> Vec<String> {
    let mut out = Vec::new();
    for i in items { if !out.contains(&i) { out.push(i); } }
    out
}

pub fn default_tier_thresholds() -> TierThresholds {
    TierThresholds { tier1_min_score: 70, tier2_min_score: 40 }
}

pub fn default_enrichment_priority() -> Vec<EnrichmentPriority> {
    vec![
        EnrichmentPriority { adapter: "github".into(), required: true, run_condition: "always".into() },
        EnrichmentPriority { adapter: "exa".into(), required: true, run_condition: "always".into() },
        EnrichmentPriority { adapter: "hunter".into(), required: false, run_condition: "if_cheap_insufficient".into() },
    ]
}

pub fn generate_anti_filters(ctx: &IntakeContext) -> Vec<AntiFilter> {
    let mut filters = Vec::new();
    if let Some(cm) = &ctx.competitor_map {
        for c in &cm.avoid_companies {
            filters.push(AntiFilter {
                kind: "exclude_company".into(), value: c.clone(),
                reason: cm.competitor_reason.get(c).cloned().unwrap_or_else(|| "User excluded".into()),
            });
        }
    }
    for p in &ctx.anti_patterns {
        filters.push(AntiFilter {
            kind: "exclude_signal".into(), value: p.clone(),
            reason: "Anti-pattern identified during intake".into(),
        });
    }
    filters
}

pub fn build_talent_profile(ctx: &IntakeContext, created_at: &str) -> Result<TalentProfile, IntakeError> {
    let role = ctx.role_parameters.clone().ok_or_else(|| IntakeError::Provider("cannot build talent profile without role parameters".into()))?;
    let company = ctx.company_intel.clone().ok_or_else(|| IntakeError::Provider("cannot build talent profile without company intel".into()))?;
    let success_patterns = ctx.composite_profile.clone().unwrap_or_else(|| CompositeProfile {
        career_trajectories: ctx.team_profiles.iter().map(|p| p.career_trajectory.clone()).collect(),
        skill_signatures: dedupe_strings(ctx.team_profiles.iter().flat_map(|p| p.skill_signatures.clone()).collect()),
        seniority_calibration: ctx.team_profiles.first().and_then(|p| p.seniority_level.clone()).unwrap_or_else(|| role.level.clone()),
        culture_signals: dedupe_strings(ctx.team_profiles.iter().flat_map(|p| p.culture_signals.clone()).collect()),
    });
    let competitor_map = ctx.competitor_map.clone().unwrap_or_else(|| CompetitorMap {
        target_companies: vec![], avoid_companies: vec![], competitor_reason: Default::default(),
    });
    Ok(TalentProfile {
        role, company, success_patterns, anti_patterns: ctx.anti_patterns.clone(),
        competitor_map, created_at: created_at.to_string(),
    })
}

async fn generate_search_queries(ctx: &IntakeContext, deps: &IntakeDeps) -> Result<Vec<SearchQueryTier>, IntakeError> {
    let role = ctx.role_parameters.as_ref().ok_or_else(|| IntakeError::Provider("cannot generate search queries without role parameters".into()))?;
    let user = format!(
        "Role: {}\nCompany: {}\nCompetitors/Target companies: {}\nTeam profiles: {} analyzed",
        serde_json::to_string(role).unwrap_or_default(),
        ctx.company_intel.as_ref().map(|c| serde_json::to_string(c).unwrap_or_default()).unwrap_or_else(|| "Not available".into()),
        ctx.competitor_map.as_ref().map(|c| serde_json::to_string(c).unwrap_or_default()).unwrap_or_else(|| "Not specified".into()),
        ctx.team_profiles.len(),
    );
    let v = deps.provider.structured_output(vec![system_msg(QUERIES_SYSTEM), user_msg(user)], "SearchQueryTier").await?;
    serde_json::from_value(v).map_err(|e| IntakeError::Deserialize(e.to_string()))
}

async fn propose_scoring_weights(ctx: &IntakeContext, deps: &IntakeDeps) -> Result<ScoringWeights, IntakeError> {
    let user = format!(
        "Role: {}\nCompany culture: {}\nMust-have skills: {}",
        ctx.role_parameters.as_ref().map(|r| serde_json::to_string(r).unwrap_or_default()).unwrap_or_default(),
        serde_json::to_string(&ctx.company_intel.as_ref().map(|c| c.culture_signals.clone()).unwrap_or_default()).unwrap_or_default(),
        serde_json::to_string(&ctx.role_parameters.as_ref().map(|r| r.must_have_skills.clone()).unwrap_or_default()).unwrap_or_default(),
    );
    let v = deps.provider.structured_output(vec![system_msg(WEIGHTS_SYSTEM), user_msg(user)], "ScoringWeights").await?;
    serde_json::from_value(v).map_err(|e| IntakeError::Deserialize(e.to_string()))
}

pub async fn build_search_config(ctx: &IntakeContext, deps: &IntakeDeps) -> Result<SearchConfig, IntakeError> {
    let role = ctx.role_parameters.clone().ok_or_else(|| IntakeError::Provider("cannot build search config without role parameters".into()))?;
    let (tiers, weights) = tokio::join!(generate_search_queries(ctx, deps), propose_scoring_weights(ctx, deps));
    let tiers = tiers?;
    let weights = weights?;
    Ok(SearchConfig {
        role_name: role.title,
        tiers,
        scoring_weights: weights,
        tier_thresholds: default_tier_thresholds(),
        enrichment_priority: default_enrichment_priority(),
        anti_filters: generate_anti_filters(ctx),
        similarity_seeds: ctx.similarity_seeds.clone(),
        max_candidates: 100,
        max_cost_usd: 5.0,
        created_at: deps.clock.now_iso8601(),
        version: 1,
    })
}

pub struct ConfigGenerate;

#[async_trait]
impl IntakeNode for ConfigGenerate {
    fn id(&self) -> NodeId { NodeId::ConfigGenerate }
    fn phase(&self) -> Phase { Phase::Strategy }

    async fn prompt(&self, ctx: &IntakeContext) -> String {
        if let Some(config) = &ctx.pending_search_config {
            let mut lines = vec![
                format!("I've generated your search configuration:\n"),
                format!("**Role:** {}", config.role_name),
                format!("**Search tiers:** {} tiers, {} queries total",
                    config.tiers.len(),
                    config.tiers.iter().map(|t| t.queries.len()).sum::<usize>()),
                "**Scoring weights:**".to_string(),
            ];
            for (dim, weight) in &config.scoring_weights {
                lines.push(format!("  - {}: {:.0}%", dim, weight * 100.0));
            }
            lines.push(format!("**Anti-filters:** {}", config.anti_filters.len()));
            if !config.similarity_seeds.is_empty() {
                lines.push(format!("**Similarity seeds:** {} URLs", config.similarity_seeds.len()));
            }
            lines.push(format!("**Max candidates:** {}", config.max_candidates));
            lines.push(format!("**Budget cap:** ${}", config.max_cost_usd));
            lines.push("\nWould you like to adjust anything, or shall we proceed?".to_string());
            lines.join("\n")
        } else {
            "I'm ready to generate your search configuration. Based on everything we've discussed — the role, company, team profiles, and anti-patterns — shall I proceed?".to_string()
        }
    }

    async fn parse(&self, response: &str, ctx: &IntakeContext, deps: &IntakeDeps)
        -> Result<ParsedResponse, IntakeError>
    {
        let confirmed = matches_any(response, CONFIRM_PHRASES);
        if confirmed {
            let config = build_search_config(ctx, deps).await?;
            let profile = build_talent_profile(ctx, &deps.clock.now_iso8601())?;
            return Ok(ParsedResponse {
                structured: serde_json::json!({ "confirmed": true }),
                context_updates: IntakeContextUpdates {
                    talent_profile: Some(profile),
                    pending_search_config: Some(config),
                    ..Default::default()
                },
                ..Default::default()
            });
        }
        Ok(ParsedResponse {
            structured: serde_json::json!({ "confirmed": false }),
            follow_up_needed: true,
            follow_up_reason: Some("User wants to adjust search config".into()),
            ..Default::default()
        })
    }

    fn next(&self, parsed: &ParsedResponse, _c: &IntakeContext) -> NodeId {
        if parsed.structured["confirmed"] == serde_json::Value::Bool(true) {
            NodeId::Done
        } else {
            NodeId::ConfigReview
        }
    }
}

pub struct ConfigReview;

#[async_trait]
impl IntakeNode for ConfigReview {
    fn id(&self) -> NodeId { NodeId::ConfigReview }
    fn phase(&self) -> Phase { Phase::Strategy }

    async fn prompt(&self, _ctx: &IntakeContext) -> String {
        "What would you like to adjust? I can change scoring weights, add/remove queries, adjust filters, or change the budget.".into()
    }

    async fn parse(&self, response: &str, _ctx: &IntakeContext, deps: &IntakeDeps)
        -> Result<ParsedResponse, IntakeError>
    {
        let v = deps.provider.structured_output(vec![
            system_msg("Parse the user's search-config adjustment request into a JSON object. Valid keys: maxCandidates (number), maxCostUsd (number), addAntiPattern (string), removeAntiPattern (string), adjustWeight ({dimension, weight}), other (string)."),
            user_msg(response.to_string()),
        ], "Adjustments").await?;
        let mut updates = IntakeContextUpdates::default();
        if let Some(ap) = v.get("addAntiPattern").and_then(|x| x.as_str()) {
            updates.anti_patterns = Some(vec![ap.to_string()]);
        }
        Ok(ParsedResponse { structured: v, context_updates: updates, ..Default::default() })
    }

    fn next(&self, _p: &ParsedResponse, _c: &IntakeContext) -> NodeId { NodeId::ConfigGenerate }
}

pub fn create_search_config_nodes() -> Vec<Box<dyn IntakeNode>> {
    vec![Box::new(ConfigGenerate), Box::new(ConfigReview)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recruiting::intake::context::IntakeContext;
    use crate::recruiting::intake::deps::testing::{FakeProvider, FakeResearch, fake_deps};
    use crate::recruiting::intake::node::{IntakeNode, NodeId};
    use crate::recruiting::intake::schemas::{CompanyIntel, CompetitorMap, RoleParameters};

    fn full_ctx() -> IntakeContext {
        IntakeContext {
            role_parameters: Some(RoleParameters { title:"Eng".into(), level:"staff".into(),
                scope:"x".into(), location:None, remote_policy:None, compensation_range:None,
                must_have_skills:vec![], nice_to_have_skills:vec![], team_size:None, reporting_to:None }),
            company_intel: Some(CompanyIntel { name:"Acme".into(), url:"u".into(), tech_stack:vec![],
                team_size:None, funding_stage:None, product_category:None, culture_signals:vec![],
                pitch:None, competitors:None, analyzed_at:"t".into() }),
            competitor_map: Some(CompetitorMap { target_companies:vec![],
                avoid_companies:vec!["OldCorp".into()], competitor_reason:Default::default() }),
            anti_patterns: vec!["job hopper".into()],
            ..Default::default()
        }
    }

    #[test]
    fn anti_filters_cover_avoid_companies_and_patterns() {
        let f = generate_anti_filters(&full_ctx());
        assert!(f.iter().any(|x| x.kind == "exclude_company" && x.value == "OldCorp"));
        assert!(f.iter().any(|x| x.kind == "exclude_signal" && x.value == "job hopper"));
    }

    #[test]
    fn talent_profile_builds_from_context() {
        let p = build_talent_profile(&full_ctx(), "2026-06-01T00:00:00Z").unwrap();
        assert_eq!(p.role.title, "Eng");
        assert_eq!(p.anti_patterns, vec!["job hopper".to_string()]);
    }

    #[test]
    fn dedupe_preserves_first_seen() {
        assert_eq!(dedupe_strings(vec!["a".into(),"b".into(),"a".into()]),
                   vec!["a".to_string(),"b".to_string()]);
    }

    #[tokio::test]
    async fn config_generate_confirm_goes_to_done() {
        let node = ConfigGenerate;
        let deps = fake_deps(FakeProvider::new(vec![
            serde_json::json!([{"priority":1,"queries":[{"text":"q"}]}]),
            serde_json::json!({"technicalDepth":1.0}),
        ]), FakeResearch::default());
        let parsed = node.parse("yes proceed", &full_ctx(), &deps).await.unwrap();
        assert_eq!(node.next(&parsed, &full_ctx()), NodeId::Done);
        assert!(parsed.context_updates.talent_profile.is_some());
        assert!(parsed.context_updates.pending_search_config.is_some());
    }

    #[tokio::test]
    async fn config_generate_adjust_goes_to_review() {
        let node = ConfigGenerate;
        let deps = fake_deps(FakeProvider::new(vec![]), FakeResearch::default());
        let parsed = node.parse("change the budget", &full_ctx(), &deps).await.unwrap();
        assert_eq!(node.next(&parsed, &full_ctx()), NodeId::ConfigReview);
    }

    #[tokio::test]
    async fn config_review_loops_back_to_generate() {
        let node = ConfigReview;
        let deps = fake_deps(FakeProvider::new(vec![serde_json::json!({"maxCandidates":50})]),
            FakeResearch::default());
        let parsed = node.parse("only 50 candidates", &full_ctx(), &deps).await.unwrap();
        assert_eq!(node.next(&parsed, &full_ctx()), NodeId::ConfigGenerate);
    }
}
