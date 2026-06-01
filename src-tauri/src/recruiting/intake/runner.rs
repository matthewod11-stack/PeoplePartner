//! Intake runner — wires the 4 phases into one graph + result extraction.
//! Port of `~/Projects/Sourcerer/packages/intake/src/intake-runner.ts`.

use super::context::IntakeContext;
use super::deps::{IntakeDeps, IntakeError};
use super::engine::{build_graph, restore_conversation, ConversationEngine, ConversationGraph};
use super::node::NodeId;
use super::phases::search_config::{build_search_config, build_talent_profile};
use super::phases::{company, role, search_config, success_profile};
use super::schemas::{SearchConfig, TalentProfile};

pub struct IntakeResult {
    pub search_config: SearchConfig,
    pub talent_profile: TalentProfile,
    pub similarity_seeds: Vec<String>,
}

pub fn build_intake_graph() -> ConversationGraph {
    let mut nodes = Vec::new();
    nodes.extend(role::create_role_context_nodes());
    nodes.extend(company::create_company_intel_nodes());
    nodes.extend(success_profile::create_success_profile_nodes());
    nodes.extend(search_config::create_search_config_nodes());
    build_graph(nodes)
}

/// Creates the intake engine starting at the first node. `deps` is accepted
/// (even though unused at construction) to match the intended API and the
/// future learning-feature seam `create_intake_engine(deps, initial_context)`.
pub fn create_intake_engine(_deps: &IntakeDeps, initial: Option<IntakeContext>) -> ConversationEngine {
    ConversationEngine::new(build_intake_graph(), NodeId::RoleJdInput, initial)
        .expect("RoleJdInput is always present in the intake graph")
}

pub fn restore_intake_engine(state_json: &str) -> Result<ConversationEngine, IntakeError> {
    restore_conversation(build_intake_graph(), state_json)
}

/// Extracts the final result. Prefers the artifacts already built during the
/// conversation (stashed in context) over rebuilding — avoids a redundant
/// second pair of LLM calls.
pub async fn extract_intake_result(ctx: &IntakeContext, deps: &IntakeDeps)
    -> Result<IntakeResult, IntakeError> {
    let search_config = match &ctx.pending_search_config {
        Some(c) => c.clone(),
        None => build_search_config(ctx, deps).await?,
    };
    let talent_profile = match &ctx.talent_profile {
        Some(t) => t.clone(),
        None => build_talent_profile(ctx, &deps.clock.now_iso8601())?,
    };
    Ok(IntakeResult {
        search_config,
        talent_profile,
        similarity_seeds: ctx.similarity_seeds.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recruiting::intake::deps::testing::{FakeProvider, FakeResearch, fake_deps};
    use crate::recruiting::intake::node::NodeId;
    use crate::recruiting::intake::schemas::{CompanyIntel, ProfileAnalysis};

    fn company() -> CompanyIntel {
        CompanyIntel { name:"Acme".into(), url:"https://acme.com".into(), tech_stack:vec![],
            team_size:None, funding_stage:None, product_category:None, culture_signals:vec![],
            pitch:None, competitors:Some(vec!["Stripe".into()]), analyzed_at:"t".into() }
    }

    #[tokio::test]
    async fn full_intake_walks_all_four_phases_to_done() {
        // Provider queue, in the exact order the walk consumes LLM calls:
        //  [0] role_jd_input  -> RoleParameters
        //  (role confirm "yes": no LLM)
        //  (company_url_input: research crawl, no LLM)
        //  (company_analysis "yes": promote pending, no LLM)
        //  (company_confirm "looks good": builds map from intel.competitors, no LLM)
        //  (team_input: research analyze_profile, no LLM)
        //  [1] team_analysis -> CompositeProfile
        //  [2] anti_patterns -> AntiPatterns (string array)
        //  [3] config_generate -> generate_search_queries (SearchQueryTier[])
        //  [4] config_generate -> propose_scoring_weights (ScoringWeights)
        let provider = FakeProvider::new(vec![
            serde_json::json!({"title":"Eng","level":"staff","scope":"x",
                "mustHaveSkills":[],"niceToHaveSkills":[]}),
            serde_json::json!({"careerTrajectories":[],"skillSignatures":["rust"],
                "seniorityCalibration":"senior","cultureSignals":[]}),
            serde_json::json!(["job hopper"]),
            serde_json::json!([{"priority":1,"queries":[{"text":"q"}]}]),
            serde_json::json!({"technicalDepth":1.0}),
        ]);
        let research = FakeResearch {
            company: Some(company()),
            profiles: vec![ProfileAnalysis { input_type:"github_url".into(), name:Some("T".into()),
                career_trajectory:vec![], skill_signatures:vec!["rust".into()], seniority_level:None,
                culture_signals:vec![], urls:vec!["https://github.com/t".into()], analyzed_at:"t".into() }],
            similar: vec![],
        };
        let deps = fake_deps(provider, research);
        let mut e = create_intake_engine(&deps, None);

        let script = [
            "Senior infra engineer", // role_jd_input
            "yes",                   // role_parse_confirm -> company
            "https://acme.com",      // company_url_input (crawl)
            "yes",                   // company_analysis -> promote -> confirm
            "looks good",            // company_confirm -> team
            "https://github.com/t",  // team_input -> analysis
            "looks accurate",        // team_analysis -> anti_patterns
            "job hopping",           // anti_patterns -> config
            "yes proceed",           // config_generate -> Done
        ];
        for resp in script {
            assert!(e.get_prompt().await.is_some(), "expected a prompt before submitting `{resp}`");
            let step = e.submit_response(resp, &deps).await.unwrap();
            if step.done { break; }
        }
        assert!(e.is_done());
        let ctx = e.context();
        assert!(ctx.role_parameters.is_some(), "role params set");
        assert!(ctx.company_intel.is_some(), "company intel promoted");
        assert!(!ctx.similarity_seeds.is_empty(), "seeds collected");
        assert!(ctx.talent_profile.is_some(), "talent profile built");

        let result = extract_intake_result(ctx, &deps).await.unwrap();
        assert_eq!(result.search_config.role_name, "Eng");
    }

    #[tokio::test]
    async fn graph_targets_all_exist() {
        let graph = build_intake_graph();
        for id in [NodeId::RoleJdInput, NodeId::RoleParseConfirm, NodeId::RoleRefine,
            NodeId::CompanyUrlInput, NodeId::CompanyAnalysis, NodeId::CompanyConfirm,
            NodeId::TeamInput, NodeId::TeamAnalysis, NodeId::AntiPatterns,
            NodeId::ConfigGenerate, NodeId::ConfigReview] {
            assert!(graph.contains_key(&id), "missing node {id:?}");
        }
    }
}
